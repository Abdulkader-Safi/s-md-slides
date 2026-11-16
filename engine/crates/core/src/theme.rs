//! Themes, role resolution, backgrounds and masters (FR-STYLE-01 to FR-STYLE-05).

use crate::deck::{Deck, Element, Kind, Master, MasterSetting};
use crate::diag::{Diagnostic, Severity};
use crate::frontmatter::Dir;
use crate::kinds::{COLOR_ROLES, Content, FONT_ROLES, is_color, parse_gradient};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// A file the host hands to the engine. The engine never reads files itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Asset {
    pub bytes: Vec<u8>,
    pub mime: String,
}

/// The only way the engine sees files: the host resolves paths relative to the deck.
pub trait AssetResolver {
    fn resolve(&self, path: &str) -> Option<Asset>;
}

/// Fonts by role.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Fonts {
    pub heading: String,
    pub body: String,
    pub code: String,
}

/// Colors by role, fonts by role and the default slide background.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct Theme {
    pub colors: BTreeMap<String, String>,
    pub fonts: Fonts,
    /// Color, role, `linear(...)` gradient or image path.
    pub background: String,
}

impl Default for Theme {
    /// Matches `themes/default.json`: light, Noto fonts.
    fn default() -> Self {
        let colors = [
            ("background", "#ffffff"),
            ("text", "#1f2328"),
            ("accent1", "#2563eb"),
            ("accent2", "#db2777"),
            ("accent3", "#16a34a"),
            ("accent4", "#ea580c"),
            ("accent5", "#7c3aed"),
            ("accent6", "#0891b2"),
            ("code", "#1f2328"),
            ("code-bg", "#f3f4f6"),
        ];
        Theme {
            colors: colors
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            fonts: Fonts {
                heading: "Noto Sans".into(),
                body: "Noto Sans".into(),
                code: "Noto Sans Mono".into(),
            },
            background: "background".into(),
        }
    }
}

impl Theme {
    /// Read a theme from JSON (`{colors, fonts, background}`), on top of the default.
    /// Problems come back as messages; good parts still apply.
    pub fn from_json(v: &Value) -> (Theme, Vec<String>) {
        let mut t = Theme::default();
        let mut problems = Vec::new();
        let Some(obj) = v.as_object() else {
            return (
                t,
                vec!["A theme must be a mapping with colors, fonts and background.".into()],
            );
        };
        for (key, val) in obj {
            match (key.as_str(), val) {
                ("colors", Value::Object(c)) => {
                    for (role, color) in c {
                        match color.as_str() {
                            Some(s)
                                if COLOR_ROLES.contains(&role.as_str())
                                    && is_color(s)
                                    && s.starts_with('#') =>
                            {
                                t.colors.insert(role.clone(), s.to_string());
                            }
                            _ => problems.push(format!(
                                "Theme color {role} must be a known role with a #hex value."
                            )),
                        }
                    }
                }
                ("fonts", Value::Object(f)) => {
                    for (role, font) in f {
                        match (role.as_str(), font.as_str()) {
                            ("heading", Some(s)) => t.fonts.heading = s.into(),
                            ("body", Some(s)) => t.fonts.body = s.into(),
                            ("code", Some(s)) => t.fonts.code = s.into(),
                            _ => problems.push(format!("Theme font {role} must be heading, body or code, with a family name.")),
                        }
                    }
                }
                ("background", Value::String(s)) => t.background = s.clone(),
                ("name" | "description", _) => {}
                _ => problems.push(format!("Theme key {key} is not used.")),
            }
        }
        (t, problems)
    }

    /// A color value with roles replaced by the theme's hex colors.
    pub fn color(&self, v: &str) -> String {
        self.colors.get(v).cloned().unwrap_or_else(|| v.to_string())
    }

    /// A font value with roles replaced by family names.
    pub fn font(&self, v: &str) -> String {
        match v {
            "heading" => self.fonts.heading.clone(),
            "body" => self.fonts.body.clone(),
            "code" => self.fonts.code.clone(),
            other => other.to_string(),
        }
    }
}

/// A slide background with roles resolved.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Background {
    Color { color: String },
    Gradient { angle: f32, stops: Vec<String> },
    Image { path: String },
}

