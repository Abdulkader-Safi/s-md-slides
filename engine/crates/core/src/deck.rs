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

/// Files larger than this many bytes open read-only.
pub const MAX_FILE_BYTES: usize = 20 * 1024 * 1024;
/// Decks with more slides than this open read-only.
pub const MAX_SLIDES: usize = 5_000;
/// Slides with more elements than this open read-only.
pub const MAX_ELEMENTS_PER_SLIDE: usize = 2_000;
/// Attribute values longer than this many bytes are an `E-LIMIT` error.
pub const MAX_ATTR_VALUE_BYTES: usize = 4 * 1024;

/// Line ending style. New lines use the file's most common one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum LineEnding {
    /// `\n`.
    Lf,
    /// `\r\n`.
    Crlf,
}

impl LineEnding {
    /// The line ending as text.
    pub fn as_str(self) -> &'static str {
        match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        }
    }
}

/// Element kind, from the directive's first word.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, type = "string"))]
pub enum Kind {
    /// Markdown text box.
    Text,
    /// One markdown image.
    Image,
    /// Rectangle, ellipse, line or arrow.
    Shape,
    /// One fenced code block.
    Code,
    /// One markdown table.
    Table,
    /// One Mermaid diagram.
    Diagram,
    /// One LaTeX block.
    Math,
    /// Video file, with an optional poster.
    Video,
    /// An https page in an iframe.
    Embed,
    /// A set of elements on the same slide that move together.
    Group,
    /// A kind the engine does not know, kept because the directive has an `id=`.
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
    /// The kind as written in a directive, such as `text`.
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

/// One box on a slide or master: its directive plus its markdown body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Element {
    /// Element id, unique in the deck. Written in the file or assigned in memory until the element is first edited.
    pub id: String,
    /// What the element is.
    pub kind: Kind,
    /// Left edge in canvas units.
    pub x: f64,
    /// Top edge in canvas units.
    pub y: f64,
    /// Width in canvas units.
    pub w: f64,
    /// Height in canvas units. 0 for a horizontal line or arrow written without `h`.
    pub h: f64,
    /// Rotation in degrees. 0 when not written.
    pub rot: f64,
    /// Opacity from 0 to 1. 1 when not written.
    pub opacity: f64,
    /// True when `locked` is set, so editors should not move the element.
    pub locked: bool,
    /// Display name from `name=`, for editors' layer lists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Text direction from `dir=`. `None` follows the deck; see [`Deck::resolved_dir`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<Dir>,
    /// Every attribute as written, in order (known ones included).
    pub attrs: Vec<Attr>,
    /// Style attributes as written (literal values or theme roles).
    #[serde(skip_serializing_if = "crate::kinds::Style::is_empty")]
    pub style: crate::kinds::Style,
    /// What the engine reads from kind-specific attributes and the body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<crate::kinds::Content>,
    /// Entrance and exit animation.
    #[serde(skip_serializing_if = "crate::kinds::Anim::is_static")]
    pub anim: crate::kinds::Anim,
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

/// Which master a slide uses, from `master=` on its slide directive.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(into = "String")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, type = "string"))]
pub enum MasterSetting {
    /// No `master=` (or `master=inherit`): the layout's master, else the deck's first master.
    Inherit,
    /// `master=none`: no master is drawn.
    None,
    /// A master id.
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
    /// Where slide content starts (after the slide directive line, if any).
    pub content_start: usize,
    /// The notes area: from the first `<!-- notes -->` line to the end of its body.
    pub notes: Option<NotesSrc>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct NotesSrc {
    /// Start of the first notes directive line.
    pub start: usize,
    /// Body of the first notes directive, without trailing blank lines.
    pub body: Range<usize>,
    /// How many notes directives the slide has.
    pub count: usize,
    /// End of the notes block (next directive or end of the slide).
    pub end: usize,
}

/// One `---`-separated section in file order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SectionInfo {
    pub span: Range<usize>,
    /// Slide index, or `None` for a definitions section.
    pub slide: Option<usize>,
}

/// Where a master or layout block sits: its directive line to the next definition.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DefSrc {
    pub block: Range<usize>,
    pub section: usize,
}

