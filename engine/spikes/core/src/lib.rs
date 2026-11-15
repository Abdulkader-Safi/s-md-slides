//! M0 spike: scanner, source map and splice-only writer for front matter,
//! separators and text elements. Throwaway code, folded into `crates/core` in M1.
//!
//! ponytail: every op splices bytes into the source and then re-scans the whole
//! text instead of shifting stored offsets. Re-scanning is O(file size) per op.
//! Move to per-span offset shifting if NFR-01 (one op plus save under 5 ms) fails.

use serde::{Deserialize, Serialize};
use std::ops::Range;

const KNOWN_KINDS: &[&str] = &[
    "slide",
    "master",
    "layout",
    "placeholder",
    "notes",
    "text",
    "image",
    "shape",
    "code",
    "table",
    "diagram",
    "math",
    "video",
    "embed",
    "group",
];

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Attr {
    pub key: String,
    /// Unescaped value. `None` for a bare key.
    pub value: Option<String>,
    /// Whole token, `key=value`, relative to the source.
    pub span: Range<usize>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Element {
    pub id: String,
    pub explicit_id: bool,
    pub kind: String,
    /// Directive line, without its line ending.
    pub line: Range<usize>,
    /// Where a new attribute goes: after the last token, before ` -->`.
    pub insert_at: usize,
    pub attrs: Vec<Attr>,
    /// From after the directive line ending to the next directive, separator or EOF.
    pub body: Range<usize>,
}

impl Element {
    pub fn span(&self) -> Range<usize> {
        self.line.start..self.body.end
    }
    pub fn attr(&self, key: &str) -> Option<&Attr> {
        self.attrs.iter().rev().find(|a| a.key == key)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Slide {
    /// Section content, without the separator line.
    pub span: Range<usize>,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct Deck {
    source: String,
    /// Front matter content between the two `---` lines.
    pub front: Option<Range<usize>>,
    pub slides: Vec<Slide>,
    pub crlf: bool,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    SetGeometry {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        x: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        y: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        w: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        h: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rot: Option<f64>,
    },
    SetBody {
        id: String,
        body: String,
        /// Write the body bytes as given, without fixing line endings. Inverses use this.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        raw: bool,
    },
    RemoveElement {
        id: String,
    },
    /// Replace a directive line as written. Inverse of attribute edits.
    SetDirective {
        id: String,
        line: String,
    },
    /// Insert raw element text before element `index` of `slide`. Inverse of removal.
    InsertRaw {
        slide: usize,
        index: usize,
        text: String,
    },
    Batch {
        ops: Vec<Op>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EngineError {
    pub code: &'static str,
    pub message: String,
}

fn err(code: &'static str, message: impl Into<String>) -> EngineError {
    EngineError {
        code,
        message: message.into(),
    }
}

/// One line: `text` excludes the line ending, `next` is the start of the following line.
struct Line {
    start: usize,
    end: usize,
    next: usize,
}

fn split_lines(src: &str, from: usize) -> Vec<Line> {
    let mut out = Vec::new();
    let bytes = src.as_bytes();
    let mut start = from;
    while start < src.len() {
        let nl = bytes[start..].iter().position(|&b| b == b'\n');
        let (end, next) = match nl {
            Some(i) => {
                let e = start + i;
                let e = if e > start && bytes[e - 1] == b'\r' {
                    e - 1
                } else {
                    e
                };
                (e, start + i + 1)
            }
            None => (src.len(), src.len()),
        };
        out.push(Line { start, end, next });
        start = next;
    }
    out
}

struct Directive {
    kind: String,
    attrs: Vec<Attr>,
    insert_at: usize,
}

/// Parse a whole line as `<!-- kind attr ... -->`. Offsets are absolute (`base` is the line start).
fn parse_directive(line: &str, base: usize) -> Option<Directive> {
    let t = line.trim_end();
    if !t.starts_with("<!--") || !t.ends_with("-->") || t.len() < 7 {
        return None;
    }
    let inner_end = t.len() - 3;
    let b = t.as_bytes();
    let mut i = 4;
    while i < inner_end && b[i] == b' ' {
        i += 1;
    }
    let ks = i;
    if i >= inner_end || !b[i].is_ascii_lowercase() {
        return None;
    }
    while i < inner_end && (b[i].is_ascii_lowercase() || b[i].is_ascii_digit() || b[i] == b'-') {
        i += 1;
    }
    let kind = t[ks..i].to_string();
    if i < inner_end && b[i] != b' ' {
        return None;
    }
    let mut insert_at = i;
    let mut attrs = Vec::new();
    loop {
        while i < inner_end && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= inner_end {
            break;
        }
        let ts = i;
        if !b[i].is_ascii_lowercase() {
            return None;
        }
        while i < inner_end
            && (b[i].is_ascii_lowercase() || b[i].is_ascii_digit() || b[i] == b'.' || b[i] == b'-')
        {
            i += 1;
        }
        let key = t[ts..i].to_string();
        let mut value = None;
        if i < inner_end && b[i] == b'=' {
            i += 1;
            if i < inner_end && b[i] == b'"' {
                i += 1;
                let mut v = String::new();
                let mut closed = false;
                while i < inner_end {
                    match b[i] {
                        b'\\' if i + 1 < inner_end && (b[i + 1] == b'"' || b[i + 1] == b'\\') => {
                            v.push(b[i + 1] as char);
                            i += 2;
                        }
                        b'"' => {
                            i += 1;
                            closed = true;
                            break;
                        }
                        _ => {
                            let ch = t[i..].chars().next()?;
                            v.push(ch);
                            i += ch.len_utf8();
                        }
                    }
                }
                if !closed {
                    return None;
                }
                value = Some(v);
            } else {
                let vs = i;
                while i < inner_end && b[i] != b' ' && b[i] != b'\t' {
                    i += 1;
                }
                value = Some(t[vs..i].to_string());
            }
        }
        if i < inner_end && b[i] != b' ' && b[i] != b'\t' {
            return None;
        }
        attrs.push(Attr {
            key,
            value,
            span: base + ts..base + i,
        });
        insert_at = i;
    }
    let is_directive = KNOWN_KINDS.contains(&kind.as_str()) || attrs.iter().any(|a| a.key == "id");
    is_directive.then_some(Directive {
        kind,
        attrs,
        insert_at: base + insert_at,
    })
}

fn fence_marker(text: &str) -> Option<(u8, usize)> {
    let t = text.trim_start();
    let c = *t.as_bytes().first()?;
    if c != b'`' && c != b'~' {
        return None;
    }
    let n = t.bytes().take_while(|&x| x == c).count();
    (n >= 3).then_some((c, n))
}

fn is_separator(text: &str) -> bool {
    text.trim_end_matches([' ', '\t']) == "---"
}

impl Deck {
    pub fn parse(src: &str) -> Deck {
        let base = if src.starts_with('\u{feff}') { 3 } else { 0 };
        let lines = split_lines(src, base);
        let crlf_count = lines.iter().filter(|l| l.next > l.end + 1).count();
        let lf_count = lines.iter().filter(|l| l.next == l.end + 1).count();
        let mut diagnostics = Vec::new();
        let mut front = None;
        let mut first = 0;
        if lines
            .first()
            .is_some_and(|l| is_separator(&src[l.start..l.end]))
        {
            match lines
                .iter()
                .skip(1)
                .position(|l| is_separator(&src[l.start..l.end]))
            {
                Some(j) => {
                    let close = &lines[j + 1];
                    front = Some(lines[0].next..close.start);
                    first = j + 2;
                }
                None => diagnostics.push(Diagnostic {
                    code: "E-FRONTMATTER",
                    message: "Front matter has no closing --- line.".into(),
                    line: 1,
                    column: 1,
                }),
            }
        }

        let mut slides = Vec::new();
        let mut sec_start = lines.get(first).map_or(src.len(), |l| l.start);
        let mut elements: Vec<Element> = Vec::new();
        let mut fence: Option<(u8, usize)> = None;
        let close_body = |els: &mut Vec<Element>, at: usize| {
            if let Some(e) = els.last_mut() {
                e.body.end = at;
            }
        };
        for l in &lines[first.min(lines.len())..] {
            let text = &src[l.start..l.end];
            if let Some((c, n)) = fence {
                if let Some((c2, n2)) = fence_marker(text) {
                    if c2 == c && n2 >= n && text.trim().bytes().all(|x| x == c) {
                        fence = None;
                    }
                }
                continue;
            }
            if let Some(m) = fence_marker(text) {
                fence = Some(m);
                continue;
            }
            if is_separator(text) {
                close_body(&mut elements, l.start);
                slides.push(Slide {
                    span: sec_start..l.start,
                    elements: std::mem::take(&mut elements),
                });
                sec_start = l.next;
                continue;
            }
            if let Some(d) = parse_directive(text, l.start) {
                close_body(&mut elements, l.start);
                let id = d.attrs.iter().rev().find(|a| a.key == "id");
                elements.push(Element {
                    id: id.and_then(|a| a.value.clone()).unwrap_or_default(),
                    explicit_id: id.is_some(),
                    kind: d.kind,
                    line: l.start..l.end,
                    insert_at: d.insert_at,
                    attrs: d.attrs,
                    body: l.next..l.next,
                });
            }
        }
        if fence.is_some() {
            diagnostics.push(Diagnostic {
                code: "E-UNCLOSED-FENCE",
                message: "A code fence is never closed.".into(),
                line: 0,
                column: 0,
            });
        }
        close_body(&mut elements, src.len());
        slides.push(Slide {
            span: sec_start..src.len(),
            elements,
        });

        let mut deck = Deck {
            source: src.to_string(),
            front,
            slides,
            crlf: crlf_count > lf_count,
            diagnostics,
        };
        deck.assign_ids();
        deck
    }

    /// Give elements without `id=` an in-memory id: kind letter plus counter, skipping used ids.
    fn assign_ids(&mut self) {
        let mut used: std::collections::HashSet<String> = self
            .slides
            .iter()
            .flat_map(|s| &s.elements)
            .filter(|e| e.explicit_id)
            .map(|e| e.id.clone())
            .collect();
        let mut counters = std::collections::HashMap::<char, u32>::new();
        for e in self.slides.iter_mut().flat_map(|s| &mut s.elements) {
            if e.explicit_id {
                continue;
            }
            let letter = e.kind.chars().next().unwrap_or('e');
            let n = counters.entry(letter).or_insert(0);
            loop {
                *n += 1;
                let id = format!("{letter}{n}");
                if used.insert(id.clone()) {
                    e.id = id;
                    break;
                }
            }
        }
    }

    pub fn text(&self) -> &str {
        &self.source
    }

    fn eol(&self) -> &'static str {
        if self.crlf { "\r\n" } else { "\n" }
    }

    fn find(&self, id: &str) -> Result<(usize, usize, &Element), EngineError> {
        for (si, s) in self.slides.iter().enumerate() {
            for (ei, e) in s.elements.iter().enumerate() {
                if e.id == id {
                    return Ok((si, ei, e));
                }
            }
        }
        Err(err("E-NOT-FOUND", format!("No element with id {id}.")))
    }

    /// Apply an op. On error nothing changes. Returns the inverse op.
    pub fn apply(&mut self, op: Op) -> Result<Op, EngineError> {
        if let Op::Batch { ops } = op {
            let backup = self.clone();
            let mut inverses = Vec::with_capacity(ops.len());
            for o in ops {
                match self.apply(o) {
                    Ok(inv) => inverses.push(inv),
                    Err(e) => {
                        *self = backup;
                        return Err(e);
                    }
                }
            }
            inverses.reverse();
            return Ok(Op::Batch { ops: inverses });
        }
        let (range, replacement, inverse, check) = self.plan(&op)?;
        let mut next = String::with_capacity(self.source.len() + replacement.len());
        next.push_str(&self.source[..range.start]);
        next.push_str(&replacement);
        next.push_str(&self.source[range.end..]);
        let candidate = Deck::parse(&next);
        if !check(self, &candidate) {
            return Err(err(
                "E-STRUCTURE",
                "The edit would change the deck structure around the element.",
            ));
        }
        *self = candidate;
        if let Op::InsertRaw { slide, index, .. } = op {
            let id = self.slides[slide].elements[index].id.clone();
            return Ok(Op::RemoveElement { id });
        }
        Ok(inverse)
    }

    #[allow(clippy::type_complexity)]
    fn plan(
        &self,
        op: &Op,
    ) -> Result<(Range<usize>, String, Op, Box<dyn Fn(&Deck, &Deck) -> bool>), EngineError> {
        let counts = |d: &Deck| {
            d.slides
                .iter()
                .map(|s| s.elements.len())
                .collect::<Vec<_>>()
        };
        match op {
            Op::SetGeometry {
                id,
                x,
                y,
                w,
                h,
                rot,
            } => {
                let mut set = Vec::new();
                for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h), ("rot", rot)] {
                    if let Some(v) = v {
                        if !v.is_finite() {
                            return Err(err("E-ATTR-VALUE", format!("{k} must be a number.")));
                        }
                        if (k == "w" || k == "h") && *v < 0.0 {
                            return Err(err("E-ATTR-VALUE", format!("{k} must not be negative.")));
                        }
                        set.push((k.to_string(), Some(fmt_num(*v))));
                    }
                }
                self.plan_attrs(id, &set)
            }
            Op::SetDirective { id, line } => {
                let (_, _, e) = self.find(id)?;
                let inverse = Op::SetDirective {
                    id: id.clone(),
                    line: self.source[e.line.clone()].to_string(),
                };
                let before = counts(self);
                Ok((
                    e.line.clone(),
                    line.clone(),
                    inverse,
                    Box::new(move |_, d| counts(d) == before),
                ))
            }
            Op::SetBody { id, body, raw } => {
                let (si, ei, e) = self.find(id)?;
                let mut body = body.clone();
                if !raw {
                    if self.crlf && !body.contains('\r') {
                        body = body.replace('\n', "\r\n");
                    }
                    if !body.is_empty() && !body.ends_with('\n') && e.body.end < self.source.len() {
                        body.push_str(self.eol());
                    }
                }
                let mut range = e.body.clone();
                let mut inverse = Op::SetBody {
                    id: id.clone(),
                    body: self.source[e.body.clone()].to_string(),
                    raw: true,
                };
                let expect = body.clone();
                let mut replacement = body;
                // Directive on the last line with no line ending: start the body on a new
                // line. Undo restores the whole element so the added line ending goes too.
                if !raw && e.body.start == e.line.end && !replacement.is_empty() {
                    replacement.insert_str(0, self.eol());
                    range = e.line.end..e.line.end;
                    inverse = Op::Batch {
                        ops: vec![
                            Op::RemoveElement { id: id.clone() },
                            Op::InsertRaw {
                                slide: si,
                                index: ei,
                                text: self.source[e.span()].to_string(),
                            },
                        ],
                    };
                }
                let before = counts(self);
                Ok((
                    range,
                    replacement,
                    inverse,
                    Box::new(move |_, d| {
                        counts(d) == before
                            && d.slides
                                .get(si)
                                .and_then(|s| s.elements.get(ei))
                                .is_some_and(|e| {
                                    d.source.get(e.body.clone()) == Some(expect.as_str())
                                })
                    }),
                ))
            }
            Op::RemoveElement { id } => {
                let (si, ei, e) = self.find(id)?;
                let inverse = Op::InsertRaw {
                    slide: si,
                    index: ei,
                    text: self.source[e.span()].to_string(),
                };
                let mut expect = counts(self);
                expect[si] -= 1;
                Ok((
                    e.span(),
                    String::new(),
                    inverse,
                    Box::new(move |_, d| counts(d) == expect),
                ))
            }
            Op::InsertRaw { slide, index, text } => {
                let s = self
                    .slides
                    .get(*slide)
                    .ok_or_else(|| err("E-NOT-FOUND", format!("No slide {slide}.")))?;
                if *index > s.elements.len() {
                    return Err(err("E-NOT-FOUND", format!("No element position {index}.")));
                }
                let at = s.elements.get(*index).map_or(s.span.end, |e| e.line.start);
                let probe = Deck::parse(text);
                let inserted = probe.slides.first().map_or(0, |s| s.elements.len());
                if probe.slides.len() != 1 || inserted != 1 {
                    return Err(err("E-RAW", "Raw text must hold exactly one element."));
                }
                let mut expect = counts(self);
                expect[*slide] += 1;
                // The real inverse needs the id the element gets inside the whole deck,
                // so `apply` fills it in after the insert.
                Ok((
                    at..at,
                    text.clone(),
                    Op::RemoveElement { id: String::new() },
                    Box::new(move |_, d| counts(d) == expect),
                ))
            }
            Op::Batch { .. } => unreachable!("handled in apply"),
        }
    }

    /// Set or remove attributes on one directive line. Writes `id=` when the element had none.
    fn plan_attrs(
        &self,
        id: &str,
        set: &[(String, Option<String>)],
    ) -> Result<(Range<usize>, String, Op, Box<dyn Fn(&Deck, &Deck) -> bool>), EngineError> {
        let (_, _, e) = self.find(id)?;
        let line = &self.source[e.line.clone()];
        let base = e.line.start;
        // (local range, replacement), applied right to left.
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        let mut appended = String::new();
        if !e.explicit_id {
            appended.push_str(&format!(" id={}", e.id));
        }
        for (key, value) in set {
            match (e.attr(key), value) {
                (Some(a), Some(v)) => {
                    if a.value.as_deref() != Some(v.as_str()) {
                        edits.push((
                            a.span.start - base..a.span.end - base,
                            format!("{key}={}", quote(v)),
                        ));
                    }
                }
                (Some(a), None) => {
                    let start = line[..a.span.start - base]
                        .trim_end_matches([' ', '\t'])
                        .len();
                    edits.push((start..a.span.end - base, String::new()));
                }
                (None, Some(v)) => appended.push_str(&format!(" {key}={}", quote(v))),
                (None, None) => {}
            }
        }
        if !appended.is_empty() {
            let at = e.insert_at - base;
            edits.push((at..at, appended));
        }
        edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
        let mut new_line = line.to_string();
        for (r, t) in edits {
            new_line.replace_range(r, &t);
        }
        let target = e.id.clone();
        self.plan(&Op::SetDirective {
            id: id.to_string(),
            line: new_line,
        })
        .map(|(r, t, _, c)| {
            (
                r,
                t,
                Op::SetDirective {
                    id: target,
                    line: line.to_string(),
                },
                c,
            )
        })
    }
}

/// Integers when whole, otherwise up to two decimals with no trailing zeros.
pub fn fmt_num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 {
        format!("{}", r as i64)
    } else {
        let s = format!("{r:.2}");
        s.trim_end_matches('0').to_string()
    }
}

/// Bare when safe, otherwise double-quoted with `\"` and `\\` escapes.
pub fn quote(v: &str) -> String {
    let bare = !v.is_empty()
        && !v.contains(|c: char| c.is_whitespace() || c == '"' || c == '\\')
        && !v.contains("-->");
    if bare {
        v.to_string()
    } else {
        format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\""))
    }
}
