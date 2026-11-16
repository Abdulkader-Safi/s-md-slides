//! Kind-specific attributes and bodies (FR-EL-01 to FR-EL-09, FR-STYLE-02).
//!
//! Values are kept as written. This module checks them and reads the typed parts
//! hosts need (image path and alt text, code language, table header flag and so on).

use crate::attrs::Directive;
use crate::deck::Kind;
use crate::diag::Severity;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::Serialize;

/// Color roles a theme defines.
pub const COLOR_ROLES: &[&str] = &[
    "background",
    "text",
    "accent1",
    "accent2",
    "accent3",
    "accent4",
    "accent5",
    "accent6",
    "code",
    "code-bg",
];

/// Font roles a theme defines.
pub const FONT_ROLES: &[&str] = &["heading", "body", "code"];

/// Style attributes as written (literal values or theme roles).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Style {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub align: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valign: Option<String>,
    #[serde(rename = "line-height", skip_serializing_if = "Option::is_none")]
    pub line_height: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub padding: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(rename = "stroke-width", skip_serializing_if = "Option::is_none")]
    pub stroke_width: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<String>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }
}

/// What the engine reads from an element's kind-specific attributes and body.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Content {
    Image {
        path: String,
        alt: String,
        fit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        crop: Option<[f32; 4]>,
    },
    Shape {
        shape: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        arrow_start: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        arrow_end: Option<String>,
    },
    Code {
        lang: String,
        code: String,
        /// Highlighted lines as inclusive 1-based ranges.
        lines: Vec<[u32; 2]>,
    },
    Table {
        header: bool,
    },
    Diagram {
        source: String,
    },
    Math {
        source: String,
    },
    Video {
        src: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        poster: Option<String>,
        autoplay: bool,
        #[serde(rename = "loop")]
        looped: bool,
        muted: bool,
    },
    Embed {
        src: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        poster: Option<String>,
    },
    Group {
        items: Vec<String>,
    },
}

pub(crate) type Report<'a> = &'a mut dyn FnMut(Severity, &'static str, String, usize);

fn is_hex_color(s: &str) -> bool {
    s.strip_prefix('#').is_some_and(|h| {
        matches!(h.len(), 3 | 4 | 6 | 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
    })
}

/// `#rgb`, `#rrggbb`, `#rrggbbaa` or a color role.
pub fn is_color(s: &str) -> bool {
    is_hex_color(s) || COLOR_ROLES.contains(&s)
}

/// `linear(angle,color,color[,color])`.
pub fn parse_gradient(s: &str) -> Option<(f32, Vec<String>)> {
    let inner = s.strip_prefix("linear(")?.strip_suffix(')')?;
    let mut parts = inner.split(',').map(str::trim);
    let angle = parts.next()?.trim_end_matches("deg").parse::<f32>().ok()?;
    let stops: Vec<String> = parts.map(str::to_string).collect();
    ((2..=3).contains(&stops.len()) && stops.iter().all(|c| is_color(c))).then_some((angle, stops))
}

fn num(s: &str) -> Option<f32> {
    s.parse::<f32>().ok().filter(|v| v.is_finite())
}

/// `1,3-5` into inclusive ranges.
fn parse_lines(s: &str) -> Option<Vec<[u32; 2]>> {
    s.split(',')
        .map(str::trim)
        .map(|p| match p.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (a.trim().parse::<u32>().ok()?, b.trim().parse::<u32>().ok()?);
                (a >= 1 && a <= b).then_some([a, b])
            }
            None => p.parse::<u32>().ok().filter(|n| *n >= 1).map(|n| [n, n]),
        })
        .collect()
}

/// Top-level blocks of a markdown body, ignoring blank space.
enum Block {
    Image { path: String, alt: String },
    Code { lang: Option<String>, code: String },
    Table,
    DisplayMath(String),
    Other,
}