/// One slide: its directive settings, elements and notes.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Slide {
    /// 0-based position in the deck.
    pub index: usize,
    /// Layout id from `layout=`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<String>,
    /// Which master the slide draws behind its elements.
    pub master: MasterSetting,
    /// Ids of master elements this slide hides, from `hide=`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
    /// Background as written in `bg=`: a color, a role, a `linear(...)` gradient or an image path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bg: Option<String>,
    /// Transition from `transition=`: `none`, `fade` or `push`. `None` uses the deck default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,
    /// Transition length in milliseconds (`dur=` on the slide directive).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition_dur: Option<u32>,
    /// Attributes of the `<!-- slide -->` directive as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attrs: Vec<Attr>,
    /// File order is stacking order.
    pub elements: Vec<Element>,
    /// Speaker notes as raw markdown. `None` when the slide has no `<!-- notes -->`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip)]
    pub(crate) src: SlideSrc,
}

/// A master: elements drawn behind every slide that uses it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Master {
    /// Master id from `id=`.
    pub id: String,
    /// Attributes of the `<!-- master -->` directive as written.
    pub attrs: Vec<Attr>,
    /// Master elements in stacking order. Their text can use `{slide}`, `{total}` and `{title}`.
    pub elements: Vec<Element>,
    #[serde(skip)]
    pub(crate) src: DefSrc,
}

/// A named position in a layout. A slide added from the layout gets one element per placeholder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Placeholder {
    /// Placeholder id, unique within its layout.
    pub id: String,
    /// Kind of element the placeholder makes, from `kind=`. `text` when not written.
    pub kind: String,
    /// Left edge in canvas units.
    pub x: f64,
    /// Top edge in canvas units.
    pub y: f64,
    /// Width in canvas units.
    pub w: f64,
    /// Height in canvas units.
    pub h: f64,
    /// Attributes of the `<!-- placeholder -->` directive as written, style included.
    pub attrs: Vec<Attr>,
}

/// A layout: placeholder positions on a master.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Layout {
    /// Layout id from `id=`.
    pub id: String,
    /// Master id from `master=`. `None` uses the deck's first master.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master: Option<String>,
    /// Attributes of the `<!-- layout -->` directive as written.
    pub attrs: Vec<Attr>,
    /// Placeholders in file order.
    pub placeholders: Vec<Placeholder>,
    #[serde(skip)]
    pub(crate) src: DefSrc,
}

/// A parsed deck. It keeps the source text, and every edit splices that text.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Deck {
    #[serde(skip)]
    pub(crate) source: String,
    /// Format version from the `s-md-slides` front matter key. Above [`frontmatter::FORMAT_VERSION`] the deck is read-only.
    pub version: u32,
    /// Front matter as read.
    pub front: FrontMatter,
    /// Masters in file order.
    pub masters: Vec<Master>,
    /// Layouts in file order.
    pub layouts: Vec<Layout>,
    /// Slides in file order.
    pub slides: Vec<Slide>,
    /// The file's most common line ending, used for new lines.
    pub line_ending: LineEnding,
    #[serde(skip)]
    pub(crate) diagnostics: Vec<Diagnostic>,
    #[serde(skip)]
    pub(crate) read_only: bool,
    /// Front matter content between the `---` lines.
    #[serde(skip)]
    pub(crate) front_span: Option<Range<usize>>,
    #[serde(skip)]
    pub(crate) sections: Vec<SectionInfo>,
    /// 3 when the file starts with a BOM, else 0.
    #[serde(skip)]
    pub(crate) bom: usize,
    #[serde(skip)]
    pub(crate) theme: crate::theme::Theme,
    /// Path of the theme file loaded through `load_theme`, if any.
    #[serde(skip)]
    pub(crate) theme_path: Option<String>,
    /// Where each phase's diagnostics end, so a one-line edit can replace just its
    /// section's diagnostics in the same order a full parse gives.
    #[serde(skip)]
    pub(crate) marks: DiagMarks,
}

/// Diagnostic counts after each parse phase. A full parse emits, in order: front
/// matter, then the syntax problems of every section, then an unclosed fence, then
/// each section's build problems, then id and reference problems (the tail).
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct DiagMarks {
    /// After front matter.
    head: usize,
    /// After each section's syntax problems.
    syntax: Vec<usize>,
    /// Where section building starts (after the unclosed-fence problem, if any).
    built: usize,
    /// After each section's build problems.
    build: Vec<usize>,
}

