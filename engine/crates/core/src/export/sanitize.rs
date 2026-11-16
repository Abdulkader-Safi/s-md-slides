//! Markdown to safe HTML (FR-EXP-07). Raw HTML goes through an allowlist: no
//! `<script>`, no `on*` handlers, no `javascript:` URLs, no iframes. Every removal is
//! reported once per element.

use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};

/// Tags kept from raw HTML (with their allowed attributes).
const ALLOWED: &[&str] = &[
    "a",
    "abbr",
    "b",
    "bdi",
    "bdo",
    "blockquote",
    "br",
    "caption",
    "cite",
    "code",
    "col",
    "colgroup",
    "dd",
    "del",
    "details",
    "dfn",
    "div",
    "dl",
    "dt",
    "em",
    "figcaption",
    "figure",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hr",
    "i",
    "img",
    "ins",
    "kbd",
    "li",
    "mark",
    "ol",
    "p",
    "pre",
    "q",
    "rp",
    "rt",
    "ruby",
    "s",
    "samp",
    "small",
    "span",
    "strong",
    "sub",
    "summary",
    "sup",
    "table",
    "tbody",
    "td",
    "tfoot",
    "th",
    "thead",
    "time",
    "tr",
    "u",
    "ul",
    "var",
    "wbr",
];

/// Tags dropped together with everything inside them, with a warning.
const DROP_WITH_CONTENT: &[&str] = &[
    "script", "style", "iframe", "object", "embed", "noscript", "template", "textarea", "title",
    "frame", "frameset", "applet", "form", "select", "svg", "math",
];

/// Tags dropped alone (their content stays), with a warning.
const DROP_WARN: &[&str] = &[
    "input", "button", "link", "meta", "base", "video", "audio", "source",
];

const VOID: &[&str] = &[
    "br", "hr", "img", "wbr", "col", "input", "link", "meta", "base", "source",
];

/// Named entities raw HTML may use. Anything else becomes literal text, so the output
/// is also valid XHTML for SVG `foreignObject`.
const ENTITIES: &[(&str, &str)] = &[
    ("amp", "&amp;"),
    ("lt", "&lt;"),
    ("gt", "&gt;"),
    ("quot", "&quot;"),
    ("apos", "&#39;"),
    ("nbsp", "&#160;"),
    ("copy", "&#169;"),
    ("reg", "&#174;"),
    ("trade", "&#8482;"),
    ("mdash", "&#8212;"),
    ("ndash", "&#8211;"),
    ("hellip", "&#8230;"),
    ("laquo", "&#171;"),
    ("raquo", "&#187;"),
    ("lsquo", "&#8216;"),
    ("rsquo", "&#8217;"),
    ("ldquo", "&#8220;"),
    ("rdquo", "&#8221;"),
    ("times", "&#215;"),
    ("divide", "&#247;"),
    ("deg", "&#176;"),
    ("middot", "&#183;"),
    ("bull", "&#8226;"),
    ("euro", "&#8364;"),
    ("pound", "&#163;"),
];

pub(crate) fn esc_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub(crate) fn esc_attr(s: &str) -> String {
    esc_text(s).replace('"', "&quot;").replace('\'', "&#39;")
}

/// True for URLs that can't run script: http(s), mailto, tel, data images, relative.
pub(crate) fn safe_url(u: &str) -> bool {
    let t: String = u
        .trim()
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    let lower = t.to_ascii_lowercase();
    match lower.split_once(':') {
        None => true,
        Some((scheme, _))
            if scheme.contains('/') || scheme.contains('?') || scheme.contains('#') =>
        {
            true
        }
        Some((scheme, rest)) => {
            matches!(scheme, "http" | "https" | "mailto" | "tel")
                || (scheme == "data"
                    && rest.starts_with("image/")
                    && !rest.starts_with("image/svg"))
        }
    }
}

/// Sanitizes raw HTML fragments. Keeps state across fragments, since the markdown
/// parser splits `<script>alert(1)</script>` into tag events and text events.
#[derive(Default)]
pub(crate) struct Sanitizer {
    /// Open tags whose content is being dropped.
    skipping: Vec<String>,
    pub removed: Vec<String>,
}

impl Sanitizer {
    fn note(&mut self, what: String) {
        if !self.removed.contains(&what) {
            self.removed.push(what);
        }
    }

    pub(crate) fn skipping(&self) -> bool {
        !self.skipping.is_empty()
    }