fn blocks(body: &str) -> Vec<Block> {
    let opts = Options::ENABLE_TABLES | Options::ENABLE_MATH | Options::ENABLE_STRIKETHROUGH;
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur: Option<Block> = None;
    // A paragraph that holds only one image (and whitespace) counts as an image block.
    let mut para_image: Option<(String, String)> = None;
    let mut para_other = false;
    let mut in_image = false;
    for ev in Parser::new_ext(body, opts) {
        match ev {
            Event::Start(tag) => {
                if depth == 0 {
                    match &tag {
                        Tag::CodeBlock(k) => {
                            let lang = match k {
                                CodeBlockKind::Fenced(l) => {
                                    Some(l.split_whitespace().next().unwrap_or("").to_string())
                                }
                                CodeBlockKind::Indented => None,
                            };
                            cur = Some(Block::Code {
                                lang,
                                code: String::new(),
                            });
                        }
                        Tag::Table(_) => cur = Some(Block::Table),
                        Tag::Paragraph => {
                            para_image = None;
                            para_other = false;
                        }
                        _ => cur = Some(Block::Other),
                    }
                } else if let Tag::Image { dest_url, .. } = &tag {
                    if para_image.is_some() {
                        para_other = true;
                    }
                    para_image = Some((dest_url.to_string(), String::new()));
                    in_image = true;
                } else if !in_image {
                    para_other = true;
                }
                depth += 1;
            }
            Event::End(end) => {
                depth = depth.saturating_sub(1);
                if matches!(end, TagEnd::Image) {
                    in_image = false;
                }
                if depth == 0 {
                    if matches!(end, TagEnd::Paragraph) {
                        out.push(match (para_image.take(), para_other) {
                            (Some((path, alt)), false) => Block::Image { path, alt },
                            _ => Block::Other,
                        });
                    } else if let Some(b) = cur.take() {
                        out.push(b);
                    }
                }
            }
            Event::Text(t) => {
                if in_image {
                    if let Some((_, alt)) = &mut para_image {
                        alt.push_str(&t);
                    }
                } else if let Some(Block::Code { code, .. }) = &mut cur {
                    code.push_str(&t);
                } else if depth == 1 && !t.trim().is_empty() && cur.is_none() {
                    para_other = true;
                }
            }
            Event::DisplayMath(m) if depth <= 1 => {
                if depth == 0 {
                    out.push(Block::DisplayMath(m.to_string()));
                } else {
                    para_other = true;
                    para_image = None;
                    // A paragraph holding only `$$...$$` is display math.
                    if !body.trim().is_empty() && body.trim().starts_with("$$") {
                        cur = Some(Block::DisplayMath(m.to_string()));
                    }
                }
            }
            Event::SoftBreak | Event::HardBreak => {}
            _ if depth == 0 => out.push(Block::Other),
            _ => {
                if !in_image {
                    para_other = true;
                }
            }
        }
    }
    // `$$...$$` alone in a paragraph: the parser wraps it, so unwrap it here.
    if out.len() == 1
        && matches!(out[0], Block::Other)
        && let Some(Block::DisplayMath(m)) = cur
    {
        return vec![Block::DisplayMath(m)];
    }
    out
}