/// True when a `bg` value is valid: a color, a role, a gradient or a relative path.
pub(crate) fn valid_background(v: &str) -> bool {
    is_color(v)
        || parse_gradient(v).is_some()
        || (!v.starts_with('#') && (v.contains('.') || v.contains('/')))
}

/// Final style values for drawing an element: roles resolved, defaults filled in.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export, optional_fields))]
pub struct ResolvedStyle {
    pub font: String,
    pub size: f32,
    /// Text color as #hex.
    pub color: String,
    pub weight: u32,
    pub align: String,
    pub valign: String,
    #[serde(rename = "line-height")]
    pub line_height: f32,
    /// Top, right, bottom, left.
    pub padding: [f32; 4],
    /// #hex or `linear(angle,#hex,#hex)`; `None` for no fill.
    pub fill: Option<String>,
    pub stroke: Option<String>,
    #[serde(rename = "stroke-width")]
    pub stroke_width: f32,
    pub radius: f32,
    pub dir: Dir,
}

fn padding(v: &str) -> Option<[f32; 4]> {
    let n: Vec<f32> = v
        .split(',')
        .map(|p| p.trim().parse::<f32>().ok())
        .collect::<Option<_>>()?;
    Some(match n.as_slice() {
        [a] => [*a; 4],
        [a, b] => [*a, *b, *a, *b],
        [a, b, c] => [*a, *b, *c, *b],
        [a, b, c, d] => [*a, *b, *c, *d],
        _ => return None,
    })
}

impl Deck {
    /// The theme in use: inline front matter, a loaded theme file, or the default.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Load a theme file named in front matter (`theme: path.json`) through the host.
    /// Returns the problems found; the deck keeps the default for anything bad.
    pub fn load_theme(&mut self, resolver: &dyn AssetResolver) -> Vec<Diagnostic> {
        let Some(Value::String(path)) = self.front.theme.clone() else {
            return Vec::new();
        };
        let diag = |severity, code, message: String| Diagnostic {
            severity,
            code,
            message,
            line: 1,
            column: 1,
            element: None,
            slide: None,
        };
        let Some(asset) = resolver.resolve(&path) else {
            return vec![diag(
                Severity::Warning,
                "W-MISSING-ASSET",
                format!("Theme file {path} was not found. Using the default theme."),
            )];
        };
        let value = match serde_json::from_slice::<Value>(&asset.bytes) {
            Ok(v) => v,
            Err(e) => {
                return vec![diag(
                    Severity::Error,
                    "E-THEME",
                    format!("Theme file {path} is not valid JSON: {e}."),
                )];
            }
        };
        let (theme, problems) = Theme::from_json(&value);
        self.theme = theme;
        self.theme_path = Some(path);
        problems
            .into_iter()
            .map(|p| diag(Severity::Warning, "W-THEME", p))
            .collect()
    }

    /// The master drawn behind slide `index`: its own `master=`, else its layout's
    /// master, else the first master in the deck. `master=none` gives `None`.
    pub fn slide_master(&self, index: usize) -> Option<&Master> {
        let s = self.slides.get(index)?;
        let by_id = |id: &str| self.masters.iter().find(|m| m.id == id);
        match &s.master {
            MasterSetting::None => None,
            MasterSetting::Id(id) => by_id(id),
            MasterSetting::Inherit => {
                let from_layout = s
                    .layout
                    .as_ref()
                    .and_then(|l| self.layouts.iter().find(|x| &x.id == l))
                    .and_then(|l| l.master.as_deref());
                match from_layout {
                    Some(id) => by_id(id),
                    None => self.masters.first(),
                }
            }
        }
    }