    /// One raw HTML fragment to safe HTML.
    pub(crate) fn fragment(
        &mut self,
        html: &str,
        url: &mut dyn FnMut(&str) -> Option<String>,
    ) -> String {
        let mut out = String::new();
        let mut rest = html;
        while !rest.is_empty() {
            let Some(lt) = rest.find('<') else {
                if !self.skipping() {
                    out.push_str(&entities(rest));
                }
                break;
            };
            if !self.skipping() {
                out.push_str(&entities(&rest[..lt]));
            }
            rest = &rest[lt..];
            if let Some(after) = rest.strip_prefix("<!--") {
                rest = after.find("-->").map_or("", |e| &after[e + 3..]);
                continue;
            }
            let Some(end) = tag_end(rest) else {
                // A lone `<` is text.
                if !self.skipping() {
                    out.push_str("&lt;");
                }
                rest = &rest[1..];
                continue;
            };
            let tag = &rest[1..end];
            rest = &rest[end + 1..];
            if let Some(name) = tag.strip_prefix('/') {
                let name = name.trim().to_ascii_lowercase();
                if let Some(top) = self.skipping.last() {
                    if *top == name {
                        self.skipping.pop();
                    }
                    continue;
                }
                if ALLOWED.contains(&name.as_str()) && !VOID.contains(&name.as_str()) {
                    out.push_str(&format!("</{name}>"));
                }
                continue;
            }
            let (name, attrs, self_closing) = parse_tag(tag);
            if name.is_empty() {
                if !self.skipping() {
                    out.push_str("&lt;");
                    out.push_str(&entities(tag));
                    out.push_str("&gt;");
                }
                continue;
            }
            if self.skipping() {
                if DROP_WITH_CONTENT.contains(&name.as_str()) && !self_closing {
                    self.skipping.push(name);
                }
                continue;
            }
            if DROP_WITH_CONTENT.contains(&name.as_str()) {
                self.note(format!("<{name}>"));
                if !self_closing && !VOID.contains(&name.as_str()) {
                    self.skipping.push(name);
                }
                continue;
            }
            if DROP_WARN.contains(&name.as_str()) {
                self.note(format!("<{name}>"));
                continue;
            }
            if !ALLOWED.contains(&name.as_str()) {
                continue;
            }
            out.push('<');
            out.push_str(&name);
            for (k, v) in attrs {
                if k.starts_with("on") {
                    self.note(format!("{k}= on <{name}>"));
                    continue;
                }
                let keep = matches!(k.as_str(), "title" | "lang" | "dir" | "class" | "id")
                    || (name == "a" && k == "href")
                    || (name == "img" && matches!(k.as_str(), "src" | "alt" | "width" | "height"))
                    || (matches!(name.as_str(), "td" | "th")
                        && matches!(k.as_str(), "align" | "colspan" | "rowspan"))
                    || (name == "time" && k == "datetime")
                    || (matches!(name.as_str(), "ol") && matches!(k.as_str(), "start" | "type"))
                    || k == "style";
                if !keep {
                    continue;
                }
                let mut v = v.unwrap_or_default();
                if k == "href" || k == "src" {
                    if !safe_url(&v) {
                        self.note(format!("{k}= with an unsafe URL on <{name}>"));
                        continue;
                    }
                    if k == "src" {
                        match url(&v) {
                            Some(u) => v = u,
                            None => continue,
                        }
                    }
                }
                if k == "style" {
                    let l = v.to_ascii_lowercase();
                    if l.contains("url(") || l.contains("expression") || l.contains("javascript:") {
                        self.note(format!("style= on <{name}>"));
                        continue;
                    }
                }
                out.push_str(&format!(" {k}=\"{}\"", esc_attr(&v)));
            }
            out.push_str(if VOID.contains(&name.as_str()) {
                " />"
            } else {
                ">"
            });
        }
        out
    }
}

/// Index of the `>` that ends the tag starting at `s[0] == '<'`, honoring quotes.
fn tag_end(s: &str) -> Option<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices().skip(1) {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if c == q => quote = None,
            (None, '>') => return Some(i),
            // An unquoted `<` means this was never a tag: emit the first `<` as text.
            (None, '<') => return None,
            _ => {}
        }
    }
    None
}

/// Tag name (lowercase) and attributes. Values are entity-decoded for `&amp;` only.
fn parse_tag(tag: &str) -> (String, Vec<(String, Option<String>)>, bool) {
    let self_closing = tag.trim_end().ends_with('/');
    let tag = tag.trim_end().trim_end_matches('/');
    let name_end = tag.find(|c: char| c.is_whitespace()).unwrap_or(tag.len());
    let name = tag[..name_end].to_ascii_lowercase();
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        || !name.starts_with(|c: char| c.is_ascii_alphabetic())
    {
        return (String::new(), Vec::new(), false);
    }
    let mut attrs = Vec::new();
    let b = tag.as_bytes();
    let mut i = name_end;
    while i < b.len() {
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let ks = i;
        while i < b.len() && !b[i].is_ascii_whitespace() && b[i] != b'=' {
            i += 1;
        }
        if ks == i {
            i += 1;
            continue;
        }
        let key = tag[ks..i].to_ascii_lowercase();
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        let mut value = None;
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                let vs = i + 1;
                i = vs;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                value = Some(tag[vs..i.min(b.len())].replace("&amp;", "&"));
                i += 1;
            } else {
                let vs = i;
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
                value = Some(tag[vs..i].replace("&amp;", "&"));
            }
        }
        attrs.push((key, value));
    }
    (name, attrs, self_closing)
}