/// Check one element. Returns its style and typed content.
pub(crate) fn check(
    kind: &Kind,
    d: &Directive,
    body: &str,
    body_at: usize,
    report: Report,
) -> (Style, Option<Content>) {
    let get = |k: &str| d.get(k).and_then(|a| a.value.clone());
    let at = |k: &str| {
        d.get(k).map_or(d.insert_at, |a| {
            a.value_span.as_ref().map_or(a.span.start, |s| s.start)
        })
    };
    let bad =
        |k: &str, what: &str, report: &mut dyn FnMut(Severity, &'static str, String, usize)| {
            report(
                Severity::Error,
                "E-ATTR-VALUE",
                format!("{k} must be {what}."),
                at(k),
            );
        };

    let style = Style {
        font: get("font"),
        size: get("size"),
        color: get("color"),
        weight: get("weight"),
        align: get("align"),
        valign: get("valign"),
        line_height: get("line-height"),
        padding: get("padding"),
        fill: get("fill"),
        stroke: get("stroke"),
        stroke_width: get("stroke-width"),
        radius: get("radius"),
    };
    if let Some(v) = &style.size
        && !num(v).is_some_and(|n| n > 0.0)
    {
        bad("size", "a positive number", report);
    }
    for (k, v) in [("color", &style.color), ("stroke", &style.stroke)] {
        if let Some(v) = v
            && !(is_color(v) || v == "none")
        {
            bad(
                k,
                "a color such as #e11, #ee1111 or a theme role such as accent1",
                report,
            );
        }
    }
    if let Some(v) = &style.fill
        && !(is_color(v) || v == "none" || parse_gradient(v).is_some())
    {
        bad(
            "fill",
            "a color, a theme role, none, or linear(angle,color,color)",
            report,
        );
    }
    if let Some(v) = &style.weight {
        let ok = matches!(v.as_str(), "normal" | "bold")
            || v.parse::<u32>()
                .is_ok_and(|w| (100..=900).contains(&w) && w % 100 == 0);
        if !ok {
            bad("weight", "normal, bold or 100 to 900", report);
        }
    }
    if let Some(v) = &style.align
        && !matches!(
            v.as_str(),
            "left" | "center" | "right" | "justify" | "start" | "end"
        )
    {
        bad(
            "align",
            "left, center, right, justify, start or end",
            report,
        );
    }
    if let Some(v) = &style.valign
        && !matches!(v.as_str(), "top" | "middle" | "bottom")
    {
        bad("valign", "top, middle or bottom", report);
    }
    for (k, v) in [
        ("line-height", &style.line_height),
        ("stroke-width", &style.stroke_width),
        ("radius", &style.radius),
    ] {
        if let Some(v) = v
            && !num(v).is_some_and(|n| n >= 0.0)
        {
            bad(k, "a number of 0 or more", report);
        }
    }
    if let Some(v) = &style.padding {
        let parts: Vec<_> = v.split(',').map(str::trim).collect();
        if !(1..=4).contains(&parts.len())
            || !parts.iter().all(|p| num(p).is_some_and(|n| n >= 0.0))
        {
            bad(
                "padding",
                "one to four numbers, such as 24 or 24,48",
                report,
            );
        }
    }

    let body_error =
        |code: &'static str,
         msg: &str,
         report: &mut dyn FnMut(Severity, &'static str, String, usize)| {
            report(Severity::Error, code, msg.to_string(), body_at);
        };
    let flag = |k: &str| {
        d.get(k)
            .is_some_and(|a| a.value.as_deref() != Some("false"))
    };
    let content = match kind {
        Kind::Image => {
            let fit = get("fit").unwrap_or_else(|| "contain".into());
            if !matches!(fit.as_str(), "contain" | "cover" | "fill") {
                bad("fit", "contain, cover or fill", report);
            }
            let crop = get("crop").and_then(|c| {
                let n: Vec<f32> = c.split(',').filter_map(|p| num(p.trim())).collect();
                if n.len() == 4 && n[2] >= 0.0 && n[3] >= 0.0 {
                    Some([n[0], n[1], n[2], n[3]])
                } else {
                    bad("crop", "x,y,w,h in image pixels", report);
                    None
                }
            });
            match blocks(body).as_slice() {
                [Block::Image { path, alt }] => {
                    if alt.trim().is_empty() {
                        report(
                            Severity::Warning,
                            "W-NO-ALT",
                            "This image has no alt text. Describe it inside the brackets: ![What it shows](path).".into(),
                            body_at,
                        );
                    }
                    Some(Content::Image {
                        path: path.clone(),
                        alt: alt.clone(),
                        fit,
                        crop,
                    })
                }
                _ => {
                    body_error(
                        "E-IMAGE-BODY",
                        "An image element holds exactly one markdown image: ![alt text](path).",
                        report,
                    );
                    None
                }
            }
        }
        Kind::Shape => {
            let shape = get("kind").unwrap_or_else(|| "rect".into());
            if !matches!(shape.as_str(), "rect" | "ellipse" | "line" | "arrow") {
                bad("kind", "rect, ellipse, line or arrow", report);
            }
            if matches!(shape.as_str(), "line" | "arrow") && !body.trim().is_empty() {
                report(
                    Severity::Warning,
                    "W-SHAPE-TEXT",
                    "Lines and arrows don't show text. Only rect and ellipse do.".into(),
                    body_at,
                );
            }
            Some(Content::Shape {
                shape,
                arrow_start: get("arrow-start"),
                arrow_end: get("arrow-end"),
            })
        }
        Kind::Code => {
            let lines = match get("lines") {
                None => Vec::new(),
                Some(l) => parse_lines(&l).unwrap_or_else(|| {
                    bad("lines", "line numbers and ranges such as 1,3-5", report);
                    Vec::new()
                }),
            };
            match blocks(body).as_slice() {
                [
                    Block::Code {
                        lang: Some(lang),
                        code,
                    },
                ] => {
                    if lang.is_empty() {
                        report(
                            Severity::Warning,
                            "W-CODE-LANG",
                            "This code block has no language tag, such as ```rust.".into(),
                            body_at,
                        );
                    }
                    Some(Content::Code {
                        lang: lang.clone(),
                        code: code.clone(),
                        lines,
                    })
                }
                _ => {
                    body_error(
                        "E-CODE-BODY",
                        "A code element holds exactly one fenced code block (``` with a language).",
                        report,
                    );
                    None
                }
            }
        }
        Kind::Table => {
            let header = !matches!(get("header").as_deref(), Some("false"));
            if !matches!(blocks(body).as_slice(), [Block::Table]) {
                body_error(
                    "E-TABLE-BODY",
                    "A table element holds exactly one markdown table.",
                    report,
                );
            }
            Some(Content::Table { header })
        }
        Kind::Diagram => match blocks(body).as_slice() {
            [
                Block::Code {
                    lang: Some(l),
                    code,
                },
            ] if l == "mermaid" => Some(Content::Diagram {
                source: code.clone(),
            }),
            _ => {
                body_error(
                    "E-DIAGRAM-BODY",
                    "A diagram element holds exactly one ```mermaid block.",
                    report,
                );
                None
            }
        },
        Kind::Math => match blocks(body).as_slice() {
            [Block::DisplayMath(m)] => Some(Content::Math {
                source: m.trim().to_string(),
            }),
            [
                Block::Code {
                    lang: Some(l),
                    code,
                },
            ] if l == "math" => Some(Content::Math {
                source: code.trim().to_string(),
            }),
            _ => {
                body_error(
                    "E-MATH-BODY",
                    "A math element holds one $$...$$ block or one ```math block.",
                    report,
                );
                None
            }
        },
        Kind::Video | Kind::Embed => {
            if !body.trim().is_empty() {
                report(
                    Severity::Warning,
                    "W-BODY-IGNORED",
                    format!(
                        "A {} element shows no text, so this body is ignored.",
                        kind.name()
                    ),
                    body_at,
                );
            }
            match get("src") {
                None => {
                    report(
                        Severity::Error,
                        "E-ATTR-MISSING",
                        format!("A {} element needs src=.", kind.name()),
                        d.insert_at,
                    );
                    None
                }
                Some(src) if *kind == Kind::Embed => {
                    if !src.starts_with("https://") {
                        bad("src", "an https:// address", report);
                    }
                    Some(Content::Embed {
                        src,
                        poster: get("poster"),
                    })
                }
                Some(src) => Some(Content::Video {
                    src,
                    poster: get("poster"),
                    autoplay: flag("autoplay"),
                    looped: flag("loop"),
                    muted: flag("muted"),
                }),
            }
        }
        Kind::Group => {
            let items: Vec<String> = get("items")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if items.len() < 2 {
                report(
                    Severity::Error,
                    "E-GROUP",
                    "A group needs items= with at least two element ids.".into(),
                    at("items"),
                );
            }
            Some(Content::Group { items })
        }
        Kind::Text | Kind::Unknown(_) => None,
    };
    (style, content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_gradients_and_lines() {
        assert!(is_color("#e11") && is_color("#ee1111aa") && is_color("accent3"));
        assert!(!is_color("red") && !is_color("#e1"));
        assert_eq!(
            parse_gradient("linear(90,#000,accent1)").map(|g| g.0),
            Some(90.0)
        );
        assert!(parse_gradient("linear(90,#000)").is_none());
        assert_eq!(parse_lines("1,3-5"), Some(vec![[1, 1], [3, 5]]));
        assert_eq!(parse_lines("0"), None);
    }

    #[test]
    fn body_blocks() {
        assert!(matches!(
            blocks("![a](b.png)\n").as_slice(),
            [Block::Image { .. }]
        ));
        assert!(matches!(
            blocks("![a](b.png) and text\n").as_slice(),
            [Block::Other]
        ));
        assert!(matches!(
            blocks("```rust\nfn x(){}\n```\n").as_slice(),
            [Block::Code { .. }]
        ));
        assert!(matches!(
            blocks("| a | b |\n|---|---|\n| 1 | 2 |\n").as_slice(),
            [Block::Table]
        ));
        assert!(matches!(
            blocks("$$E = mc^2$$\n").as_slice(),
            [Block::DisplayMath(_)]
        ));
        assert!(matches!(
            blocks("# Title\n\n![a](b.png)\n").as_slice(),
            [Block::Other, Block::Image { .. }]
        ));
    }
}
