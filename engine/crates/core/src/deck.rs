//! The deck model and the parser that builds it from text.
//!
//! The model is a view over the source text. Every piece keeps the byte spans it came
//! from, so edits splice the source and never print the file from the model.

use crate::attrs::{Attr, Directive, parse_directive};
use crate::diag::{Diagnostic, LineIndex, Severity};
use crate::frontmatter::{self, Canvas, Dir, FrontMatter};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
pub const MAX_SLIDES: usize = 5_000;
pub const MAX_ELEMENTS_PER_SLIDE: usize = 2_000;
pub const MAX_ATTR_VALUE_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    Lf,
    Crlf,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
pub enum Kind {
    Text,
    Image,
    Shape,
    Code,
    Table,
    Diagram,
    Math,
    Video,
    Embed,
    Group,
    Unknown(String),
}

impl Kind {
    fn from_name(s: &str) -> Kind {
        match s {
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
            other => Kind::Unknown(other.to_string()),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Kind::Text => "text",
            Kind::Image => "image",
            Kind::Shape => "shape",
            Kind::Code => "code",
            Kind::Table => "table",
            Kind::Diagram => "diagram",
            Kind::Math => "math",
            Kind::Video => "video",
            Kind::Embed => "embed",
            Kind::Group => "group",
            Kind::Unknown(s) => s,
        }
    }
}

impl From<Kind> for String {
    fn from(k: Kind) -> String {
        k.name().to_string()
    }
}

/// Where an element sits in the source.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ElementSrc {
    /// Directive line without its line ending. `None` for implicit text.
    pub line: Option<Range<usize>>,
    /// Where a new attribute goes in the directive line.
    pub insert_at: usize,
    /// Body without trailing blank lines.
    pub body: Range<usize>,
    /// End of the trailing blank lines after the body.
    pub end: usize,
    /// The `id=` value as written, if any.
    pub written_id: Option<String>,
}

