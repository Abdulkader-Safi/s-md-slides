//! Operations: every edit is an `Op`. Each one is validated, applied as one splice into
//! the source text, re-read, checked, and answered with its inverse.
//!
//! ponytail: after each splice the whole text is parsed again rather than shifting
//! stored offsets. That costs O(file size) per op; switch to offset shifting if the
//! NFR-01 benchmark (one op plus save under 5 ms) fails.

use crate::attrs::{Attr, fmt_num, quote};
use crate::deck::{Carry, Deck, Element, Kind, MasterSetting, trim_blank_tail};
use crate::diag::{EngineError, Severity};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// Where `SetZ` moves an element: an index in its slide (0 is the back) or a step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ZTo {
    Index(usize),
    Step(ZStep),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum ZStep {
    Front,
    Back,
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum Op {
    /// Insert a slide before `index` (or at the end). Empty, from a layout, or raw text.
    AddSlide {
        index: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        layout: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw: Option<String>,
    },
    RemoveSlide {
        index: usize,
    },
    /// Copy a slide right after itself, with fresh element ids.
    DuplicateSlide {
        index: usize,
    },
    MoveSlide {
        from: usize,
        to: usize,
    },
    /// Add an element at `index` in the slide's stacking order (default: the top).
    /// Either `kind` plus `attrs` and `body`, or `raw` source text for one element.
    AddElement {
        slide: usize,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<Map<String, Value>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        body: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        raw: Option<String>,
    },
    RemoveElement {
        id: String,
    },
    /// Copy an element right above itself with a fresh id, offset by `dx`, `dy`.
    DuplicateElement {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dx: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dy: Option<f64>,
    },
    /// Move, resize or rotate. On a group it moves and scales the members.
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
    /// Set and remove attributes. Values: string, number, `true` for a bare flag, a
    /// list for comma-separated values; `false` or `null` removes the key.
    SetAttrs {
        id: String,
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        set: Map<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<String>,
    },
    SetBody {
        id: String,
        text: String,
    },
    SetZ {
        id: String,
        to: ZTo,
    },
    /// Set the slide's speaker notes, or remove them with `null`.
    SetNotes {
        slide: usize,
        #[serde(default)]
        text: Option<String>,
    },
    /// Edit the `<!-- slide -->` directive (bg, layout, master, hide, transition).
    SetSlideAttrs {
        slide: usize,
        #[serde(default, skip_serializing_if = "Map::is_empty")]
        set: Map<String, Value>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        remove: Vec<String>,
    },
    /// Set a top-level front matter key, or remove it with `null`.
    SetFrontMatter {
        key: String,
        #[serde(default)]
        value: Option<Value>,
    },
    /// Add a master or layout block given as source text.
    AddDefinition {
        raw: String,
    },
    /// Remove a master or layout. Refused while a slide or layout uses it.
    RemoveDefinition {
        id: String,
    },
    /// Group elements on one slide under a new `group` element.
    Group {
        ids: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
    },
    Ungroup {
        id: String,
    },
    /// Low-level: replace the bytes `old` found at byte offset `at` with `new`. Fails if
    /// the bytes there are not `old`. Every inverse is one of these, so undo restores the
    /// exact bytes. Offsets are UTF-8 byte offsets into `deck.text()`.
    Replace {
        at: usize,
        old: String,
        new: String,
    },
    /// All or nothing. The inverse is one `Batch`.
    Batch {
        ops: Vec<Op>,
    },
}

fn err(code: &'static str, message: impl Into<String>) -> EngineError {
    EngineError::new(code, message)
}

/// An attribute value to write: `key=value` or a bare `key`.
#[derive(Debug, Clone, PartialEq)]
enum AttrVal {
    Val(String),
    Flag,
}

fn token(key: &str, v: &AttrVal) -> String {
    match v {
        AttrVal::Val(s) => format!("{key}={}", quote(s)),
        AttrVal::Flag => key.to_string(),
    }
}

fn valid_key(k: &str) -> bool {
    let b = k.as_bytes();
    !b.is_empty()
        && b[0].is_ascii_lowercase()
        && b.iter()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'.' || *c == b'-')
}

fn valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Attributes to set, then keys to remove.
type Changes = (Vec<(String, AttrVal)>, Vec<String>);