/// Result of [`Deck::parse`]: always a deck, plus whatever went wrong.
#[derive(Debug, Clone)]
pub struct Parsed {
    /// The deck, read as far as it could be.
    pub deck: Deck,
    /// Problems found, with line and column.
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

/// Split `lines` (which cover `span`) into sections at `---` lines outside code
/// fences, parsing every directive line. Returns the sections, the start of a fence
/// that never closes, and the diagnostic count after each section's syntax problems.
fn scan(
    cx: &mut Ctx,
    lines: &[Line],
    span: Range<usize>,
) -> (Vec<Section>, Option<usize>, Vec<usize>) {
    let src = cx.src;
    let end = span.end;
    let mut sections = Vec::new();
    let mut syntax = Vec::new();
    let mut cur = Section {
        span,
        directives: Vec::new(),
    };
    let mut fence: Option<(u8, usize, usize)> = None;
    for l in lines {
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
                    span: l.next..end,
                    directives: Vec::new(),
                },
            ));
            syntax.push(cx.diags.len());
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
    syntax.push(cx.diags.len());
    (sections, fence.map(|(_, _, at)| at), syntax)
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

    /// Canvas size from front matter.
    pub fn canvas(&self) -> Canvas {
        self.front.canvas
    }

    /// Slide at a 0-based index.
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

    /// The element with this id, on a slide or a master.
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
            front_span: None,
            sections: Vec::new(),
            bom: 0,
            theme: Default::default(),
            theme_path: None,
            marks: DiagMarks::default(),
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
        deck.bom = bom;
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
                    let base = lines[0].next;
                    deck.front_span = Some(base..close.start);
                    let parsed = frontmatter::parse(text);
                    deck.front = parsed.front;
                    deck.version = parsed.version;
                    let (theme, problems) = crate::theme::inline_theme(deck.front.theme.as_ref());
                    deck.theme = theme;
                    for p in problems {
                        let at = text.find("theme").map_or(base, |o| base + o);
                        cx.push(Severity::Warning, "W-THEME", p, at, None, None);
                    }
                    for i in parsed.issues {
                        cx.push(
                            Severity::Error,
                            i.code,
                            i.message,
                            base + i.offset,
                            None,
                            None,
                        );
                    }
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
        deck.marks.head = cx.diags.len();
        let start = lines.get(first).map_or(src.len(), |l| l.start);
        let (mut sections, fence, syntax) =
            scan(&mut cx, &lines[first.min(lines.len())..], start..src.len());
        deck.marks.syntax = syntax;
        if let Some(at) = fence {
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
            deck.marks.syntax.clear();
        }
        deck.marks.built = cx.diags.len();
        let mut slide_sections = 0;
        for sec in sections {
            let defs = sec
                .directives
                .first()
                .is_some_and(|(_, _, d)| d.kind == "master" || d.kind == "layout");
            if defs {
                deck.sections.push(SectionInfo {
                    span: sec.span.clone(),
                    slide: None,
                });
                build_definitions(&mut cx, &mut deck, sec);
                deck.marks.build.push(cx.diags.len());
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
                deck.sections.push(SectionInfo {
                    span: sec.span.clone(),
                    slide: Some(deck.slides.len()),
                });
                let index = deck.slides.len();
                let slide = build_slide(&mut cx, &mut deck, sec, index);
                deck.slides.push(slide);
                deck.marks.build.push(cx.diags.len());
            }
        }
        assign_ids(&mut cx, &mut deck, carry);
        check_refs(&mut cx, &deck);
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