impl ElementSrc {
    pub(crate) fn start(&self) -> usize {
        self.line.as_ref().map_or(self.body.start, |l| l.start)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Element {
    pub id: String,
    pub kind: Kind,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub rot: f32,
    pub opacity: f32,
    pub locked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<Dir>,
    /// Every attribute as written, in order (known ones included).
    pub attrs: Vec<Attr>,
    /// Raw markdown, never normalized.
    pub body: String,
    /// Markdown before a slide's first directive, read as a full-slide text element.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub implicit: bool,
    #[serde(skip)]
    pub(crate) src: ElementSrc,
}

impl Element {
    /// Last value written for `key`.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .rev()
            .find(|a| a.key == key)
            .and_then(|a| a.value.as_deref())
    }
    /// True once the element's id is in the file.
    pub fn id_written(&self) -> bool {
        self.src.written_id.as_deref() == Some(self.id.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
pub enum MasterSetting {
    Inherit,
    None,
    Id(String),
}

impl From<MasterSetting> for String {
    fn from(m: MasterSetting) -> String {
        match m {
            MasterSetting::Inherit => "inherit".into(),
            MasterSetting::None => "none".into(),
            MasterSetting::Id(s) => s,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct SlideSrc {
    /// Section content, without the separator line.
    pub span: Range<usize>,
    /// The `<!-- slide ... -->` line, if any.
    pub directive: Option<Range<usize>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Slide {
    pub index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    pub master: MasterSetting,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,
    /// Attributes of the `<!-- slide -->` directive as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attrs: Vec<Attr>,
    /// File order is stacking order.
    pub elements: Vec<Element>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip)]
    pub(crate) src: SlideSrc,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Master {
    pub id: String,
    pub attrs: Vec<Attr>,
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Placeholder {
    pub id: String,
    pub kind: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub attrs: Vec<Attr>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Layout {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master: Option<String>,
    pub attrs: Vec<Attr>,
    pub placeholders: Vec<Placeholder>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Deck {
    #[serde(skip)]
    pub(crate) source: String,
    pub version: u32,
    pub front: FrontMatter,
    pub masters: Vec<Master>,
    pub layouts: Vec<Layout>,
    pub slides: Vec<Slide>,
    pub line_ending: LineEnding,
    #[serde(skip)]
    pub(crate) diagnostics: Vec<Diagnostic>,
    #[serde(skip)]
    pub(crate) read_only: bool,
}

/// Result of [`Deck::parse`]: always a deck, plus whatever went wrong.
#[derive(Debug, Clone)]
pub struct Parsed {
    pub deck: Deck,
    pub diagnostics: Vec<Diagnostic>,
}

/// In-memory ids of elements without `id=`, keyed by where they start.
pub(crate) type Carry = HashMap<usize, String>;

struct Line {
    start: usize,
    end: usize,
    next: usize,
}

fn split_lines(src: &str, from: usize) -> Vec<Line> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut start = from;
    while start < src.len() {
        let (end, next) = match b[start..].iter().position(|&c| c == b'\n') {
            Some(i) => {
                let nl = start + i;
                let end = if nl > start && b[nl - 1] == b'\r' {
                    nl - 1
                } else {
                    nl
                };
                (end, nl + 1)
            }
            None => (src.len(), src.len()),
        };
        out.push(Line { start, end, next });
        start = next;
    }
    out
}

fn fence_marker(text: &str) -> Option<(u8, usize)> {
    let t = text.trim_start_matches(' ');
    if text.len() - t.len() > 3 {
        return None;
    }
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

/// End of `range` with trailing blank lines dropped (keeps the last line's ending).
pub(crate) fn trim_blank_tail(src: &str, range: Range<usize>) -> usize {
    let s = &src[range.clone()];
    match s.rfind(|c: char| !c.is_whitespace()) {
        None => range.start,
        Some(p) => match s[p..].find('\n') {
            Some(nl) => range.start + p + nl + 1,
            None => range.end,
        },
    }
}

/// Start of the first non-blank line in `range`.
fn skip_blank_head(src: &str, range: Range<usize>) -> usize {
    let s = &src[range.clone()];
    match s.find(|c: char| !c.is_whitespace()) {
        None => range.end,
        Some(p) => range.start + s[..p].rfind('\n').map_or(0, |nl| nl + 1),
    }
}

fn valid_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

const ELEMENT_KINDS: &[&str] = &[
    "text", "image", "shape", "code", "table", "diagram", "math", "video", "embed", "group",
];

fn is_element_kind(kind: &str) -> bool {
    ELEMENT_KINDS.contains(&kind) || !crate::attrs::KNOWN_KINDS.contains(&kind)
}

/// Collects diagnostics with positions.
struct Ctx<'a> {
    src: &'a str,
    index: LineIndex,
    diags: Vec<Diagnostic>,
    canvas: Canvas,
}

impl Ctx<'_> {
    fn push(
        &mut self,
        severity: Severity,
        code: &'static str,
        message: String,
        offset: usize,
        element: Option<&str>,
        slide: Option<usize>,
    ) {
        let (line, column) = self.index.pos(self.src, offset);
        self.diags.push(Diagnostic {
            severity,
            code,
            message,
            line,
            column,
            element: element.map(str::to_string),
            slide,
        });
    }
}

/// Where a slide or definition section starts and the directives found in it.
struct Section {
    span: Range<usize>,
    directives: Vec<(Range<usize>, usize, Directive)>, // (line text range, next line start, directive)
}

impl Deck {
    /// Parse a deck. Never fails: problems come back as diagnostics.
    pub fn parse(src: &str) -> Parsed {
        let deck = Deck::build(src, &Carry::new());
        Parsed {
            diagnostics: deck.diagnostics.clone(),
            deck,
        }
    }

    /// The current file text. Unchanged decks return the input byte for byte.
    pub fn text(&self) -> &str {
        &self.source
    }

    /// Diagnostics for the current text.
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// True for decks from a newer format version or over a size limit. Ops refuse to run.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    pub fn canvas(&self) -> Canvas {
        self.front.canvas
    }

    pub fn slide(&self, index: usize) -> Option<&Slide> {
        self.slides.get(index)
    }

    /// Every element, masters first, then slides in order.
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.masters
            .iter()
            .flat_map(|m| &m.elements)
            .chain(self.slides.iter().flat_map(|s| &s.elements))
    }

    pub fn element(&self, id: &str) -> Option<&Element> {
        self.elements().find(|e| e.id == id)
    }

    /// Text direction of an element: its own `dir`, else the deck's, else the first
    /// strong character of its body (FR-DIR-01).
    pub fn resolved_dir(&self, element: &Element) -> Dir {
        match (element.dir, self.front.dir) {
            (Some(d), _) if d != Dir::Auto => d,
            (_, d) if d != Dir::Auto => d,
            _ => first_strong(&element.body),
        }
    }

    pub(crate) fn build(src: &str, carry: &Carry) -> Deck {
        let mut cx = Ctx {
            src,
            index: LineIndex::new(src),
            diags: Vec::new(),
            canvas: Canvas::default(),
        };
        let mut deck = Deck {
            source: src.to_string(),
            version: frontmatter::FORMAT_VERSION,
            front: FrontMatter::default(),
            masters: Vec::new(),
            layouts: Vec::new(),
            slides: Vec::new(),
            line_ending: LineEnding::Lf,
            diagnostics: Vec::new(),
            read_only: false,
        };
        if src.len() > MAX_FILE_BYTES {
            cx.push(
                Severity::Error,
                "E-LIMIT",
                format!(
                    "The file is over {} MB. It opens read-only and empty.",
                    MAX_FILE_BYTES >> 20
                ),
                0,
                None,
                None,
            );
            deck.read_only = true;
            deck.diagnostics = cx.diags;
            return deck;
        }
        let bom = if src.starts_with('\u{feff}') { 3 } else { 0 };
        let lines = split_lines(src, bom);
        let crlf = lines.iter().filter(|l| l.next > l.end + 1).count();
        let lf = lines.iter().filter(|l| l.next == l.end + 1).count();
        if crlf > lf {
            deck.line_ending = LineEnding::Crlf;
        }

        // Front matter: `---` on the first line up to the next `---` line.
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
                    let text = &src[lines[0].next..close.start];
                    let parsed = frontmatter::parse(text, 2);
                    deck.front = parsed.front;
                    deck.version = parsed.version;
                    cx.diags.extend(parsed.diagnostics);
                    first = j + 2;
                }
                None => cx.push(
                    Severity::Error,
                    "E-FRONTMATTER",
                    "Front matter has no closing --- line. Reading the file without it.".into(),
                    bom,
                    None,
                    None,
                ),
            }
        }
        cx.canvas = deck.front.canvas;
        if deck.version > frontmatter::FORMAT_VERSION {
            deck.read_only = true;
        }

        // Split into sections at `---` lines outside code fences.
        let mut sections = Vec::new();
        let mut cur = Section {
            span: lines.get(first).map_or(src.len(), |l| l.start)..src.len(),
            directives: Vec::new(),
        };
        let mut fence: Option<(u8, usize, usize)> = None;
        for l in lines.iter().skip(first) {
            let text = &src[l.start..l.end];
            if let Some((c, n, _)) = fence {
                if let Some((c2, n2)) = fence_marker(text)
                    && c2 == c
                    && n2 >= n
                    && text.trim().bytes().all(|x| x == c)
                {
                    fence = None;
                }
                continue;
            }
            if let Some((c, n)) = fence_marker(text) {
                fence = Some((c, n, l.start));
                continue;
            }
            if is_separator(text) {
                cur.span.end = l.start;
                sections.push(std::mem::replace(
                    &mut cur,
                    Section {
                        span: l.next..src.len(),
                        directives: Vec::new(),
                    },
                ));
                continue;
            }
            if let Some(d) = parse_directive(text, l.start) {
                if let Some(at) = d.error_at {
                    cx.push(
                        Severity::Error,
                        "E-ATTR-SYNTAX",
                        "This attribute can't be read. Use key=value, key=\"quoted value\" or a bare key.".into(),
                        at,
                        None,
                        None,
                    );
                }
                cur.directives.push((l.start..l.end, l.next, d));
            }
        }
        sections.push(cur);
        if let Some((_, _, at)) = fence {
            cx.push(
                Severity::Error,
                "E-UNCLOSED-FENCE",
                "This code fence is never closed, so the rest of the file is inside it.".into(),
                at,
                None,
                None,
            );
        }

        // A file with no content after the front matter has no slides.
        if sections.len() == 1
            && sections[0].directives.is_empty()
            && src[sections[0].span.clone()].trim().is_empty()
        {
            sections.clear();
        }
        let mut slide_sections = 0;
        for sec in sections {
            let defs = sec
                .directives
                .first()
                .is_some_and(|(_, _, d)| d.kind == "master" || d.kind == "layout");
            if defs {
                build_definitions(&mut cx, &mut deck, sec);
            } else {
                if slide_sections == MAX_SLIDES {
                    cx.push(
                        Severity::Error,
                        "E-LIMIT",
                        format!("The deck has more than {MAX_SLIDES} slides. It opens read-only."),
                        sec.span.start,
                        None,
                        None,
                    );
                    deck.read_only = true;
                    break;
                }
                slide_sections += 1;
                let slide = build_slide(&mut cx, &mut deck, sec);
                deck.slides.push(slide);
            }
        }
        assign_ids(&mut cx, &mut deck, carry);
        deck.diagnostics = cx.diags;
        deck
    }

    /// In-memory ids of elements without a written id, keyed by start offset.
    pub(crate) fn carry(&self) -> Carry {
        self.masters
            .iter()
            .flat_map(|m| &m.elements)
            .chain(self.slides.iter().flat_map(|s| &s.elements))
            .filter(|e| !e.id_written())
            .map(|e| (e.src.start(), e.id.clone()))
            .collect()
    }
}

/// Body range after a directive: up to the next boundary, minus trailing blank lines.
fn body_ranges(src: &str, from: usize, to: usize) -> (Range<usize>, usize) {
    let from = from.min(to);
    (from..trim_blank_tail(src, from..to), to)
}

fn build_slide(cx: &mut Ctx, deck: &mut Deck, sec: Section) -> Slide {
    let src = cx.src;
    let index = deck.slides.len();
    let mut slide = Slide {
        index,
        layout: None,
        master: MasterSetting::Inherit,
        hide: Vec::new(),
        bg: None,
        transition: None,
        attrs: Vec::new(),
        elements: Vec::new(),
        notes: None,
        src: SlideSrc {
            span: sec.span.clone(),
            directive: None,
        },
    };
    let mut content_start = sec.span.start;
    let mut boundaries = Vec::new();
    for (i, (line, next, d)) in sec.directives.into_iter().enumerate() {
        match d.kind.as_str() {
            "slide" if i == 0 => {
                for a in &d.attrs {
                    let v = a.value.clone();
                    match a.key.as_str() {
                        "layout" => slide.layout = v,
                        "master" => {
                            slide.master = match v.as_deref() {
                                Some("none") => MasterSetting::None,
                                Some("inherit") | None => MasterSetting::Inherit,
                                Some(id) => MasterSetting::Id(id.to_string()),
                            }
                        }
                        "hide" => {
                            slide.hide = v
                                .unwrap_or_default()
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect()
                        }
                        "bg" => slide.bg = v,
                        "transition" => slide.transition = v,
                        _ => {}
                    }
                }
                slide.attrs = d.attrs;
                slide.src.directive = Some(line);
                content_start = next;
            }
            "notes" => boundaries.push((line, next, d)),
            k if is_element_kind(k) => boundaries.push((line, next, d)),
            other => cx.push(
                Severity::Warning,
                "W-MISPLACED",
                format!("A {other} directive does not belong in a slide here. It is read as text."),
                line.start,
                None,
                Some(index),
            ),
        }
    }

    // Markdown before the first element is an implicit full-slide text element.
    let first_boundary = boundaries.first().map_or(sec.span.end, |(l, _, _)| l.start);
    let pre_start = skip_blank_head(src, content_start..first_boundary);
    let pre_end = trim_blank_tail(src, pre_start..first_boundary);
    if pre_end > pre_start {
        let c = cx.canvas;
        let (mx, my) = (
            (c.width as f32 * 0.05).round(),
            (c.height as f32 * 0.05).round(),
        );
        slide.elements.push(Element {
            id: String::new(),
            kind: Kind::Text,
            x: mx,
            y: my,
            w: c.width as f32 - 2.0 * mx,
            h: c.height as f32 - 2.0 * my,
            rot: 0.0,
            opacity: 1.0,
            locked: false,
            name: None,
            dir: None,
            attrs: Vec::new(),
            body: src[pre_start..pre_end].to_string(),
            implicit: true,
            src: ElementSrc {
                line: None,
                insert_at: pre_start,
                body: pre_start..pre_end,
                end: first_boundary,
                written_id: None,
            },
        });
    }

    let mut notes_seen = false;
    for k in 0..boundaries.len() {
        let to = boundaries
            .get(k + 1)
            .map_or(sec.span.end, |(l, _, _)| l.start);
        let (line, next, d) = &boundaries[k];
        if d.kind == "notes" {
            let (body, _) = body_ranges(src, *next, to);
            let text = &src[body];
            if notes_seen {
                cx.push(
                    Severity::Warning,
                    "W-NOTES-ORDER",
                    "This slide already has notes. These are added to them.".into(),
                    line.start,
                    None,
                    Some(index),
                );
                let n = slide.notes.get_or_insert_with(String::new);
                n.push_str(deck.line_ending.as_str());
                n.push_str(text);
            } else {
                slide.notes = Some(text.to_string());
            }
            notes_seen = true;
            continue;
        }
        if notes_seen {
            cx.push(
                Severity::Warning,
                "W-NOTES-ORDER",
                "An element after the notes is read as an element. Move it above <!-- notes -->."
                    .into(),
                line.start,
                None,
                Some(index),
            );
        }
        if slide.elements.len() == MAX_ELEMENTS_PER_SLIDE {
            cx.push(
                Severity::Error,
                "E-LIMIT",
                format!("Slide {} has more than {MAX_ELEMENTS_PER_SLIDE} elements. The deck opens read-only.", index + 1),
                line.start,
                None,
                Some(index),
            );
            deck.read_only = true;
            break;
        }
        let e = build_element(cx, line.clone(), *next, to, d, Some(index));
        slide.elements.push(e);
    }
    slide
}

fn build_definitions(cx: &mut Ctx, deck: &mut Deck, sec: Section) {
    enum Open {
        Master(usize),
        Layout(usize),
    }
    let mut open: Option<Open> = None;
    let mut kept: Vec<(Range<usize>, usize, Directive)> = Vec::new();
    for (line, next, d) in sec.directives {
        let ok = match d.kind.as_str() {
            "master" | "layout" => true,
            "placeholder" => matches!(open, Some(Open::Layout(_))),
            k if is_element_kind(k) => matches!(open, Some(Open::Master(_))),
            _ => false,
        };
        if ok {
            if d.kind == "master" {
                open = Some(Open::Master(0));
            } else if d.kind == "layout" {
                open = Some(Open::Layout(0));
            }
            kept.push((line, next, d));
        } else {
            cx.push(
                Severity::Warning,
                "W-MISPLACED",
                format!(
                    "A {} directive does not belong here. It is read as text.",
                    d.kind
                ),
                line.start,
                None,
                None,
            );
        }
    }
    let mut open: Option<Open> = None;
    for k in 0..kept.len() {
        let to = kept.get(k + 1).map_or(sec.span.end, |(l, _, _)| l.start);
        let (line, next, d) = &kept[k];
        let id = || d.get("id").and_then(|a| a.value.clone());
        match d.kind.as_str() {
            "master" => {
                let id = id().unwrap_or_else(|| {
                    cx.push(
                        Severity::Error,
                        "E-ATTR-MISSING",
                        "A master needs an id.".into(),
                        line.start,
                        None,
                        None,
                    );
                    format!("master-{}", deck.masters.len() + 1)
                });
                if deck.masters.iter().any(|m| m.id == id) {
                    cx.push(
                        Severity::Error,
                        "E-DUP-ID",
                        format!("Master id {id} is used twice."),
                        line.start,
                        None,
                        None,
                    );
                }
                deck.masters.push(Master {
                    id,
                    attrs: d.attrs.clone(),
                    elements: Vec::new(),
                });
                open = Some(Open::Master(deck.masters.len() - 1));
            }
            "layout" => {
                let id = id().unwrap_or_else(|| {
                    cx.push(
                        Severity::Error,
                        "E-ATTR-MISSING",
                        "A layout needs an id.".into(),
                        line.start,
                        None,
                        None,
                    );
                    format!("layout-{}", deck.layouts.len() + 1)
                });
                if deck.layouts.iter().any(|l| l.id == id) {
                    cx.push(
                        Severity::Error,
                        "E-DUP-ID",
                        format!("Layout id {id} is used twice."),
                        line.start,
                        None,
                        None,
                    );
                }
                deck.layouts.push(Layout {
                    id,
                    master: d.get("master").and_then(|a| a.value.clone()),
                    attrs: d.attrs.clone(),
                    placeholders: Vec::new(),
                });
                open = Some(Open::Layout(deck.layouts.len() - 1));
            }
            "placeholder" => {
                let Some(Open::Layout(li)) = open else {
                    continue;
                };
                let g = geometry(cx, d, true, true, None, None);
                let id = id().unwrap_or_else(|| {
                    cx.push(
                        Severity::Error,
                        "E-ATTR-MISSING",
                        "A placeholder needs an id.".into(),
                        line.start,
                        None,
                        None,
                    );
                    format!("p{}", deck.layouts[li].placeholders.len() + 1)
                });
                if deck.layouts[li].placeholders.iter().any(|p| p.id == id) {
                    cx.push(
                        Severity::Error,
                        "E-DUP-ID",
                        format!("Placeholder id {id} is used twice in this layout."),
                        line.start,
                        None,
                        None,
                    );
                }
                deck.layouts[li].placeholders.push(Placeholder {
                    id,
                    kind: d
                        .get("kind")
                        .and_then(|a| a.value.clone())
                        .unwrap_or_else(|| "text".into()),
                    x: g[0],
                    y: g[1],
                    w: g[2],
                    h: g[3],
                    attrs: d.attrs.clone(),
                });
            }
            _ => {
                let Some(Open::Master(mi)) = open else {
                    continue;
                };
                let e = build_element(cx, line.clone(), *next, to, d, None);
                deck.masters[mi].elements.push(e);
            }
        }
    }
}

/// Read x, y, w, h with diagnostics. Missing values are 0.
fn geometry(
    cx: &mut Ctx,
    d: &Directive,
    need_h: bool,
    required: bool,
    element: Option<&str>,
    slide: Option<usize>,
) -> [f32; 4] {
    let mut g = [0.0f32; 4];
    for (i, key) in ["x", "y", "w", "h"].into_iter().enumerate() {
        match d.get(key) {
            None => {
                if required && (key != "h" || need_h) {
                    cx.push(
                        Severity::Error,
                        "E-ATTR-MISSING",
                        format!("The {} directive needs {key}=.", d.kind),
                        d.insert_at,
                        element,
                        slide,
                    );
                }
            }
            Some(a) => match a
                .value
                .as_deref()
                .and_then(|v| v.parse::<f32>().ok())
                .filter(|v| v.is_finite())
            {
                Some(v) if (key == "w" || key == "h") && v < 0.0 => cx.push(
                    Severity::Error,
                    "E-ATTR-VALUE",
                    format!("{key} must not be negative."),
                    a.value_span.as_ref().map_or(a.span.start, |s| s.start),
                    element,
                    slide,
                ),
                Some(v) => g[i] = v,
                None => cx.push(
                    Severity::Error,
                    "E-ATTR-VALUE",
                    format!(
                        "{key} must be a number, not {:?}.",
                        a.value.as_deref().unwrap_or("")
                    ),
                    a.value_span.as_ref().map_or(a.span.start, |s| s.start),
                    element,
                    slide,
                ),
            },
        }
    }
    g
}

fn build_element(
    cx: &mut Ctx,
    line: Range<usize>,
    next: usize,
    to: usize,
    d: &Directive,
    slide: Option<usize>,
) -> Element {
    let src = cx.src;
    let kind = Kind::from_name(&d.kind);
    let written_id = d.get("id").and_then(|a| a.value.clone());
    let label = written_id.clone();
    let label = label.as_deref();

    let mut seen = HashSet::new();
    for a in &d.attrs {
        if !seen.insert(a.key.as_str()) {
            cx.push(
                Severity::Warning,
                "W-DUP-ATTR",
                format!("{} is set more than once. The last value wins.", a.key),
                a.span.start,
                label,
                slide,
            );
        }
        if a.value
            .as_ref()
            .is_some_and(|v| v.len() > MAX_ATTR_VALUE_BYTES)
        {
            cx.push(
                Severity::Error,
                "E-LIMIT",
                format!("The value of {} is over 4 KB.", a.key),
                a.span.start,
                label,
                slide,
            );
        }
    }
    if let Some(a) = d.get("id")
        && !a.value.as_deref().is_some_and(valid_id)
    {
        cx.push(
            Severity::Error,
            "E-ID-FORMAT",
            "An id is 1 to 32 characters of a-z, 0-9 and -.".into(),
            a.span.start,
            label,
            slide,
        );
    }
    let known = !matches!(kind, Kind::Unknown(_));
    let shape_line = kind == Kind::Shape
        && matches!(
            d.get("kind").and_then(|a| a.value.as_deref()),
            Some("line" | "arrow")
        );
    let g = geometry(
        cx,
        d,
        !shape_line,
        known && kind != Kind::Group,
        label,
        slide,
    );

    let mut num = |key: &str, default: f32, range: Option<(f32, f32)>| -> f32 {
        let Some(a) = d.get(key) else { return default };
        match a
            .value
            .as_deref()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite())
        {
            Some(v) if range.is_none_or(|(lo, hi)| (lo..=hi).contains(&v)) => v,
            _ => {
                let what = range.map_or("a number".to_string(), |(lo, hi)| {
                    format!("a number from {lo} to {hi}")
                });
                cx.push(
                    Severity::Error,
                    "E-ATTR-VALUE",
                    format!("{key} must be {what}."),
                    a.value_span.as_ref().map_or(a.span.start, |s| s.start),
                    label,
                    slide,
                );
                default
            }
        }
    };
    let rot = num("rot", 0.0, None);
    let opacity = num("opacity", 1.0, Some((0.0, 1.0)));
    let dir = match d.get("dir") {
        None => None,
        Some(a) => match a.value.as_deref().and_then(Dir::parse) {
            Some(v) => Some(v),
            None => {
                cx.push(
                    Severity::Error,
                    "E-ATTR-VALUE",
                    "dir must be auto, ltr or rtl.".into(),
                    a.span.start,
                    label,
                    slide,
                );
                None
            }
        },
    };
    let locked = d
        .get("locked")
        .is_some_and(|a| a.value.as_deref() != Some("false"));

    let c = cx.canvas;
    if known
        && kind != Kind::Group
        && (g[0] < 0.0
            || g[1] < 0.0
            || g[0] + g[2] > c.width as f32
            || g[1] + g[3] > c.height as f32)
    {
        cx.push(
            Severity::Warning,
            "W-OFF-CANVAS",
            format!(
                "This element reaches outside the {}x{} canvas.",
                c.width, c.height
            ),
            line.start,
            label,
            slide,
        );
    }

    let (body, end) = body_ranges(src, next, to);
    Element {
        id: written_id.clone().unwrap_or_default(),
        kind,
        x: g[0],
        y: g[1],
        w: g[2],
        h: g[3],
        rot,
        opacity,
        locked,
        name: d.get("name").and_then(|a| a.value.clone()),
        dir,
        attrs: d.attrs.clone(),
        body: src[body.clone()].to_string(),
        implicit: false,
        src: ElementSrc {
            line: Some(line),
            insert_at: d.insert_at,
            body,
            end,
            written_id,
        },
    }
}

/// Give every element a deck-unique id. Written ids win; duplicates and elements without
/// one get an in-memory id (kind letter plus counter), reused from `carry` when possible.
fn assign_ids(cx: &mut Ctx, deck: &mut Deck, carry: &Carry) {
    let mut used: HashSet<String> = HashSet::new();
    let mut needs: Vec<usize> = Vec::new();
    let mut all: Vec<&mut Element> = deck
        .masters
        .iter_mut()
        .flat_map(|m| m.elements.iter_mut())
        .chain(deck.slides.iter_mut().flat_map(|s| s.elements.iter_mut()))
        .collect();
    for (i, e) in all.iter_mut().enumerate() {
        match &e.src.written_id {
            Some(id) if valid_id(id) && !used.contains(id) => {
                used.insert(id.clone());
            }
            Some(id) => {
                if valid_id(id) {
                    let (line, column) = cx.index.pos(cx.src, e.src.start());
                    cx.diags.push(Diagnostic {
                        severity: Severity::Error,
                        code: "E-DUP-ID",
                        message: format!("Id {id} is already used. This element gets a new id when it is next edited."),
                        line,
                        column,
                        element: Some(id.clone()),
                        slide: None,
                    });
                }
                needs.push(i);
            }
            None => needs.push(i),
        }
    }
    // Carry first, so ids survive edits elsewhere in the file.
    let mut pending = Vec::new();
    for i in needs {
        let e = &mut all[i];
        match carry.get(&e.src.start()) {
            Some(id) if !used.contains(id) => {
                used.insert(id.clone());
                e.id = id.clone();
            }
            _ => pending.push(i),
        }
    }
    let mut counters: HashMap<char, u32> = HashMap::new();
    for i in pending {
        let e = &mut all[i];
        let letter = e.kind.name().chars().next().unwrap_or('e');
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

/// First strong character decides: Hebrew, Arabic and related blocks are rtl, other
/// letters ltr. ponytail: block ranges, not the full Unicode bidi class table; swap in
/// a bidi-class lookup if a script outside these blocks needs it.
pub fn first_strong(text: &str) -> Dir {
    for c in text.chars() {
        let u = c as u32;
        let rtl = matches!(u, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF);
        if rtl {
            return Dir::Rtl;
        }
        if c.is_alphabetic() {
            return Dir::Ltr;
        }
    }
    Dir::Ltr
}