    /// Master elements drawn on slide `index`, minus the ones the slide hides.
    pub fn master_elements(&self, index: usize) -> Vec<&Element> {
        let hide = self.slides.get(index).map_or(&[][..], |s| &s.hide[..]);
        self.slide_master(index)
            .map(|m| {
                m.elements
                    .iter()
                    .filter(|e| !hide.contains(&e.id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Master text with `{slide}`, `{total}` and `{title}` filled in for slide `index`.
    pub fn master_text(&self, text: &str, index: usize) -> String {
        text.replace("{slide}", &(index + 1).to_string())
            .replace("{total}", &self.slides.len().to_string())
            .replace("{title}", self.front.title.as_deref().unwrap_or(""))
    }

    /// The background of slide `index`: its `bg=`, else the theme's.
    pub fn background(&self, index: usize) -> Background {
        let raw = self
            .slides
            .get(index)
            .and_then(|s| s.bg.clone())
            .filter(|b| valid_background(b))
            .unwrap_or_else(|| self.theme.background.clone());
        if is_color(&raw) {
            Background::Color {
                color: self.theme.color(&raw),
            }
        } else if let Some((angle, stops)) = parse_gradient(&raw) {
            Background::Gradient {
                angle,
                stops: stops.iter().map(|c| self.theme.color(c)).collect(),
            }
        } else {
            Background::Image { path: raw }
        }
    }

    /// Style values for drawing element `id`, with theme roles and defaults applied.
    pub fn resolved_style(&self, id: &str) -> Option<ResolvedStyle> {
        let e = self.element(id)?;
        let t = &self.theme;
        let st = &e.style;
        let code = e.kind == Kind::Code;
        let line_shape = matches!(&e.content, Some(Content::Shape { shape, .. }) if shape == "line" || shape == "arrow");
        let paint = |v: &Option<String>, default: Option<&str>| -> Option<String> {
            match v.as_deref().or(default) {
                None | Some("none") => None,
                Some(g) if g.starts_with("linear(") => parse_gradient(g).map(|(a, stops)| {
                    let stops: Vec<String> = stops.iter().map(|c| t.color(c)).collect();
                    format!("linear({a},{})", stops.join(","))
                }),
                Some(c) => Some(t.color(c)),
            }
        };
        let num = |v: &Option<String>, default: f32| {
            v.as_deref()
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(default)
        };
        let fill_default = match e.kind {
            Kind::Shape if !line_shape => Some("accent1"),
            Kind::Code => Some("code-bg"),
            _ => None,
        };
        let stroke = paint(&st.stroke, line_shape.then_some("text"));
        Some(ResolvedStyle {
            font: t.font(
                st.font
                    .as_deref()
                    .unwrap_or(if code { "code" } else { "body" }),
            ),
            size: num(
                &st.size,
                if code || e.kind == Kind::Table {
                    24.0
                } else {
                    32.0
                },
            ),
            color: t.color(
                st.color
                    .as_deref()
                    .unwrap_or(if code { "code" } else { "text" }),
            ),
            weight: match st.weight.as_deref() {
                Some("bold") => 700,
                Some(w) => w.parse().unwrap_or(400),
                None => 400,
            },
            align: st.align.clone().unwrap_or_else(|| "start".into()),
            valign: st.valign.clone().unwrap_or_else(|| "top".into()),
            line_height: num(&st.line_height, 1.3),
            padding: st
                .padding
                .as_deref()
                .and_then(padding)
                .unwrap_or([if code { 24.0 } else { 0.0 }; 4]),
            fill: paint(&st.fill, fill_default),
            stroke_width: num(&st.stroke_width, if stroke.is_some() { 2.0 } else { 0.0 }),
            stroke,
            radius: num(&st.radius, 0.0),
            dir: self.resolved_dir(e),
        })
    }
}

/// Theme from front matter: inline mapping now, file path later through `load_theme`.
pub(crate) fn inline_theme(v: Option<&Value>) -> (Theme, Vec<String>) {
    match v {
        Some(obj @ Value::Object(_)) => Theme::from_json(obj),
        _ => (Theme::default(), Vec::new()),
    }
}

#[allow(dead_code)]
fn roles_are_complete() -> bool {
    FONT_ROLES.len() == 3
        && COLOR_ROLES
            .iter()
            .all(|r| Theme::default().colors.contains_key(*r))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_every_role() {
        assert!(roles_are_complete());
    }

    #[test]
    fn padding_shorthand() {
        assert_eq!(padding("10"), Some([10.0; 4]));
        assert_eq!(padding("10,20"), Some([10.0, 20.0, 10.0, 20.0]));
        assert_eq!(padding("x"), None);
    }
}