fn build_slide(cx: &mut Ctx, deck: &mut Deck, sec: Section, index: usize) -> Slide {
    let src = cx.src;
    let mut slide = Slide {
        index,
        layout: None,
        master: MasterSetting::Inherit,
        hide: Vec::new(),
        bg: None,
        transition: None,
        transition_dur: None,
        attrs: Vec::new(),
        elements: Vec::new(),
        notes: None,
        src: SlideSrc {
            span: sec.span.clone(),
            directive: None,
            content_start: sec.span.start,
            notes: None,
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
                        "bg" => {
                            if let Some(b) = &v
                                && !crate::theme::valid_background(b)
                            {
                                cx.push(
                                        Severity::Error,
                                        "E-ATTR-VALUE",
                                        "bg must be a color, a theme role, linear(angle,color,color) or an image path.".into(),
                                        a.value_span.as_ref().map_or(a.span.start, |r| r.start),
                                        None,
                                        Some(index),
                                    );
                            }
                            slide.bg = v
                        }
                        "transition" => {
                            if !matches!(v.as_deref(), Some("none" | "fade" | "push")) {
                                cx.push(
                                    Severity::Error,
                                    "E-ATTR-VALUE",
                                    "transition must be none, fade or push.".into(),
                                    a.span.start,
                                    None,
                                    Some(index),
                                );
                            }
                            slide.transition = v
                        }
                        "dur" => match v.as_deref().and_then(|d| d.parse::<u32>().ok()) {
                            Some(d) if d <= 60_000 => slide.transition_dur = Some(d),
                            _ => cx.push(
                                Severity::Error,
                                "E-ATTR-VALUE",
                                "dur must be milliseconds as a whole number, up to 60000.".into(),
                                a.span.start,
                                None,
                                Some(index),
                            ),
                        },
                        _ => {}
                    }
                }
                slide.attrs = d.attrs;
                slide.src.directive = Some(line);
                content_start = next;
                slide.src.content_start = next;
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
            (c.width as f64 * 0.05).round(),
            (c.height as f64 * 0.05).round(),
        );
        slide.elements.push(Element {
            id: String::new(),
            kind: Kind::Text,
            x: mx,
            y: my,
            w: c.width as f64 - 2.0 * mx,
            h: c.height as f64 - 2.0 * my,
            rot: 0.0,
            opacity: 1.0,
            locked: false,
            name: None,
            dir: None,
            attrs: Vec::new(),
            style: Default::default(),
            content: None,
            anim: Default::default(),
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
    let ends: Vec<usize> = boundaries
        .iter()
        .skip(1)
        .map(|(l, _, _)| l.start)
        .chain([sec.span.end])
        .collect();
    for ((line, next, d), to) in boundaries.into_iter().zip(ends) {
        if d.kind == "notes" {
            let (body, _) = body_ranges(src, next, to);
            let text = &src[body.clone()];
            match &mut slide.src.notes {
                Some(n) => n.count += 1,
                None => {
                    slide.src.notes = Some(NotesSrc {
                        start: line.start,
                        body,
                        count: 1,
                        end: to,
                    })
                }
            }
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
        let e = build_element(cx, line, next, to, d, Some(index));
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
    let section = deck.sections.len().saturating_sub(1);
    for k in 0..kept.len() {
        let to = kept.get(k + 1).map_or(sec.span.end, |(l, _, _)| l.start);
        let block_end = kept[k + 1..]
            .iter()
            .find(|(_, _, d)| d.kind == "master" || d.kind == "layout")
            .map_or(sec.span.end, |(l, _, _)| l.start);
        let def_src = DefSrc {
            block: kept[k].0.start..block_end,
            section,
        };
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
                    src: def_src,
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
                    src: def_src,
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
                let e = build_element(cx, line.clone(), *next, to, d.clone(), None);
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
) -> [f64; 4] {
    let mut g = [0.0f64; 4];
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
                .and_then(|v| v.parse::<f64>().ok())
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
    d: Directive,
    slide: Option<usize>,
) -> Element {
    let src = cx.src;
    let kind = Kind::from_name(&d.kind);
    let written_id = d.get("id").and_then(|a| a.value.clone());
    let label = written_id.clone();
    let label = label.as_deref();

    // Elements have about ten attributes: a scan beats hashing each key.
    for (i, a) in d.attrs.iter().enumerate() {
        if d.attrs[..i].iter().any(|b| b.key == a.key) {
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
        &d,
        !shape_line,
        known && kind != Kind::Group,
        label,
        slide,
    );

    let mut num = |key: &str, default: f64, range: Option<(f64, f64)>| -> f64 {
        let Some(a) = d.get(key) else { return default };
        match a
            .value
            .as_deref()
            .and_then(|v| v.parse::<f64>().ok())
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
            || g[0] + g[2] > c.width as f64
            || g[1] + g[3] > c.height as f64)
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
    let (style, content, anim) = crate::kinds::check(
        &kind,
        &d,
        &src[body.clone()],
        body.start,
        &mut |sev, code, msg, at| cx.push(sev, code, msg, at, label, slide),
    );
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
        attrs: d.attrs,
        style,
        content,
        anim,
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
    let mut needs: Vec<usize> = Vec::new();
    let mut all: Vec<&mut Element> = deck
        .masters
        .iter_mut()
        .flat_map(|m| m.elements.iter_mut())
        .chain(deck.slides.iter_mut().flat_map(|s| s.elements.iter_mut()))
        .collect();
    let mut used: HashSet<String> = HashSet::with_capacity(all.len());
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

/// References between parts: layouts, masters, hidden master elements, group members.
fn check_refs(cx: &mut Ctx, deck: &Deck) {
    let master_ids: HashSet<&str> = deck.masters.iter().map(|m| m.id.as_str()).collect();
    for l in &deck.layouts {
        if let Some(m) = &l.master
            && !master_ids.contains(m.as_str())
        {
            cx.push(
                Severity::Error,
                "E-UNKNOWN-MASTER",
                format!("Layout {} uses master {m}, which doesn't exist.", l.id),
                l.src.block.start,
                None,
                None,
            );
        }
    }
    for s in &deck.slides {
        let at = s
            .src
            .directive
            .as_ref()
            .map_or(s.src.span.start, |r| r.start);
        if let Some(l) = &s.layout
            && !deck.layouts.iter().any(|x| &x.id == l)
        {
            cx.push(
                Severity::Error,
                "E-UNKNOWN-LAYOUT",
                format!("There is no layout {l}."),
                at,
                None,
                Some(s.index),
            );
        }
        if let MasterSetting::Id(m) = &s.master
            && !master_ids.contains(m.as_str())
        {
            cx.push(
                Severity::Error,
                "E-UNKNOWN-MASTER",
                format!("There is no master {m}."),
                at,
                None,
                Some(s.index),
            );
        }
        let master = deck.slide_master(s.index);
        for h in s.hide.iter().filter(|_| master.is_some()) {
            if !master.is_some_and(|m| m.elements.iter().any(|e| &e.id == h)) {
                cx.push(
                    Severity::Warning,
                    "W-HIDE-UNKNOWN",
                    format!("hide names {h}, which is not an element of this slide's master."),
                    at,
                    None,
                    Some(s.index),
                );
            }
        }
        for g in s.elements.iter().filter(|e| e.kind == Kind::Group) {
            if let Some(crate::kinds::Content::Group { items }) = &g.content {
                for i in items {
                    match s.elements.iter().find(|e| &e.id == i) {
                        None => cx.push(
                            Severity::Error,
                            "E-GROUP",
                            format!("Group {} lists {i}, which is not on this slide.", g.id),
                            g.src.start(),
                            Some(&g.id),
                            Some(s.index),
                        ),
                        Some(e) if e.kind == Kind::Group => cx.push(
                            Severity::Error,
                            "E-GROUP",
                            "Groups can't be nested.".into(),
                            g.src.start(),
                            Some(&g.id),
                            Some(s.index),
                        ),
                        Some(_) => {}
                    }
                }
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

/// Move a byte offset by `d` (an edit earlier in the file changed its length).
fn sh(x: &mut usize, d: isize) {
    *x = x.wrapping_add_signed(d);
}

fn sh_range(r: &mut Range<usize>, d: isize) {
    sh(&mut r.start, d);
    sh(&mut r.end, d);
}

fn sh_attrs(attrs: &mut [Attr], d: isize) {
    for a in attrs {
        sh_range(&mut a.span, d);
        if let Some(v) = &mut a.value_span {
            sh_range(v, d);
        }
    }
}

fn sh_elements(elements: &mut [Element], d: isize) {
    for e in elements {
        sh_attrs(&mut e.attrs, d);
        if let Some(l) = &mut e.src.line {
            sh_range(l, d);
        }
        sh(&mut e.src.insert_at, d);
        sh_range(&mut e.src.body, d);
        sh(&mut e.src.end, d);
    }
}

fn sh_slide(s: &mut Slide, d: isize) {
    sh_attrs(&mut s.attrs, d);
    sh_elements(&mut s.elements, d);
    sh_range(&mut s.src.span, d);
    if let Some(r) = &mut s.src.directive {
        sh_range(r, d);
    }
    sh(&mut s.src.content_start, d);
    if let Some(n) = &mut s.src.notes {
        sh(&mut n.start, d);
        sh_range(&mut n.body, d);
        sh(&mut n.end, d);
    }
}

impl Deck {
    /// The deck after replacing `range` with `text` (giving the source `next`), built
    /// by re-parsing only the edited slide when the edit stays inside one line of a
    /// slide and cannot change where sections or code fences are. Everything else is
    /// kept and shifted. Returns `None` when a full parse is needed. The result equals
    /// `Deck::build(next, carry)`; debug builds check that on every edit.
    pub(crate) fn rebuild_line(
        &self,
        range: &Range<usize>,
        text: &str,
        next: &str,
        carry: &Carry,
    ) -> Option<Deck> {
        let old = &self.source[range.clone()];
        let newline = |t: &str| t.contains(['\n', '\r']);
        // The first three bytes can make or break a byte order mark, which moves where
        // the first section starts.
        if self.read_only
            || range.start < 3
            || newline(old)
            || newline(text)
            || self.sections.len() < 2
        {
            return None;
        }
        if self.marks.syntax.len() != self.sections.len()
            || self.marks.build.len() != self.sections.len()
        {
            return None;
        }
        let k = self
            .sections
            .iter()
            .position(|s| s.span.start <= range.start && range.end <= s.span.end)?;
        let slide = self.sections[k].slide?;
        // The edited line, before and after, must not be a separator or a fence line.
        let line_of = |src: &str, at: usize, end: usize| {
            let start = src[..at].rfind('\n').map_or(0, |i| i + 1);
            let stop = src[end..].find('\n').map_or(src.len(), |i| end + i);
            src[start..stop].trim_end_matches('\r').to_string()
        };
        let delta = text.len() as isize - old.len() as isize;
        let after = range.end.wrapping_add_signed(delta);
        for line in [
            line_of(&self.source, range.start, range.end),
            line_of(next, range.start, after),
        ] {
            if is_separator(&line) || fence_marker(&line).is_some() {
                return None;
            }
        }

        let span =
            self.sections[k].span.start..self.sections[k].span.end.wrapping_add_signed(delta);
        let mut cx = Ctx {
            src: next,
            index: LineIndex::new(next),
            diags: Vec::new(),
            canvas: self.front.canvas,
        };
        let lines = split_lines(&next[..span.end], span.start);
        let (mut secs, fence, _) = scan(&mut cx, &lines, span.clone());
        if secs.len() != 1 || fence.is_some() {
            return None;
        }
        let sec = secs.pop()?;
        if sec
            .directives
            .first()
            .is_some_and(|(_, _, d)| d.kind == "master" || d.kind == "layout")
        {
            return None;
        }
        let syntax = std::mem::take(&mut cx.diags);

        let mut deck = self.clone();
        deck.source = next.to_string();
        let built = build_slide(&mut cx, &mut deck, sec, slide);
        if deck.read_only {
            return None;
        }
        let build = std::mem::take(&mut cx.diags);
        deck.slides[slide] = built;
        deck.sections[k].span = span;
        for j in k + 1..deck.sections.len() {
            sh_range(&mut deck.sections[j].span, delta);
            match deck.sections[j].slide {
                Some(s) => sh_slide(&mut deck.slides[s], delta),
                None => {
                    for m in deck.masters.iter_mut().filter(|m| m.src.section == j) {
                        sh_attrs(&mut m.attrs, delta);
                        sh_elements(&mut m.elements, delta);
                        sh_range(&mut m.src.block, delta);
                    }
                    for l in deck.layouts.iter_mut().filter(|l| l.src.section == j) {
                        sh_attrs(&mut l.attrs, delta);
                        for p in &mut l.placeholders {
                            sh_attrs(&mut p.attrs, delta);
                        }
                        sh_range(&mut l.src.block, delta);
                    }
                }
            }
        }

        // Diagnostics: keep everything outside this section, in full-parse order.
        let m = &self.marks;
        let old_diags = &self.diagnostics;
        let s0 = if k == 0 { m.head } else { m.syntax[k - 1] };
        let b0 = if k == 0 { m.built } else { m.build[k - 1] };
        let tail = *m.build.last()?;
        let mut diags = Vec::with_capacity(old_diags.len());
        diags.extend_from_slice(&old_diags[..s0]);
        diags.extend(syntax.iter().cloned());
        diags.extend_from_slice(&old_diags[m.syntax[k]..b0]);
        diags.extend(build.iter().cloned());
        diags.extend_from_slice(&old_diags[m.build[k]..tail]);
        let ds = syntax.len() as isize - (m.syntax[k] - s0) as isize;
        let db = build.len() as isize - (m.build[k] - b0) as isize;
        for x in deck.marks.syntax.iter_mut().skip(k) {
            sh(x, ds);
        }
        sh(&mut deck.marks.built, ds);
        // The whole build phase comes after every section's syntax problems, so all
        // build marks move by the syntax change; this section's and later ones also
        // move by the build change.
        for (j, x) in deck.marks.build.iter_mut().enumerate() {
            sh(x, if j < k { ds } else { ds + db });
        }
        cx.diags = diags;
        assign_ids(&mut cx, &mut deck, carry);
        check_refs(&mut cx, &deck);
        deck.diagnostics = cx.diags;
        Some(deck)
    }
}