/// Keep known entities (as numeric refs) and escape every other `&`.
fn entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i].replace('<', "&lt;").replace('>', "&gt;"));
        rest = &rest[i..];
        let semi = rest[1..].find(';').map(|p| p + 1).filter(|p| *p <= 10);
        let name = semi.map(|p| &rest[1..p]);
        let known = name.and_then(|n| {
            if let Some(num) = n.strip_prefix('#') {
                let ok = num.strip_prefix(['x', 'X']).map_or_else(
                    || num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty(),
                    |h| h.chars().all(|c| c.is_ascii_hexdigit()) && !h.is_empty(),
                );
                ok.then(|| format!("&{n};"))
            } else {
                ENTITIES
                    .iter()
                    .find(|(k, _)| *k == n)
                    .map(|(_, v)| v.to_string())
            }
        });
        match (known, semi) {
            (Some(k), Some(p)) => {
                out.push_str(&k);
                rest = &rest[p + 1..];
            }
            _ => {
                out.push_str("&amp;");
                rest = &rest[1..];
            }
        }
    }
    out.push_str(&rest.replace('<', "&lt;").replace('>', "&gt;"));
    out
}

/// Render markdown to sanitized HTML. `url` maps image paths (embedding or checking
/// assets) and returns `None` to drop the image. Returns the HTML and what was removed.
pub(crate) fn markdown(
    md: &str,
    url: &mut dyn FnMut(&str) -> Option<String>,
) -> (String, Vec<String>) {
    let opts = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let mut san = Sanitizer::default();
    let mut events: Vec<Event> = Vec::new();
    for ev in Parser::new_ext(md, opts) {
        match ev {
            Event::Html(h) | Event::InlineHtml(h) => {
                let safe = san.fragment(&h, url);
                if !safe.is_empty() {
                    events.push(Event::InlineHtml(CowStr::from(safe)));
                }
            }
            _ if san.skipping() => {}
            Event::InlineMath(m) => events.push(Event::InlineHtml(CowStr::from(format!(
                "<span class=\"smd-math-inline\">${}$</span>",
                esc_text(&m)
            )))),
            Event::DisplayMath(m) => events.push(Event::Html(CowStr::from(format!(
                "<div class=\"smd-math-block\">$${}$$</div>",
                esc_text(&m)
            )))),
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let dest = if safe_url(&dest_url) {
                    dest_url
                } else {
                    san.note("javascript: link".into());
                    CowStr::from("#")
                };
                events.push(Event::Start(Tag::Link {
                    link_type,
                    dest_url: dest,
                    title,
                    id,
                }));
            }
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            }) => {
                let dest = if safe_url(&dest_url) {
                    url(&dest_url)
                } else {
                    None
                };
                if dest.is_none() && !safe_url(&dest_url) {
                    san.note("unsafe image URL".into());
                }
                events.push(Event::Start(Tag::Image {
                    link_type,
                    dest_url: CowStr::from(dest.unwrap_or_default()),
                    title,
                    id,
                }));
            }
            other => events.push(other),
        }
    }
    let mut out = String::new();
    html::push_html(&mut out, events.into_iter());
    (out, san.removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(s: &str) -> (String, Vec<String>) {
        markdown(s, &mut |u| Some(u.to_string()))
    }

    #[test]
    fn scripts_handlers_and_js_urls_are_removed() {
        let (h, r) = md(
            "Hi <script>alert(1)</script> there <img src=x.png onerror=\"alert(2)\"> [x](javascript:alert(3))\n",
        );
        assert!(
            !h.contains("script") && !h.contains("alert") && !h.contains("onerror"),
            "{h}"
        );
        assert!(h.contains("<img src=\"x.png\" />"), "{h}");
        assert!(h.contains("href=\"#\""), "{h}");
        assert_eq!(r.len(), 3, "{r:?}");
    }

    #[test]
    fn block_html_and_iframes() {
        let (h, r) = md(
            "<div class=\"note\" onclick=\"x()\">\n<iframe src=\"https://evil\"></iframe>\nkept\n</div>\n",
        );
        assert!(
            h.contains("<div class=\"note\">") && h.contains("kept") && !h.contains("iframe"),
            "{h}"
        );
        assert_eq!(r, ["onclick= on <div>", "<iframe>"]);
    }

    #[test]
    fn safe_markup_and_entities_survive() {
        let (h, r) = md("a <b>bold</b> &nbsp; &bogus; <br> 5 < 6\n");
        assert!(r.is_empty(), "{r:?}");
        assert!(
            h.contains("<b>bold</b>") && h.contains("<br />") && h.contains("&#160;")
                || h.contains("\u{a0}"),
            "{h}"
        );
        assert!(!safe_url(" JaVa\tScript:alert(1)"));
        assert!(safe_url("assets/a.png") && safe_url("https://x.y") && safe_url("#top"));
    }
}
