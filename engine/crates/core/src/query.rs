//! Read-only queries over a deck (FR-OPS-05, FR-ASSET-03).

use crate::deck::{Deck, Element, Kind};
use serde::Serialize;
use serde_json::Value;
use std::ops::Range;

/// Where a search match was found.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct Hit {
    pub slide: usize,
    /// The element, or `None` for a match in the notes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<String>,
    pub field: HitField,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum HitField {
    Body,
    Notes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub enum AssetKind {
    Image,
    Video,
    Poster,
    Theme,
    Background,
}

/// A file the deck points at, by its path as written (relative to the deck).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct AssetRef {
    pub path: String,
    pub kind: AssetKind,
    /// Element ids, `slide:N` (0-based) for backgrounds, or `front` for the theme.
    pub used_by: Vec<String>,
}

/// True for paths the host should resolve: not URLs, data, absolute or colors.
fn is_local_path(p: &str) -> bool {
    !p.is_empty()
        && !p.contains("://")
        && !p.starts_with("data:")
        && !p.starts_with('/')
        && !p.starts_with('#')
        && !p.starts_with("mailto:")
}

/// Paths of markdown images `![alt](path "title")` in a body.
fn image_paths(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(i) = rest.find("![") {
        rest = &rest[i + 2..];
        let Some(close) = rest.find("](") else { break };
        if rest[..close].contains('\n') {
            continue;
        }
        let after = &rest[close + 2..];
        let end = after.find(')').unwrap_or(after.len());
        let target = after[..end].trim();
        let path = if let Some(t) = target.strip_prefix('<') {
            t.split('>').next().unwrap_or("")
        } else {
            target.split_whitespace().next().unwrap_or("")
        };
        out.push(path.to_string());
        rest = &after[end..];
    }
    out
}

impl Deck {
    /// Byte range of an element in `text()`: its directive line, body and the blank
    /// lines after it. Hosts can use it to jump from a box to its source.
    pub fn element_range(&self, id: &str) -> Option<Range<usize>> {
        self.elements()
            .find(|e| e.id == id)
            .map(|e| e.src.start()..e.src.end)
    }

    /// Byte range of a slide in `text()`, without its `---` lines.
    pub fn slide_range(&self, index: usize) -> Option<Range<usize>> {
        self.slides.get(index).map(|s| s.src.span.clone())
    }

    /// Elements under a point on a slide, top first. Rotation is taken into account.
    /// Groups are skipped (they have no box of their own).
    pub fn elements_at(&self, slide: usize, x: f64, y: f64) -> Vec<String> {
        let Some(s) = self.slides.get(slide) else {
            return Vec::new();
        };
        s.elements
            .iter()
            .rev()
            .filter(|e| e.kind != Kind::Group && contains(e, x, y))
            .map(|e| e.id.clone())
            .collect()
    }

    /// Case-insensitive search in element bodies and speaker notes.
    pub fn search(&self, query: &str) -> Vec<Hit> {
        let q = query.to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for s in &self.slides {
            for e in &s.elements {
                if e.body.to_lowercase().contains(&q) {
                    hits.push(Hit {
                        slide: s.index,
                        element: Some(e.id.clone()),
                        field: HitField::Body,
                    });
                }
            }
            if s.notes
                .as_ref()
                .is_some_and(|n| n.to_lowercase().contains(&q))
            {
                hits.push(Hit {
                    slide: s.index,
                    element: None,
                    field: HitField::Notes,
                });
            }
        }
        hits
    }

    /// Every file the deck references, each path once, in file order.
    pub fn assets(&self) -> Vec<AssetRef> {
        let mut out: Vec<AssetRef> = Vec::new();
        let mut add = |path: &str, kind: AssetKind, by: String| {
            if !is_local_path(path) {
                return;
            }
            match out.iter_mut().find(|a| a.path == path) {
                Some(a) => {
                    if !a.used_by.contains(&by) {
                        a.used_by.push(by);
                    }
                }
                None => out.push(AssetRef {
                    path: path.to_string(),
                    kind,
                    used_by: vec![by],
                }),
            }
        };
        if let Some(Value::String(t)) = &self.front.theme {
            add(t, AssetKind::Theme, "front".into());
        }
        for s in &self.slides {
            if let Some(bg) = &s.bg {
                let looks_like_file = bg.contains('.') || bg.contains('/');
                if looks_like_file && !bg.starts_with("linear(") {
                    add(bg, AssetKind::Background, format!("slide:{}", s.index));
                }
            }
        }
        for e in self.elements() {
            for p in image_paths(&e.body) {
                add(&p, AssetKind::Image, e.id.clone());
            }
            if e.kind == Kind::Video
                && let Some(src) = e.attr("src")
            {
                add(src, AssetKind::Video, e.id.clone());
            }
            if let Some(poster) = e.attr("poster") {
                add(poster, AssetKind::Poster, e.id.clone());
            }
        }
        out
    }
}

/// Point-in-box test in the element's own rotated frame (rotation about its center).
fn contains(e: &Element, x: f64, y: f64) -> bool {
    let (cx, cy) = (e.x + e.w / 2.0, e.y + e.h / 2.0);
    let (s, c) = (-e.rot).to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    let (lx, ly) = (dx * c - dy * s, dx * s + dy * c);
    lx.abs() <= e.w / 2.0 && ly.abs() <= e.h / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_image_paths() {
        assert_eq!(
            image_paths("![a](x.png) text ![b](<y z.jpg>) ![c](q.png \"t\")"),
            ["x.png", "y z.jpg", "q.png"]
        );
    }
}
