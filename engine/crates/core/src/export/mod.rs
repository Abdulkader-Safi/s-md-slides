//! Exports that need no fonts or rasterizer: slide HTML, browser SVG and the
//! standalone HTML presentation (FR-EXP-01, FR-EXP-02, FR-EXP-07, FR-ASSET-01, 02).

mod html;
mod presentation;
pub(crate) mod sanitize;
mod svg;

pub use html::slide_html;
pub use presentation::presentation_html;
pub use svg::slide_svg;

use crate::diag::{Diagnostic, Severity};
use crate::theme::AssetResolver;
use std::collections::HashMap;

/// Which moment of a slide's animation to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Frame {
    /// Before anything plays.
    Start,
    /// After every step.
    End,
    /// Milliseconds into the slide's timeline.
    At(u32),
}

/// Export settings.
#[derive(Clone, Copy, Default)]
pub struct Options<'a> {
    /// Put every asset into the output as a data URI, so one file works offline.
    pub embed: bool,
    /// Where asset bytes come from. Without one, paths stay relative and nothing is
    /// checked or embedded.
    pub resolver: Option<&'a dyn AssetResolver>,
}

/// Output text plus the problems found while making it (missing assets, removed HTML).
#[derive(Debug, Clone, PartialEq)]
pub struct Rendered {
    pub text: String,
    pub diagnostics: Vec<Diagnostic>,
}

pub(crate) const BASE_CSS: &str = include_str!("base.css");

/// Resolves each asset path once (FR-ASSET-01) and remembers the answer.
pub(crate) struct Assets<'a> {
    opts: Options<'a>,
    cache: HashMap<String, Option<String>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Assets<'a> {
    pub(crate) fn new(opts: Options<'a>) -> Self {
        Assets {
            opts,
            cache: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    /// The URL to use for `path`: a data URI when embedding, the path itself otherwise.
    /// `None` when the resolver can't find it (reported once as `W-MISSING-ASSET`).
    pub(crate) fn url(&mut self, path: &str) -> Option<String> {
        let local = !path.contains("://") && !path.starts_with("data:") && !path.starts_with('#');
        if !local {
            return Some(path.to_string());
        }
        if let Some(hit) = self.cache.get(path) {
            return hit.clone();
        }
        let answer = match self.opts.resolver {
            None => Some(path.to_string()),
            Some(r) => match r.resolve(path) {
                None => {
                    self.diagnostics.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "W-MISSING-ASSET",
                        message: format!("{path} was not found. A gray box shows in its place."),
                        line: 1,
                        column: 1,
                        element: None,
                        slide: None,
                    });
                    None
                }
                Some(a) if self.opts.embed => {
                    let mime = if a.mime.is_empty() {
                        mime_for(path)
                    } else {
                        a.mime.as_str()
                    };
                    Some(format!("data:{mime};base64,{}", base64(&a.bytes)))
                }
                Some(_) => Some(path.to_string()),
            },
        };
        self.cache.insert(path.to_string(), answer.clone());
        answer
    }
}

/// A best guess from the file extension, for resolvers that don't say.
pub(crate) fn mime_for(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "json" => "application/json",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

/// Standard base64 with padding.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A diagnostic for raw HTML the sanitizer removed (FR-EXP-07).
pub(crate) fn removed_html(
    what: &[String],
    element: &str,
    slide: Option<usize>,
) -> Option<Diagnostic> {
    (!what.is_empty()).then(|| Diagnostic {
        severity: Severity::Warning,
        code: "W-HTML-REMOVED",
        message: format!("Removed unsafe HTML: {}.", what.join(", ")),
        line: 1,
        column: 1,
        element: Some(element.to_string()),
        slide,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_matches_rfc4648() {
        for (i, o) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(super::base64(i.as_bytes()), o);
        }
    }
}