/// Turn op attribute maps into (key, value) pairs to set and keys to remove.
fn attr_changes(set: &Map<String, Value>, remove: &[String]) -> Result<Changes, EngineError> {
    let mut out = Vec::new();
    let mut gone: Vec<String> = Vec::new();
    for k in remove {
        if !valid_key(k) {
            return Err(err(
                "E-ATTR-KEY",
                format!("{k:?} is not a valid attribute name."),
            ));
        }
        gone.push(k.clone());
    }
    for (k, v) in set {
        if !valid_key(k) {
            return Err(err(
                "E-ATTR-KEY",
                format!("{k:?} is not a valid attribute name."),
            ));
        }
        let val = match v {
            Value::String(s) => AttrVal::Val(s.clone()),
            Value::Number(n) => AttrVal::Val(fmt_num(n.as_f64().unwrap_or(0.0))),
            Value::Bool(true) => AttrVal::Flag,
            Value::Bool(false) | Value::Null => {
                gone.push(k.clone());
                continue;
            }
            Value::Array(items) => AttrVal::Val(
                items
                    .iter()
                    .map(|i| match i {
                        Value::String(s) => s.clone(),
                        other => other.to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            Value::Object(_) => {
                return Err(err("E-ATTR-VALUE", format!("{k} can't be an object.")));
            }
        };
        if let AttrVal::Val(s) = &val {
            if s.len() > crate::MAX_ATTR_VALUE_BYTES {
                return Err(err("E-LIMIT", format!("The value of {k} is over 4 KB.")));
            }
            if s.contains(['\n', '\r']) {
                return Err(err("E-ATTR-VALUE", format!("{k} can't hold a line break.")));
            }
        }
        out.push((k.clone(), val));
    }
    Ok((out, gone))
}

/// Check values of keys the engine reads, so an op can't write a broken deck.
fn check_known(set: &[(String, AttrVal)]) -> Result<(), EngineError> {
    for (k, v) in set {
        let s = match v {
            AttrVal::Val(s) => s.as_str(),
            AttrVal::Flag => "",
        };
        let num = || s.parse::<f64>().ok().filter(|f| f.is_finite());
        let ok = match k.as_str() {
            "id" => return Err(err("E-ATTR-ID", "Ids can't be changed with SetAttrs.")),
            "x" | "y" | "rot" => num().is_some(),
            "w" | "h" => num().is_some_and(|f| f >= 0.0),
            "opacity" => num().is_some_and(|f| (0.0..=1.0).contains(&f)),
            "dir" => matches!(s, "auto" | "ltr" | "rtl"),
            _ => true,
        };
        if !ok {
            return Err(err(
                "E-ATTR-VALUE",
                format!("{s:?} is not a valid value for {k}."),
            ));
        }
    }
    Ok(())
}

/// Element counts per container: masters, then layouts' placeholders, then slides.
fn shape(d: &Deck) -> Vec<usize> {
    d.masters
        .iter()
        .map(|m| m.elements.len())
        .chain(d.layouts.iter().map(|l| l.placeholders.len()))
        .chain(d.slides.iter().map(|s| s.elements.len()))
        .collect()
}

/// (container, index) of element `id`; containers are numbered as in `shape`.
fn locate(d: &Deck, id: &str) -> Option<(usize, usize)> {
    let mut c = 0;
    for m in &d.masters {
        if let Some(i) = m.elements.iter().position(|e| e.id == id) {
            return Some((c, i));
        }
        c += 1;
    }
    c += d.layouts.len();
    for s in &d.slides {
        if let Some(i) = s.elements.iter().position(|e| e.id == id) {
            return Some((c, i));
        }
        c += 1;
    }
    None
}

fn container(d: &Deck, c: usize) -> Option<&Vec<Element>> {
    let (m, l) = (d.masters.len(), d.layouts.len());
    if c < m {
        Some(&d.masters[c].elements)
    } else if c >= m + l {
        d.slides.get(c - m - l).map(|s| &s.elements)
    } else {
        None
    }
}

fn element_at(d: &Deck, (c, i): (usize, usize)) -> Option<&Element> {
    container(d, c)?.get(i)
}

type Check = Box<dyn Fn(&Deck, &Deck) -> bool>;

/// One splice: replace `range` with `text`, then `check(old, new)` must hold.
struct Plan {
    range: Range<usize>,
    text: String,
    check: Check,
}

enum Step {
    Edit(Plan),
    /// Run these ops as one batch.
    Expand(Vec<Op>),
}

fn plan(range: Range<usize>, text: String, check: impl Fn(&Deck, &Deck) -> bool + 'static) -> Step {
    Step::Edit(Plan {
        range,
        text,
        check: Box::new(check),
    })
}

/// The new deck keeps element `id` at `at` and the same container sizes.
fn same_at(at: (usize, usize), id: String) -> impl Fn(&Deck, &Deck) -> bool {
    move |old, new| shape(old) == shape(new) && element_at(new, at).is_some_and(|e| e.id == id)
}

fn blank_before(text: &str) -> bool {
    text.trim().is_empty() || text.ends_with("\n\n") || text.ends_with("\r\n\r\n")
}

impl Deck {
    /// Apply one op. On error the deck is unchanged. Returns the inverse op.
    pub fn apply(&mut self, op: Op) -> Result<Op, EngineError> {
        if self.read_only {
            return Err(err("E-READ-ONLY", "This deck is read-only."));
        }
        let ops = match op {
            Op::Batch { ops } => ops,
            op => match self.plan(&op)? {
                Step::Edit(p) => return self.splice(p),
                Step::Expand(ops) => ops,
            },
        };
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
        Ok(Op::Batch { ops: inverses })
    }

    /// Apply several ops as one step.
    pub fn apply_batch(&mut self, ops: Vec<Op>) -> Result<Op, EngineError> {
        self.apply(Op::Batch { ops })
    }

    fn splice(&mut self, p: Plan) -> Result<Op, EngineError> {
        let old = self.source[p.range.clone()].to_string();
        let mut next = String::with_capacity(self.source.len() + p.text.len());
        next.push_str(&self.source[..p.range.start]);
        next.push_str(&p.text);
        next.push_str(&self.source[p.range.end..]);
        if next.len() > crate::MAX_FILE_BYTES {
            return Err(err("E-LIMIT", "The deck would be over 20 MB."));
        }
        // Keep in-memory ids of untouched elements: shift their anchors past the edit.
        let delta = p.text.len() as isize - p.range.len() as isize;
        let carry: Carry = self
            .carry()
            .into_iter()
            .filter_map(|(at, id)| {
                if at < p.range.start {
                    Some((at, id))
                } else if at >= p.range.end {
                    Some(((at as isize + delta) as usize, id))
                } else {
                    None
                }
            })
            .collect();
        let mut candidate = Deck::build(&next, &carry);
        // A theme file loaded by the host stays loaded while front matter still names it.
        if let Some(path) = &self.theme_path
            && candidate.front.theme.as_ref().and_then(Value::as_str) == Some(path.as_str())
        {
            candidate.theme = self.theme.clone();
            candidate.theme_path = Some(path.clone());
        }
        if candidate.read_only && !self.read_only {
            return Err(err(
                "E-LIMIT",
                "The edit would push the deck over a size limit.",
            ));
        }
        if !(p.check)(self, &candidate) {
            return Err(err(
                "E-STRUCTURE",
                "The edit would change the deck around its target (for example text that adds --- or a directive line).",
            ));
        }
        let inverse = Op::Replace {
            at: p.range.start,
            old: p.text,
            new: old,
        };
        *self = candidate;
        Ok(inverse)
    }

    fn find(&self, id: &str) -> Result<((usize, usize), &Element), EngineError> {
        locate(self, id)
            .and_then(|at| element_at(self, at).map(|e| (at, e)))
            .ok_or_else(|| err("E-NOT-FOUND", format!("No element with id {id}.")).on(id))
    }

    fn slide_or_err(&self, i: usize) -> Result<&crate::Slide, EngineError> {
        self.slides
            .get(i)
            .ok_or_else(|| err("E-NOT-FOUND", format!("No slide {i}.")))
    }

    fn eol(&self) -> &'static str {
        self.line_ending.as_str()
    }

    /// Line ending to put before text inserted at `at`, if the text there ends mid-line.
    fn fix_eol(&self, at: usize) -> &'static str {
        let before = &self.source[self.bom.min(at)..at];
        if before.is_empty() || before.ends_with('\n') {
            ""
        } else {
            self.eol()
        }
    }

    /// Convert lone `\n` to the file's line ending and end with one, unless at EOF.
    fn norm_text(&self, text: &str, at_eof: bool) -> String {
        let mut t = if self.eol() == "\r\n" && !text.contains('\r') {
            text.replace('\n', "\r\n")
        } else {
            text.to_string()
        };
        if !t.is_empty() && !t.ends_with('\n') && !at_eof {
            t.push_str(self.eol());
        }
        t
    }

    /// A fresh id: kind letter plus the lowest free counter.
    fn fresh_id(&self, kind: &str, taken: &mut HashSet<String>) -> String {
        let letter = kind.chars().next().unwrap_or('e');
        let used: HashSet<&str> = self.elements().map(|e| e.id.as_str()).collect();
        let mut n = 1;
        loop {
            let id = format!("{letter}{n}");
            if !used.contains(id.as_str()) && !taken.contains(&id) {
                taken.insert(id.clone());
                return id;
            }
            n += 1;
        }
    }

    fn section_pos(&self, slide: usize) -> usize {
        self.sections
            .iter()
            .position(|s| s.slide == Some(slide))
            .unwrap_or(0)
    }

    /// Byte range that removes section `k` together with one neighbouring `---` line.
    fn section_removal(&self, k: usize) -> Range<usize> {
        let span = self.sections[k].span.clone();
        if k + 1 < self.sections.len() {
            span.start..self.sections[k + 1].span.start
        } else if k > 0 {
            self.sections[k - 1].span.end..span.end
        } else {
            span
        }
    }

    /// Where and how to insert a new slide's text so it becomes slide `index`.
    fn slide_insertion(&self, index: usize, new: &str) -> (usize, String) {
        let sep = format!("---{}", self.eol());
        if index < self.slides.len() {
            let at = self.slides[index].src.span.start;
            (at, format!("{new}{sep}"))
        } else if let Some(last) = self.slides.last() {
            let at = last.src.span.end;
            (at, format!("{}{sep}{new}", self.fix_eol(at)))
        } else {
            let at = self.source.len();
            let lead = if self.sections.is_empty() {
                String::new()
            } else {
                sep.clone()
            };
            (at, format!("{}{lead}{new}", self.fix_eol(at)))
        }
    }

    /// Where a new element goes so it lands at `index` on slide `s`.
    fn element_insertion(
        &self,
        s: usize,
        index: usize,
        text: &str,
    ) -> Result<(usize, String), EngineError> {
        let slide = self.slide_or_err(s)?;
        let n = slide.elements.len();
        if index > n {
            return Err(err(
                "E-NOT-FOUND",
                format!("Slide {s} has no position {index}."),
            ));
        }
        if index < n {
            let e = &slide.elements[index];
            if e.implicit {
                return Err(err(
                    "E-STRUCTURE",
                    "Nothing can go below the slide's implicit text. Insert above it instead.",
                ));
            }
            return Ok((e.src.start(), format!("{text}{}", self.eol())));
        }
        let at = slide
            .src
            .notes
            .as_ref()
            .map_or(slide.src.span.end, |n| n.start);
        let before = &self.source[slide.src.span.start..at];
        let blank = if blank_before(before) { "" } else { self.eol() };
        let tail = if slide.src.notes.is_some() {
            self.eol()
        } else {
            ""
        };
        Ok((at, format!("{}{blank}{text}{tail}", self.fix_eol(at))))
    }

    /// Edit a directive line: set and remove attributes, keep everything else as written.
    fn edit_line(
        &self,
        line: &Range<usize>,
        attrs: &[Attr],
        insert_at: usize,
        set: &[(String, AttrVal)],
        remove: &[String],
    ) -> String {
        let base = line.start;
        let text = &self.source[line.clone()];
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        for k in remove {
            for a in attrs.iter().filter(|a| &a.key == k) {
                let start = text[..a.span.start - base]
                    .trim_end_matches([' ', '\t'])
                    .len();
                edits.push((start..a.span.end - base, String::new()));
            }
        }
        let mut appended = String::new();
        for (k, v) in set {
            if remove.contains(k) {
                continue;
            }
            let current = attrs.iter().rev().find(|a| &a.key == k);
            let same = |a: &Attr| match v {
                AttrVal::Val(s) => a.value.as_deref() == Some(s.as_str()),
                AttrVal::Flag => a.value.is_none(),
            };
            match current {
                Some(a) if same(a) => {}
                Some(a) => edits.push((a.span.start - base..a.span.end - base, token(k, v))),
                None => {
                    appended.push(' ');
                    appended.push_str(&token(k, v));
                }
            }
        }
        if !appended.is_empty() {
            let at = insert_at - base;
            edits.push((at..at, appended));
        }
        edits.sort_by_key(|(r, _)| std::cmp::Reverse((r.start, r.end)));
        let mut out = text.to_string();
        for (r, t) in edits {
            out.replace_range(r, &t);
        }
        out
    }

    /// The element's directive line with changes applied and its id written.
    fn element_line(&self, e: &Element, set: &[(String, AttrVal)], remove: &[String]) -> String {
        let mut set = set.to_vec();
        if !e.id_written() {
            set.insert(0, ("id".into(), AttrVal::Val(e.id.clone())));
        }
        match &e.src.line {
            Some(line) => self.edit_line(line, &e.attrs, e.src.insert_at, &set, remove),
            None => self.implicit_line(e, &set),
        }
    }

    /// A directive line for implicit text: id and geometry, then `set`.
    fn implicit_line(&self, e: &Element, set: &[(String, AttrVal)]) -> String {
        let get = |k: &str, v: f64| match set.iter().find(|(key, _)| key == k) {
            Some((_, AttrVal::Val(s))) => s.clone(),
            _ => fmt_num(v),
        };
        let mut line = format!(
            "<!-- text id={} x={} y={} w={} h={}",
            e.id,
            get("x", e.x),
            get("y", e.y),
            get("w", e.w),
            get("h", e.h)
        );
        for (k, v) in set {
            if !matches!(k.as_str(), "id" | "x" | "y" | "w" | "h") {
                line.push(' ');
                line.push_str(&token(k, v));
            }
        }
        line.push_str(" -->");
        line
    }

    /// Replace an element's directive line (writing a new one for implicit text).
    fn rewrite_element(&self, at: (usize, usize), e: &Element, line: String) -> Step {
        let id = e.id.clone();
        match &e.src.line {
            Some(r) => plan(r.clone(), line, same_at(at, id)),
            None => {
                let start = e.src.body.start;
                plan(
                    start..start,
                    format!("{line}{}", self.eol()),
                    same_at(at, id),
                )
            }
        }
    }

    fn plan(&self, op: &Op) -> Result<Step, EngineError> {
        match op {
            Op::SetGeometry {
                id,
                x,
                y,
                w,
                h,
                rot,
            } => {
                let (at, e) = self.find(id)?;
                for (k, v) in [("x", x), ("y", y), ("w", w), ("h", h), ("rot", rot)] {
                    if let Some(v) = v {
                        if !v.is_finite() {
                            return Err(
                                err("E-ATTR-VALUE", format!("{k} must be a number.")).on(id)
                            );
                        }
                        if (k == "w" || k == "h") && *v < 0.0 {
                            return Err(
                                err("E-ATTR-VALUE", format!("{k} must not be negative.")).on(id)
                            );
                        }
                    }
                }
                if e.kind == Kind::Group {
                    return self
                        .group_geometry(e, *x, *y, *w, *h, *rot)
                        .map(Step::Expand);
                }
                let set: Vec<(String, AttrVal)> =
                    [("x", x), ("y", y), ("w", w), ("h", h), ("rot", rot)]
                        .into_iter()
                        .filter_map(|(k, v)| v.map(|v| (k.to_string(), AttrVal::Val(fmt_num(v)))))
                        .collect();
                Ok(self.rewrite_element(at, e, self.element_line(e, &set, &[])))
            }
            Op::SetAttrs { id, set, remove } => {
                let (at, e) = self.find(id)?;
                let (set, remove) = attr_changes(set, remove)?;
                check_known(&set).map_err(|x| x.on(id))?;
                if remove.iter().any(|k| k == "id") {
                    return Err(err("E-ATTR-ID", "Ids can't be removed.").on(id));
                }
                // Known kinds need their geometry (line and arrow shapes may drop h).
                let line_shape =
                    e.kind == Kind::Shape && matches!(e.attr("kind"), Some("line" | "arrow"));
                let needed = |k: &str| {
                    matches!(k, "x" | "y" | "w" | "h")
                        && !(k == "h" && line_shape)
                        && Kind::from_name_pub(e.kind.name()).is_some()
                        && e.kind != Kind::Group
                };
                if let Some(k) = remove.iter().find(|k| needed(k)) {
                    return Err(err(
                        "E-ATTR-MISSING",
                        format!("A {} element needs {k}.", e.kind.name()),
                    )
                    .on(id));
                }
                Ok(self.rewrite_element(at, e, self.element_line(e, &set, &remove)))
            }
            Op::SetBody { id, text } => {
                let (at, e) = self.find(id)?;
                let at_eof = e.src.body.end == self.source.len();
                let body = self.norm_text(text, at_eof);
                let expect = body[..trim_blank_tail(&body, 0..body.len())].to_string();
                let id = id.clone();
                let check = move |old: &Deck, new: &Deck| {
                    shape(old) == shape(new)
                        && element_at(new, at).is_some_and(|e| e.id == id && e.body == expect)
                };
                let (range, text) = match &e.src.line {
                    None => (
                        e.src.body.clone(),
                        format!("{}{}{body}", self.implicit_line(e, &[]), self.eol()),
                    ),
                    Some(line) => {
                        let has_eol = e.src.body.start > line.end;
                        let gap = if has_eol {
                            self.source[line.end..e.src.body.start].to_string()
                        } else if body.is_empty() {
                            String::new()
                        } else {
                            self.eol().to_string()
                        };
                        if e.id_written() {
                            if has_eol {
                                (e.src.body.clone(), body)
                            } else {
                                (line.end..e.src.body.end, format!("{gap}{body}"))
                            }
                        } else {
                            (
                                line.start..e.src.body.end,
                                format!("{}{gap}{body}", self.element_line(e, &[], &[])),
                            )
                        }
                    }
                };
                Ok(plan(range, text, check))
            }
            Op::RemoveElement { id } => {
                let (at, e) = self.find(id)?;
                Ok(plan(
                    e.src.start()..e.src.end,
                    String::new(),
                    move |old, new| {
                        let mut want = shape(old);
                        want[at.0] -= 1;
                        shape(new) == want
                    },
                ))
            }
            Op::Ungroup { id } => {
                let (_, e) = self.find(id)?;
                if e.kind != Kind::Group {
                    return Err(err("E-NOT-GROUP", format!("{id} is not a group.")).on(id));
                }
                self.plan(&Op::RemoveElement { id: id.clone() })
            }
            Op::DuplicateElement { id, dx, dy } => self.plan_duplicate_element(id, *dx, *dy),
            Op::SetZ { id, to } => self.plan_z(id, to),
            Op::AddElement {
                slide,
                index,
                kind,
                attrs,
                body,
                raw,
            } => self.plan_add_element(*slide, *index, kind, attrs, body, raw),
            Op::SetNotes { slide, text } => self.plan_notes(*slide, text.as_deref()),
            Op::SetSlideAttrs { slide, set, remove } => self.plan_slide_attrs(*slide, set, remove),
            Op::SetFrontMatter { key, value } => self.plan_front_matter(key, value.as_ref()),
            Op::AddSlide { index, layout, raw } => self.plan_add_slide(*index, layout, raw),
            Op::RemoveSlide { index } => {
                self.slide_or_err(*index)?;
                let range = self.section_removal(self.section_pos(*index));
                Ok(plan(range, String::new(), |old, new| {
                    new.slides.len() + 1 == old.slides.len()
                }))
            }
            Op::DuplicateSlide { index } => self.plan_duplicate_slide(*index),
            Op::MoveSlide { from, to } => self.plan_move_slide(*from, *to),
            Op::AddDefinition { raw } => self.plan_add_definition(raw),
            Op::RemoveDefinition { id } => self.plan_remove_definition(id),
            Op::Group { ids, id } => self.plan_group(ids, id),
            Op::Replace { at, old, new } => {
                let end = at.checked_add(old.len()).unwrap_or(usize::MAX);
                if self.source.get(*at..end) != Some(old.as_str()) {
                    return Err(err(
                        "E-STALE",
                        "The text at this offset has changed since the op was made.",
                    ));
                }
                Ok(plan(*at..end, new.clone(), |_, _| true))
            }
            Op::Batch { ops } => Ok(Step::Expand(ops.clone())),
        }
    }

    fn plan_duplicate_element(
        &self,
        id: &str,
        dx: Option<f64>,
        dy: Option<f64>,
    ) -> Result<Step, EngineError> {
        let (at, e) = self.find(id)?;
        if e.kind == Kind::Group {
            return Err(err("E-UNSUPPORTED", "Duplicate the members, then group them.").on(id));
        }
        let new_id = self.fresh_id(e.kind.name(), &mut HashSet::new());
        let mut set = vec![("id".to_string(), AttrVal::Val(new_id.clone()))];
        if let Some(dx) = dx {
            set.push(("x".into(), AttrVal::Val(fmt_num(e.x + dx))));
        }
        if let Some(dy) = dy {
            set.push(("y".into(), AttrVal::Val(fmt_num(e.y + dy))));
        }
        let line = match &e.src.line {
            Some(l) => self.edit_line(l, &e.attrs, e.src.insert_at, &set, &[]),
            None => {
                let copy = Element {
                    id: new_id.clone(),
                    ..e.clone()
                };
                self.implicit_line(&copy, &set)
            }
        };
        let rest = &self.source[e.src.body.start..e.src.end];
        let pos = e.src.end;
        let no_gap = rest.is_empty()
            && e.src
                .line
                .as_ref()
                .is_some_and(|l| l.end == e.src.body.start);
        let gap = if no_gap { "" } else { self.eol() };
        let mut text = format!("{}{line}{gap}{rest}", self.fix_eol(pos));
        if pos < self.source.len() && !text.ends_with('\n') {
            text.push_str(self.eol());
        }
        let target = (at.0, at.1 + 1);
        Ok(plan(pos..pos, text, move |old, new| {
            let mut want = shape(old);
            want[at.0] += 1;
            shape(new) == want && element_at(new, target).is_some_and(|e| e.id == new_id)
        }))
    }

    fn plan_z(&self, id: &str, to: &ZTo) -> Result<Step, EngineError> {
        let (at, e) = self.find(id)?;
        if e.implicit {
            return Err(err("E-STRUCTURE", "Implicit text always stays at the back.").on(id));
        }
        let els = container(self, at.0).cloned().unwrap_or_default();
        let floor = usize::from(els.first().is_some_and(|x| x.implicit));
        let top = els.len() - 1;
        let i = at.1;
        let target = match to {
            ZTo::Index(n) => (*n).clamp(floor, top),
            ZTo::Step(ZStep::Front) => top,
            ZTo::Step(ZStep::Back) => floor,
            ZTo::Step(ZStep::Forward) => (i + 1).min(top),
            ZTo::Step(ZStep::Backward) => i.saturating_sub(1).max(floor),
        };
        let (lo, hi) = (i.min(target), i.max(target));
        let mut blocks: Vec<String> = els[lo..=hi]
            .iter()
            .map(|x| self.source[x.src.start()..x.src.end].to_string())
            .collect();
        let moved = blocks.remove(i - lo);
        blocks.insert(target - lo, moved);
        let last = blocks.len() - 1;
        for (k, b) in blocks.iter_mut().enumerate() {
            if k < last && !b.ends_with('\n') {
                b.push_str(self.eol());
            }
        }
        let range = els[lo].src.start()..els[hi].src.end;
        Ok(plan(
            range,
            blocks.concat(),
            same_at((at.0, target), id.to_string()),
        ))
    }

    fn plan_add_element(
        &self,
        slide: usize,
        index: Option<usize>,
        kind: &Option<String>,
        attrs: &Option<Map<String, Value>>,
        body: &Option<String>,
        raw: &Option<String>,
    ) -> Result<Step, EngineError> {
        let s = self.slide_or_err(slide)?;
        let index = index.unwrap_or(s.elements.len());
        let (text, expect_id) = match (raw, kind) {
            (Some(raw), None) => {
                let probe = Deck::parse(raw).deck;
                let ok = probe.slides.len() == 1
                    && probe.slides[0].elements.len() == 1
                    && !probe.slides[0].elements[0].implicit
                    && probe.slides[0].src.directive.is_none()
                    && probe.slides[0].notes.is_none();
                if !ok {
                    return Err(err("E-RAW", "Raw text must hold exactly one element."));
                }
                let pe = &probe.slides[0].elements[0];
                if pe.id_written() && self.element(&pe.id).is_some() {
                    return Err(err("E-DUP-ID", format!("Id {} is already used.", pe.id)));
                }
                let id = pe.id_written().then(|| pe.id.clone());
                (self.norm_text(raw, true), id)
            }
            (None, Some(kind)) => {
                let known = Kind::from_name_pub(kind).is_some();
                let reserved = crate::attrs::KNOWN_KINDS.contains(&kind.as_str()) && !known;
                if !valid_key(kind) || kind.contains('.') || reserved {
                    return Err(err("E-KIND", format!("{kind:?} is not an element kind.")));
                }
                let (set, _) = attr_changes(attrs.as_ref().unwrap_or(&Map::new()), &[])?;
                let id = match set.iter().find(|(k, _)| k == "id") {
                    Some((_, AttrVal::Val(id))) => {
                        if !valid_id(id) || self.element(id).is_some() {
                            return Err(err(
                                "E-DUP-ID",
                                format!("Id {id} is invalid or already used."),
                            ));
                        }
                        id.clone()
                    }
                    _ => self.fresh_id(kind, &mut HashSet::new()),
                };
                let rest: Vec<(String, AttrVal)> =
                    set.into_iter().filter(|(k, _)| k != "id").collect();
                check_known(&rest)?;
                let shape_line = kind == "shape"
                    && rest.iter().any(|(k, v)| {
                        k == "kind" && matches!(v, AttrVal::Val(s) if s == "line" || s == "arrow")
                    });
                if known && kind != "group" {
                    for g in ["x", "y", "w", "h"] {
                        let optional = g == "h" && shape_line;
                        if !optional && !rest.iter().any(|(k, _)| k == g) {
                            return Err(err(
                                "E-ATTR-MISSING",
                                format!("A {kind} element needs {g}."),
                            ));
                        }
                    }
                }
                let mut line = format!("<!-- {kind} id={id}");
                let geo = |k: &str| matches!(k, "x" | "y" | "w" | "h");
                for g in ["x", "y", "w", "h"] {
                    if let Some((k, v)) = rest.iter().find(|(k, _)| k == g) {
                        line.push(' ');
                        line.push_str(&token(k, v));
                    }
                }
                for (k, v) in rest.iter().filter(|(k, _)| !geo(k)) {
                    line.push(' ');
                    line.push_str(&token(k, v));
                }
                line.push_str(" -->");
                let body = body.as_deref().unwrap_or("");
                let text = if body.is_empty() {
                    line
                } else {
                    format!("{line}{}{}", self.eol(), self.norm_text(body, true))
                };
                (text, Some(id))
            }
            _ => {
                return Err(err(
                    "E-ADD-ELEMENT",
                    "Give either kind (with attrs and body) or raw.",
                ));
            }
        };
        let text = text.trim_end_matches(['\r', '\n']).to_string();
        let (at, mut ins) = self.element_insertion(slide, index, &text)?;
        if at < self.source.len() && !ins.ends_with('\n') {
            ins.push_str(self.eol());
        }
        let c = self.masters.len() + self.layouts.len() + slide;
        Ok(plan(at..at, ins, move |old, new| {
            let mut want = shape(old);
            want[c] += 1;
            shape(new) == want
                && expect_id
                    .as_ref()
                    .is_none_or(|id| element_at(new, (c, index)).is_some_and(|e| &e.id == id))
        }))
    }

    fn plan_notes(&self, slide: usize, text: Option<&str>) -> Result<Step, EngineError> {
        let s = self.slide_or_err(slide)?;
        if s.src.notes.as_ref().is_some_and(|n| n.count > 1) {
            return Err(err(
                "E-UNSUPPORTED",
                "This slide has more than one notes block. Merge them by hand first.",
            ));
        }
        let si = slide;
        match (text, &s.src.notes) {
            (None, None) => Ok(plan(0..0, String::new(), |_, _| true)),
            (None, Some(n)) => Ok(plan(n.start..n.end, String::new(), move |old, new| {
                shape(old) == shape(new) && new.slides[si].notes.is_none()
            })),
            (Some(t), notes) => {
                let (range, insert_at) = match notes {
                    Some(n) => (n.body.clone(), None),
                    None => {
                        let at = s.src.span.end;
                        (at..at, Some(at))
                    }
                };
                let at_eof = range.end == self.source.len();
                let t = self.norm_text(t, at_eof);
                let expect = t[..trim_blank_tail(&t, 0..t.len())].to_string();
                let text = match insert_at {
                    None => t,
                    Some(at) => {
                        let before = &self.source[s.src.span.start..at];
                        let blank = if blank_before(before) { "" } else { self.eol() };
                        let eol = self.eol();
                        format!("{}{blank}<!-- notes -->{eol}{t}", self.fix_eol(at))
                    }
                };
                Ok(plan(range, text, move |old, new| {
                    shape(old) == shape(new)
                        && new.slides[si].notes.as_deref().unwrap_or("") == expect
                }))
            }
        }
    }

    fn plan_slide_attrs(
        &self,
        slide: usize,
        set: &Map<String, Value>,
        remove: &[String],
    ) -> Result<Step, EngineError> {
        let s = self.slide_or_err(slide)?;
        let (set, remove) = attr_changes(set, remove)?;
        let si = slide;
        let want: Vec<(String, Option<String>)> = set
            .iter()
            .filter(|(k, _)| !remove.contains(k))
            .map(|(k, v)| {
                let v = match v {
                    AttrVal::Val(s) => Some(s.clone()),
                    AttrVal::Flag => None,
                };
                (k.clone(), v)
            })
            .collect();
        let gone = remove.clone();
        let check = move |old: &Deck, new: &Deck| {
            let attrs = &new.slides[si].attrs;
            shape(old) == shape(new)
                && want.iter().all(|(k, v)| {
                    attrs
                        .iter()
                        .rev()
                        .find(|a| &a.key == k)
                        .is_some_and(|a| &a.value == v)
                })
                && gone.iter().all(|k| attrs.iter().all(|a| &a.key != k))
        };
        match &s.src.directive {
            Some(line) => {
                let text = &self.source[line.clone()];
                let insert_at = s.attrs.last().map_or_else(
                    || line.start + text.find("slide").map_or(4, |p| p + 5),
                    |a| a.span.end,
                );
                let new_line = self.edit_line(line, &s.attrs, insert_at, &set, &remove);
                Ok(plan(line.clone(), new_line, check))
            }
            None => {
                let mut line = "<!-- slide".to_string();
                for (k, v) in set.iter().filter(|(k, _)| !remove.contains(k)) {
                    line.push(' ');
                    line.push_str(&token(k, v));
                }
                line.push_str(" -->");
                let at = s.src.span.start;
                Ok(plan(
                    at..at,
                    format!("{}{line}{}", self.fix_eol(at), self.eol()),
                    check,
                ))
            }
        }
    }

    fn plan_add_slide(
        &self,
        index: usize,
        layout: &Option<String>,
        raw: &Option<String>,
    ) -> Result<Step, EngineError> {
        let index = index.min(self.slides.len());
        let mut taken = HashSet::new();
        let new = match (raw, layout) {
            (Some(raw), None) => {
                let probe = Deck::parse(raw);
                let d = &probe.deck;
                let ok = d.slides.len() == 1
                    && d.masters.is_empty()
                    && d.layouts.is_empty()
                    && d.front_span.is_none();
                if !ok {
                    return Err(err("E-RAW", "Raw text must hold exactly one slide."));
                }
                if let Some(dup) = d
                    .elements()
                    .find(|e| e.id_written() && self.element(&e.id).is_some())
                {
                    return Err(err("E-DUP-ID", format!("Id {} is already used.", dup.id)));
                }
                self.norm_text(raw, false)
            }
            (None, Some(layout)) => {
                let l = self
                    .layouts
                    .iter()
                    .find(|l| &l.id == layout)
                    .ok_or_else(|| err("E-NOT-FOUND", format!("No layout {layout}.")))?;
                let eol = self.eol();
                let mut t = format!("<!-- slide layout={} -->{eol}", quote(layout));
                for p in &l.placeholders {
                    let kind = if Kind::from_name_pub(&p.kind).is_some() {
                        p.kind.as_str()
                    } else {
                        "text"
                    };
                    let id = self.fresh_id(kind, &mut taken);
                    // Style set on the placeholder carries over to the new element.
                    let extra: String = p
                        .attrs
                        .iter()
                        .filter(|a| {
                            !matches!(a.key.as_str(), "id" | "x" | "y" | "w" | "h" | "kind")
                        })
                        .map(|a| match &a.value {
                            Some(v) => format!(" {}={}", a.key, quote(v)),
                            None => format!(" {}", a.key),
                        })
                        .collect();
                    t.push_str(&format!(
                        "{eol}<!-- {kind} id={id} x={} y={} w={} h={}{extra} -->{eol}",
                        fmt_num(p.x),
                        fmt_num(p.y),
                        fmt_num(p.w),
                        fmt_num(p.h)
                    ));
                }
                t
            }
            (None, None) => format!("<!-- slide -->{}", self.eol()),
            _ => return Err(err("E-ADD-SLIDE", "Give layout or raw, not both.")),
        };
        let count = Deck::parse(&new)
            .deck
            .slides
            .first()
            .map_or(0, |s| s.elements.len());
        let (at, text) = self.slide_insertion(index, &new);
        Ok(plan(at..at, text, move |old, new| {
            new.slides.len() == old.slides.len() + 1 && new.slides[index].elements.len() == count
        }))
    }

    fn plan_duplicate_slide(&self, index: usize) -> Result<Step, EngineError> {
        let s = self.slide_or_err(index)?;
        let base = s.src.span.start;
        let mut taken = HashSet::new();
        let mut remap: HashMap<String, String> = HashMap::new();
        for e in s.elements.iter().filter(|e| e.id_written()) {
            remap.insert(e.id.clone(), self.fresh_id(e.kind.name(), &mut taken));
        }
        let mut edits: Vec<(Range<usize>, String)> = Vec::new();
        for e in &s.elements {
            for a in &e.attrs {
                let Some(vs) = &a.value_span else { continue };
                let v = a.value.as_deref().unwrap_or("");
                let new = match a.key.as_str() {
                    "id" => remap.get(v).cloned(),
                    "items" if e.kind == Kind::Group => Some(
                        v.split(',')
                            .map(|i| remap.get(i.trim()).map_or(i.trim(), |s| s).to_string())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    _ => None,
                };
                if let Some(n) = new {
                    edits.push((vs.start - base..vs.end - base, quote(&n)));
                }
            }
        }
        edits.sort_by_key(|(r, _)| std::cmp::Reverse(r.start));
        let mut copy = self.source[s.src.span.clone()].to_string();
        for (r, t) in edits {
            copy.replace_range(r, &t);
        }
        let at = s.src.span.end;
        if at < self.source.len() && !copy.is_empty() && !copy.ends_with('\n') {
            copy.push_str(self.eol());
        }
        let text = format!("{}---{}{copy}", self.fix_eol(at), self.eol());
        let n = s.elements.len();
        let target = index + 1;
        Ok(plan(at..at, text, move |old, new| {
            new.slides.len() == old.slides.len() + 1 && new.slides[target].elements.len() == n
        }))
    }

    fn plan_move_slide(&self, from: usize, to: usize) -> Result<Step, EngineError> {
        self.slide_or_err(from)?;
        self.slide_or_err(to)?;
        let (pf, pt) = (self.section_pos(from), self.section_pos(to));
        let (lo, hi) = (pf.min(pt), pf.max(pt));
        let secs = &self.sections[lo..=hi];
        let mut slides: Vec<String> = secs
            .iter()
            .filter(|s| s.slide.is_some())
            .map(|s| self.source[s.span.clone()].to_string())
            .collect();
        let first_slide = secs.iter().find_map(|s| s.slide).unwrap_or(0);
        let moved = slides.remove(from - first_slide);
        slides.insert(to - first_slide, moved);
        let mut slides = slides.into_iter();
        let mut out = String::new();
        for (k, s) in secs.iter().enumerate() {
            let mut text = match s.slide {
                Some(_) => slides.next().unwrap_or_default(),
                None => self.source[s.span.clone()].to_string(),
            };
            if k + 1 < secs.len() {
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push_str(self.eol());
                }
                out.push_str(&text);
                out.push_str(&self.source[s.span.end..secs[k + 1].span.start]);
            } else {
                out.push_str(&text);
            }
        }
        let range = secs[0].span.start..secs[secs.len() - 1].span.end;
        let mut counts: Vec<usize> = self.slides.iter().map(|s| s.elements.len()).collect();
        let c = counts.remove(from);
        counts.insert(to, c);
        Ok(plan(range, out, move |_, new| {
            new.slides
                .iter()
                .map(|s| s.elements.len())
                .collect::<Vec<_>>()
                == counts
        }))
    }

    fn plan_add_definition(&self, raw: &str) -> Result<Step, EngineError> {
        let probe = Deck::parse(raw).deck;
        let defs = probe.masters.len() + probe.layouts.len();
        if defs != 1
            || !probe.slides.is_empty()
            || probe.front_span.is_some()
            || probe.sections.len() != 1
        {
            return Err(err(
                "E-RAW",
                "Raw text must hold exactly one master or layout.",
            ));
        }
        let id = probe
            .masters
            .first()
            .map(|m| &m.id)
            .or(probe.layouts.first().map(|l| &l.id))
            .cloned()
            .unwrap_or_default();
        let taken =
            self.masters.iter().any(|m| m.id == id) || self.layouts.iter().any(|l| l.id == id);
        if taken {
            return Err(err(
                "E-DUP-ID",
                format!("A master or layout {id} already exists."),
            ));
        }
        if let Some(dup) = probe
            .elements()
            .find(|e| e.id_written() && self.element(&e.id).is_some())
        {
            return Err(err("E-DUP-ID", format!("Id {} is already used.", dup.id)));
        }
        let block = self.norm_text(raw.trim_end_matches(['\r', '\n']), false);
        let (at, text) = match self.sections.iter().rposition(|s| s.slide.is_none()) {
            Some(k) => {
                let at = self.sections[k].span.end;
                let mut t = format!("{}{}{block}", self.fix_eol(at), self.eol());
                if at == self.source.len() {
                    t.truncate(t.trim_end_matches(['\r', '\n']).len());
                }
                (at, t)
            }
            None => match self.slides.first() {
                Some(s) => (s.src.span.start, format!("{block}---{}", self.eol())),
                None => {
                    let at = self.source.len();
                    (at, format!("{}{block}", self.fix_eol(at)))
                }
            },
        };
        let n = self.masters.len() + self.layouts.len();
        Ok(plan(at..at, text, move |old, new| {
            new.masters.len() + new.layouts.len() == n + 1
                && new
                    .slides
                    .iter()
                    .map(|s| s.elements.len())
                    .eq(old.slides.iter().map(|s| s.elements.len()))
        }))
    }

    fn plan_remove_definition(&self, id: &str) -> Result<Step, EngineError> {
        let master = self
            .masters
            .iter()
            .find(|m| m.id == id)
            .map(|m| m.src.clone());
        let layout = self
            .layouts
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.src.clone());
        let src = master
            .clone()
            .or(layout)
            .ok_or_else(|| err("E-NOT-FOUND", format!("No master or layout {id}.")))?;
        let used = if master.is_some() {
            self.slides
                .iter()
                .any(|s| s.master == MasterSetting::Id(id.to_string()))
                || self.layouts.iter().any(|l| l.master.as_deref() == Some(id))
        } else {
            self.slides.iter().any(|s| s.layout.as_deref() == Some(id))
        };
        if used {
            return Err(err(
                "E-IN-USE",
                format!("{id} is still used by a slide or layout."),
            ));
        }
        let siblings = self
            .masters
            .iter()
            .map(|m| m.src.section)
            .chain(self.layouts.iter().map(|l| l.src.section))
            .filter(|s| *s == src.section)
            .count();
        let range = if siblings == 1 {
            self.section_removal(src.section)
        } else {
            src.block.clone()
        };
        let n = self.masters.len() + self.layouts.len();
        Ok(plan(range, String::new(), move |old, new| {
            new.masters.len() + new.layouts.len() + 1 == n
                && new
                    .slides
                    .iter()
                    .map(|s| s.elements.len())
                    .eq(old.slides.iter().map(|s| s.elements.len()))
        }))
    }

    fn plan_group(&self, ids: &[String], id: &Option<String>) -> Result<Step, EngineError> {
        if ids.len() < 2 {
            return Err(err("E-GROUP", "A group needs at least two elements."));
        }
        let first_slide = self.masters.len() + self.layouts.len();
        let mut slide = None;
        let mut top = 0;
        let mut seen = HashSet::new();
        for m in ids {
            if !seen.insert(m) {
                return Err(err("E-GROUP", format!("{m} is listed twice.")).on(m));
            }
            let ((c, i), e) = self.find(m)?;
            let s = c.checked_sub(first_slide);
            if s.is_none() || slide.is_some_and(|x| Some(x) != s) {
                return Err(err("E-GROUP", "Group members must be on the same slide.").on(m));
            }
            if e.kind == Kind::Group {
                return Err(err("E-GROUP", "Groups can't be nested.").on(m));
            }
            slide = s;
            top = top.max(i);
        }
        let slide = slide.unwrap_or(0);
        let gid = match id {
            Some(g) if !valid_id(g) || self.element(g).is_some() => {
                return Err(err(
                    "E-DUP-ID",
                    format!("Id {g} is invalid or already used."),
                ));
            }
            Some(g) => g.clone(),
            None => self.fresh_id("group", &mut HashSet::new()),
        };
        let mut ops: Vec<Op> = ids
            .iter()
            .filter(|m| self.element(m).is_some_and(|e| !e.id_written()))
            .map(|m| Op::SetAttrs {
                id: m.clone(),
                set: Map::new(),
                remove: Vec::new(),
            })
            .collect();
        ops.push(Op::AddElement {
            slide,
            index: Some(top + 1),
            kind: None,
            attrs: None,
            body: None,
            raw: Some(format!("<!-- group id={gid} items={} -->", ids.join(","))),
        });
        Ok(Step::Expand(ops))
    }

    /// Moving or resizing a group moves and scales its members.
    fn group_geometry(
        &self,
        g: &Element,
        x: Option<f64>,
        y: Option<f64>,
        w: Option<f64>,
        h: Option<f64>,
        rot: Option<f64>,
    ) -> Result<Vec<Op>, EngineError> {
        if rot.is_some() {
            return Err(err("E-UNSUPPORTED", "Rotate group members one by one.").on(&g.id));
        }
        let members: Vec<&Element> = g
            .attr("items")
            .unwrap_or("")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|m| {
                self.element(m).ok_or_else(|| {
                    err("E-NOT-FOUND", format!("Group member {m} is missing.")).on(&g.id)
                })
            })
            .collect::<Result<_, _>>()?;
        if members.is_empty() {
            return Err(err("E-GROUP", "This group has no members.").on(&g.id));
        }
        let gx = members.iter().map(|m| m.x).fold(f64::INFINITY, f64::min);
        let gy = members.iter().map(|m| m.y).fold(f64::INFINITY, f64::min);
        let gw = members
            .iter()
            .map(|m| m.x + m.w)
            .fold(f64::NEG_INFINITY, f64::max)
            - gx;
        let gh = members
            .iter()
            .map(|m| m.y + m.h)
            .fold(f64::NEG_INFINITY, f64::max)
            - gy;
        let scale = |want: Option<f64>, have: f64| -> Result<f64, EngineError> {
            match want {
                Some(_) if have == 0.0 => Err(err(
                    "E-UNSUPPORTED",
                    "A group with no width or height can't be resized.",
                )
                .on(&g.id)),
                Some(v) => Ok(v / have),
                None => Ok(1.0),
            }
        };
        let (sx, sy) = (scale(w, gw)?, scale(h, gh)?);
        let (nx, ny) = (x.unwrap_or(gx), y.unwrap_or(gy));
        Ok(members
            .iter()
            .map(|m| Op::SetGeometry {
                id: m.id.clone(),
                x: (x.is_some() || w.is_some()).then_some(nx + (m.x - gx) * sx),
                y: (y.is_some() || h.is_some()).then_some(ny + (m.y - gy) * sy),
                w: w.map(|_| m.w * sx),
                h: h.map(|_| m.h * sy),
                rot: None,
            })
            .collect())
    }

    fn plan_front_matter(&self, key: &str, value: Option<&Value>) -> Result<Step, EngineError> {
        let ok_key = !key.is_empty()
            && !key.starts_with('-')
            && key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'));
        if !ok_key {
            return Err(err(
                "E-ATTR-KEY",
                format!("{key:?} is not a valid front matter key."),
            ));
        }
        let line = match value {
            None | Some(Value::Null) => None,
            Some(v) => Some(format!("{key}: {}{}", yaml_value(v), self.eol())),
        };
        let front_errors = |d: &Deck| {
            let end = d
                .front_span
                .as_ref()
                .map_or(0, |r| d.source[..r.end].lines().count() as u32 + 1);
            d.diagnostics
                .iter()
                .filter(|x| x.severity == Severity::Error && x.line <= end)
                .count()
        };
        let check = move |old: &Deck, new: &Deck| {
            shape(old) == shape(new)
                && new.front_span.is_some()
                && front_errors(new) <= front_errors(old)
        };
        let Some(fm) = self.front_span.clone() else {
            return Ok(match line {
                None => plan(0..0, String::new(), |_, _| true),
                Some(l) => {
                    let eol = self.eol();
                    plan(self.bom..self.bom, format!("---{eol}{l}---{eol}"), check)
                }
            });
        };
        // The key line plus any indented or list lines that continue its value.
        let text = &self.source[fm.clone()];
        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let mut at = fm.start;
        let mut found: Option<Range<usize>> = None;
        for (i, l) in lines.iter().enumerate() {
            let is_key = l.strip_prefix(key).is_some_and(|r| {
                r.strip_prefix(':')
                    .is_some_and(|v| v.is_empty() || v.starts_with([' ', '\t', '\r', '\n']))
            });
            if is_key && found.is_none() {
                let mut end = at + l.len();
                for (j, n) in lines.iter().enumerate().skip(i + 1) {
                    let cont =
                        n.starts_with([' ', '\t']) || n.starts_with("- ") || n.trim_end() == "-";
                    let blank_then_cont = n.trim().is_empty()
                        && lines[j + 1..]
                            .iter()
                            .find(|x| !x.trim().is_empty())
                            .is_some_and(|x| x.starts_with([' ', '\t']));
                    if !(cont || blank_then_cont) {
                        break;
                    }
                    end += n.len();
                }
                found = Some(at..end);
            }
            at += l.len();
        }
        Ok(match (found, line) {
            (Some(r), l) => plan(r, l.unwrap_or_default(), check),
            (None, None) => plan(0..0, String::new(), |_, _| true),
            (None, Some(l)) => plan(fm.end..fm.end, l, check),
        })
    }
}

/// A YAML value that reads back as `v`: plain when safe, otherwise JSON (valid YAML).
fn yaml_value(v: &Value) -> String {
    match v {
        Value::String(s) => {
            let plain = !s.is_empty()
                && s.chars()
                    .all(|c| c.is_alphanumeric() || " _./-()".contains(c))
                && !s.starts_with([' ', '-'])
                && !s.ends_with(' ')
                && s.parse::<f64>().is_err()
                && !matches!(
                    s.to_ascii_lowercase().as_str(),
                    "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "~" | "y" | "n"
                );
            if plain {
                s.clone()
            } else {
                serde_json::to_string(s).unwrap_or_default()
            }
        }
        other => other.to_string(),
    }
}

impl Kind {
    /// The kind for a known element name, `None` for anything else.
    pub(crate) fn from_name_pub(s: &str) -> Option<Kind> {
        Some(match s {
            "text" => Kind::Text,
            "image" => Kind::Image,
            "shape" => Kind::Shape,
            "code" => Kind::Code,
            "table" => Kind::Table,
            "diagram" => Kind::Diagram,
            "math" => Kind::Math,
            "video" => Kind::Video,
            "embed" => Kind::Embed,
            "group" => Kind::Group,
            _ => return None,
        })
    }
}
