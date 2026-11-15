//! YAML front matter, read with serde-saphyr. Never written from the model: edits
//! splice key lines (M2).

use crate::diag::{Diagnostic, Severity};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// Newest format version this engine edits. Newer decks open read-only.
pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}

impl Default for Canvas {
    fn default() -> Self {
        Canvas {
            width: 1920,
            height: 1080,
        }
    }
}

impl Canvas {
    /// `16:9`, `4:5` or `WxH` (for example `1200x800`).
    pub fn parse(s: &str) -> Option<Canvas> {
        match s.trim() {
            "16:9" => Some(Canvas::default()),
            "4:5" => Some(Canvas {
                width: 1080,
                height: 1350,
            }),
            other => {
                let (w, h) = other.split_once(['x', 'X'])?;
                let (w, h) = (w.trim().parse::<u32>().ok()?, h.trim().parse::<u32>().ok()?);
                (1..=100_000).contains(&w).then_some(())?;
                (1..=100_000).contains(&h).then_some(())?;
                Some(Canvas {
                    width: w,
                    height: h,
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Dir {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

impl Dir {
    pub fn parse(s: &str) -> Option<Dir> {
        match s {
            "auto" => Some(Dir::Auto),
            "ltr" => Some(Dir::Ltr),
            "rtl" => Some(Dir::Rtl),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct FrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub canvas: Canvas,
    pub dir: Dir,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Inline mapping or a relative path to a JSON theme. Resolved in M3.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<String>,
    /// Keys the engine does not know, kept for hosts. They round-trip in the text.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub extra: BTreeMap<String, Value>,
}

pub(crate) struct ParsedFront {
    pub front: FrontMatter,
    pub version: u32,
    pub diagnostics: Vec<Diagnostic>,
}

fn diag(code: &'static str, message: String, line: u32, column: u32) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        code,
        message,
        line,
        column,
        element: None,
        slide: None,
    }
}

/// Line (1-based) where `key:` starts at the top level, for diagnostics.
fn key_line(text: &str, key: &str, first_line: u32) -> u32 {
    text.lines()
        .position(|l| {
            l.strip_prefix(key)
                .is_some_and(|r| r.trim_start().starts_with(':'))
        })
        .map_or(first_line, |i| first_line + i as u32)
}

fn as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// `text` is the front matter content; `first_line` is its 1-based line in the file.
pub(crate) fn parse(text: &str, first_line: u32) -> ParsedFront {
    let mut out = ParsedFront {
        front: FrontMatter::default(),
        version: FORMAT_VERSION,
        diagnostics: Vec::new(),
    };
    if text.trim().is_empty() {
        return out;
    }
    let map = match serde_saphyr::from_str::<Value>(text) {
        Ok(Value::Object(map)) => map,
        Ok(Value::Null) => return out,
        Ok(_) => {
            out.diagnostics.push(diag(
                "E-FRONTMATTER",
                "Front matter must be a mapping of keys to values.".into(),
                first_line,
                1,
            ));
            return out;
        }
        Err(e) => {
            let (line, column) = e.location().map_or((first_line, 1), |l| {
                (
                    first_line + l.line().saturating_sub(1) as u32,
                    l.column().max(1) as u32,
                )
            });
            let msg = e.to_string();
            let msg = msg.lines().next().unwrap_or("invalid YAML").trim();
            out.diagnostics.push(diag(
                "E-FRONTMATTER",
                format!("Front matter is not valid YAML: {msg}"),
                line,
                column,
            ));
            return out;
        }
    };
    let f = &mut out.front;
    for (key, value) in map {
        let line = || key_line(text, &key, first_line);
        match key.as_str() {
            "s-md-slides" => match value.as_u64() {
                Some(v) if v >= 1 => out.version = v.min(u32::MAX as u64) as u32,
                _ => out.diagnostics.push(diag(
                    "E-FRONTMATTER",
                    "s-md-slides must be a whole number such as 1.".into(),
                    line(),
                    1,
                )),
            },
            "title" => f.title = as_string(&value),
            "author" => f.author = as_string(&value),
            "lang" => f.lang = as_string(&value),
            "transition" => f.transition = as_string(&value),
            "theme" => f.theme = Some(value),
            "canvas" => match as_string(&value).as_deref().and_then(Canvas::parse) {
                Some(c) => f.canvas = c,
                None => out.diagnostics.push(diag(
                    "E-CANVAS",
                    "canvas must be 16:9, 4:5 or WxH such as 1200x800. Using 16:9.".into(),
                    line(),
                    1,
                )),
            },
            "dir" => match as_string(&value).as_deref().and_then(Dir::parse) {
                Some(d) => f.dir = d,
                None => out.diagnostics.push(diag(
                    "E-ATTR-VALUE",
                    "dir must be auto, ltr or rtl. Using auto.".into(),
                    line(),
                    1,
                )),
            },
            _ => {
                f.extra.insert(key, value);
            }
        }
    }
    if out.version > FORMAT_VERSION {
        out.diagnostics.push(diag(
            "E-VERSION",
            format!(
                "This deck uses format version {}, newer than {FORMAT_VERSION}. It opens read-only.",
                out.version
            ),
            key_line(text, "s-md-slides", first_line),
            1,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_forms() {
        assert_eq!(
            Canvas::parse("16:9"),
            Some(Canvas {
                width: 1920,
                height: 1080
            })
        );
        assert_eq!(
            Canvas::parse("4:5"),
            Some(Canvas {
                width: 1080,
                height: 1350
            })
        );
        assert_eq!(
            Canvas::parse("1200x800"),
            Some(Canvas {
                width: 1200,
                height: 800
            })
        );
        assert_eq!(Canvas::parse("0x800"), None);
        assert_eq!(Canvas::parse("wide"), None);
    }

    #[test]
    fn reads_known_and_unknown_keys() {
        let p = parse(
            "s-md-slides: 1\ntitle: Q3\ncanvas: 4:5\ndir: rtl\nfoo: bar\n",
            2,
        );
        assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
        assert_eq!(p.front.title.as_deref(), Some("Q3"));
        assert_eq!(p.front.canvas.height, 1350);
        assert_eq!(p.front.dir, Dir::Rtl);
        assert_eq!(p.front.extra["foo"], Value::String("bar".into()));
    }

    #[test]
    fn newer_version_and_bad_yaml() {
        let p = parse("s-md-slides: 2\n", 2);
        assert_eq!(p.version, 2);
        assert_eq!(p.diagnostics[0].code, "E-VERSION");
        let p = parse("title: [unclosed\n", 2);
        assert_eq!(p.diagnostics[0].code, "E-FRONTMATTER");
    }
}
