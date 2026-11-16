//! A slide at one frame as layers: background SVG, then per element its shapes and
//! images as SVG plus its text as glyph runs. `to_svg` draws the runs as outline paths;
//! the PDF writer draws them as real text.

use crate::fonts::{Fonts, Inner};
use crate::graphics;
use crate::layout::{Run, TextOpts, code_box, hex, rgba, text_box};
use s_md_slides::export::{Frame, base64, mime_for, shape_svg};
use s_md_slides::{
    AssetResolver, Background, Content, Deck, Diagnostic, Element, Kind, Severity, State,
};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

/// One element, ready to draw.
pub(crate) struct Item {
    /// Element-local SVG (shapes, images, rules), drawn first.
    pub svg: String,
    /// Text, drawn over `svg`.
    pub runs: Vec<Run>,
    /// Element to slide transform: [a, b, c, d, e, f].
    pub matrix: [f64; 6],
    pub opacity: f64,
    /// Clip text to the element box (width, height), as the HTML export does.
    pub clip: Option<(f64, f64)>,
}

pub(crate) struct Scene {
    pub width: u32,
    pub height: u32,
    pub background: String,
    pub items: Vec<Item>,
    pub diagnostics: Vec<Diagnostic>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn num(v: f64) -> String {
    s_md_slides::attrs::fmt_num(v)
}

/// translate(x+dx, y+dy) · rotate(rot about the box center) · scale(s about the center).
fn matrix(e: &Element, s: State) -> [f64; 6] {
    let (cx, cy) = (e.w / 2.0, e.h / 2.0);
    let (sin, cos) = e.rot.to_radians().sin_cos();
    let k = s.scale;
    // R·S about the center, then translate.
    let (a, b, c, d) = (cos * k, sin * k, -sin * k, cos * k);
    let e_ = e.x + s.dx + cx - (a * cx + c * cy);
    let f_ = e.y + s.dy + cy - (b * cx + d * cy);
    [a, b, c, d, e_, f_]
}

/// Asset bytes as data URIs, each path asked for once.
pub(crate) struct Assets<'a> {
    resolver: &'a dyn AssetResolver,
    cache: HashMap<String, Option<String>>,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Assets<'a> {
    pub(crate) fn new(resolver: &'a dyn AssetResolver) -> Self {
        Assets {
            resolver,
            cache: HashMap::new(),
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn data_uri(&mut self, path: &str) -> Option<String> {
        if let Some(hit) = self.cache.get(path) {
            return hit.clone();
        }
        // No network in native export: only local files through the resolver.
        let local = !path.contains("://") && !path.starts_with("data:") && !path.contains(':');
        let uri = local
            .then(|| self.resolver.resolve(path))
            .flatten()
            .map(|a| {
                let mime = if a.mime.is_empty() {
                    mime_for(path).to_string()
                } else {
                    a.mime
                };
                format!("data:{mime};base64,{}", base64(&a.bytes))
            });
        if uri.is_none() {
            self.diagnostics.push(Diagnostic {
                severity: Severity::Warning,
                code: "W-MISSING-ASSET",
                message: format!("{path} was not found. A gray box shows in its place."),
                line: 1,
                column: 1,
                element: None,
                slide: None,
            });
        }
        self.cache.insert(path.to_string(), uri.clone());
        uri
    }
}

/// Animation states of slide `index` at `frame`.
pub(crate) fn states(deck: &Deck, index: usize, frame: Frame) -> BTreeMap<String, State> {
    let Some(t) = deck.timeline(index) else {
        return BTreeMap::new();
    };
    match frame {
        Frame::Start => t.initial.clone(),
        Frame::End => t.state_at(t.duration as f64),
        Frame::At(ms) => t.state_at(ms as f64),
    }
}

fn gray_box(w: f64, h: f64) -> String {
    format!(
        "<rect width=\"{}\" height=\"{}\" fill=\"#d0d4da\" />",
        num(w),
        num(h)
    )
}

/// Text for a gray placeholder box (a missing asset's path).
fn label(f: &mut Inner, deck: &Deck, text: &str, w: f64, h: f64) -> Vec<Run> {
    let t = deck.theme();
    let style = s_md_slides::ResolvedStyle {
        font: t.fonts.body.clone(),
        size: 16.0,
        color: "#444444".into(),
        weight: 400,
        align: "center".into(),
        valign: "middle".into(),
        line_height: 1.3,
        padding: [8.0; 4],
        fill: None,
        stroke: None,
        stroke_width: 0.0,
        radius: 0.0,
        dir: s_md_slides::Dir::Ltr,
    };
    let o = TextOpts {
        style: &style,
        heading_font: &t.fonts.heading,
        code_font: &t.fonts.code,
        link: rgba("#444444"),
        dir: s_md_slides::Dir::Ltr,
        align: None,
        valign: None,
    };
    text_box(f, &text.replace(['*', '_', '`', '#', '['], " "), &o, w, h).runs
}

fn image(uri: &str, w: f64, h: f64, fit: &str, crop: Option<[f64; 4]>) -> String {
    let par = match fit {
        "cover" => "xMidYMid slice",
        "fill" => "none",
        _ => "xMidYMid meet",
    };
    match crop {
        Some([x, y, cw, ch]) => format!(
            "<svg width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"{par}\"><image href=\"{}\" /></svg>",
            num(w),
            num(h),
            num(x),
            num(y),
            num(cw),
            num(ch),
            esc(uri)
        ),
        None => format!(
            "<image href=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{par}\" />",
            esc(uri),
            num(w),
            num(h)
        ),
    }
}

/// Build the scene for slide `index` at `frame`.
pub(crate) fn scene(
    deck: &Deck,
    index: usize,
    frame: Frame,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Option<Scene> {
    scene_with(deck, index, &states(deck, index, frame), resolver, fonts)
}

/// Build the scene for slide `index` with given animation states.
pub(crate) fn scene_with(
    deck: &Deck,
    index: usize,
    st: &BTreeMap<String, State>,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Option<Scene> {
    let slide = deck.slides.get(index)?;
    let c = deck.canvas();
    let (w, h) = (c.width as f64, c.height as f64);
    let mut assets = Assets::new(resolver);
    let mut diags = Vec::new();
    let background = match deck.background(index) {
        Background::Color { color } => format!(
            "<rect width=\"{}\" height=\"{}\" fill=\"{}\" />",
            num(w),
            num(h),
            esc(&color)
        ),
        Background::Gradient { angle, stops } => {
            let (s, co) = angle.to_radians().sin_cos();
            let n = stops.len().max(2) - 1;
            let st: String = stops
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    format!(
                        "<stop offset=\"{}\" stop-color=\"{}\" />",
                        num(i as f64 / n as f64),
                        esc(col)
                    )
                })
                .collect();
            format!(
                "<defs><linearGradient id=\"smd-bg\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{st}</linearGradient></defs><rect width=\"{}\" height=\"{}\" fill=\"url(#smd-bg)\" />",
                num(0.5 - 0.5 * s),
                num(0.5 + 0.5 * co),
                num(0.5 + 0.5 * s),
                num(0.5 - 0.5 * co),
                num(w),
                num(h)
            )
        }
        Background::Image { path } => match assets.data_uri(&path) {
            Some(u) => image(&u, w, h, "cover", None),
            None => gray_box(w, h),
        },
    };
    let mut items = Vec::new();
    let mut f = fonts.inner.borrow_mut();
    let masters: Vec<(&Element, bool)> = deck
        .master_elements(index)
        .into_iter()
        .map(|e| (e, true))
        .collect();
    for (e, master) in masters
        .into_iter()
        .chain(slide.elements.iter().map(|e| (e, false)))
    {
        let state = if master {
            State::VISIBLE
        } else {
            st.get(&e.id).copied().unwrap_or(State::VISIBLE)
        };
        if let Some(item) = element(
            &mut f,
            deck,
            e,
            index,
            master,
            state,
            &mut assets,
            &mut diags,
        ) {
            items.push(item);
        }
    }
    diags.append(&mut assets.diagnostics);
    Some(Scene {
        width: c.width,
        height: c.height,
        background,
        items,
        diagnostics: diags,
    })
}

#[allow(clippy::too_many_arguments)]
fn element(
    f: &mut Inner,
    deck: &Deck,
    e: &Element,
    slide: usize,
    master: bool,
    state: State,
    assets: &mut Assets,
    diags: &mut Vec<Diagnostic>,
) -> Option<Item> {
    let r = deck.resolved_style(&e.id)?;
    let t = deck.theme();
    let o = TextOpts {
        style: &r,
        heading_font: &t.fonts.heading,
        code_font: &t.fonts.code,
        link: rgba(&t.color("accent1")),
        dir: deck.resolved_dir(e),
        align: None,
        valign: None,
    };
    let body = if master {
        deck.master_text(&e.body, slide)
    } else {
        e.body.clone()
    };
    let (w, h) = (e.w, e.h);
    let mut svg = String::new();
    let mut runs = Vec::new();
    let mut clip = Some((w, h));
    match (&e.kind, &e.content) {
        (Kind::Text, _) => {
            let tb = text_box(f, &body, &o, w, h);
            svg.push_str(&tb.svg);
            runs = tb.runs;
        }
        (Kind::Table, _) => {
            let tb = text_box(f, &body, &o, w, h);
            svg.push_str(&tb.svg);
            runs = tb.runs;
        }
        (Kind::Shape, Some(Content::Shape { shape, .. })) => {
            let s = shape_svg(e, &r, shape).replacen(
                "<svg ",
                &format!(
                    "<svg width=\"{}\" height=\"{}\" overflow=\"visible\" ",
                    num(w),
                    num(h)
                ),
                1,
            );
            svg.push_str(&s);
            clip = None;
            if matches!(shape.as_str(), "rect" | "ellipse") && !body.trim().is_empty() {
                let mut sr = r.clone();
                sr.padding = [0.5 * r.size; 4];
                let so = TextOpts {
                    style: &sr,
                    align: Some("center"),
                    valign: Some("middle"),
                    ..o
                };
                runs = text_box(f, &body, &so, w, h).runs;
            }
        }
        (
            Kind::Image,
            Some(Content::Image {
                path, fit, crop, ..
            }),
        ) => match assets.data_uri(path) {
            Some(u) => svg.push_str(&image(&u, w, h, fit, *crop)),
            None => {
                svg.push_str(&gray_box(w, h));
                runs = label(f, deck, path, w, h);
            }
        },
        (Kind::Code, Some(Content::Code { code, lines, .. })) => {
            if let Some(fill) = &r.fill {
                svg.push_str(&format!(
                    "<rect width=\"{}\" height=\"{}\" fill=\"{}\" />",
                    num(w),
                    num(h),
                    esc(fill)
                ));
            }
            let tb = code_box(f, code, lines, &o, w);
            svg.push_str(&tb.svg);
            runs = tb.runs;
        }
        (Kind::Diagram | Kind::Math, content) => {
            let made = match content {
                Some(Content::Diagram { source }) => graphics::mermaid(source),
                Some(Content::Math { source }) => graphics::math(source, &r.color, r.size),
                _ => Err("no source".into()),
            };
            match made {
                Ok(g) => svg.push_str(&format!(
                    "<image href=\"data:image/svg+xml;base64,{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"xMidYMid meet\" />",
                    base64(g.as_bytes()), num(w), num(h)
                )),
                Err(why) => {
                    diags.push(Diagnostic {
                        severity: Severity::Warning,
                        code: "W-DIAGRAM",
                        message: format!("Could not draw this {}: {why}. Its source shows instead.", e.kind.name()),
                        line: 1,
                        column: 1,
                        element: Some(e.id.clone()),
                        slide: Some(slide),
                    });
                    let source = match content {
                        Some(Content::Diagram { source } | Content::Math { source }) => source.clone(),
                        _ => String::new(),
                    };
                    let mut cr = r.clone();
                    cr.font = t.fonts.code.clone();
                    cr.size = cr.size.min(24.0);
                    cr.color = t.color("code");
                    cr.padding = [16.0; 4];
                    svg.push_str(&format!("<rect width=\"{}\" height=\"{}\" rx=\"8\" fill=\"{}\" />", num(w), num(h), esc(&t.color("code-bg"))));
                    let co = TextOpts { style: &cr, ..o };
                    runs = code_box(f, &source, &[], &co, w).runs;
                }
            }
        }
        (Kind::Video | Kind::Embed, content) => {
            // No playback outside HTML: the poster frame, or a gray box with the source.
            let (src, poster) = match content {
                Some(Content::Video { src, poster, .. }) | Some(Content::Embed { src, poster }) => {
                    (src.clone(), poster.clone())
                }
                _ => (String::new(), None),
            };
            match poster.and_then(|p| assets.data_uri(&p)) {
                Some(u) => svg.push_str(&image(&u, w, h, "cover", None)),
                None => {
                    svg.push_str(&gray_box(w, h));
                    runs = label(f, deck, &src, w, h);
                }
            }
        }
        _ => return None,
    }
    Some(Item {
        svg,
        runs,
        matrix: matrix(e, state),
        opacity: e.opacity * state.opacity,
        clip,
    })
}

/// Collects glyph outlines as SVG path data (y flipped to SVG's downward axis).
struct Pen(String);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "M{x} {}", -y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let _ = write!(self.0, "L{x} {}", -y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let _ = write!(self.0, "Q{cx0} {} {x} {}", -cy0, -y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let _ = write!(self.0, "C{cx0} {} {cx1} {} {x} {}", -cy0, -cy1, -y);
    }
    fn close(&mut self) {
        self.0.push('Z');
    }
}

fn outline(f: &mut Inner, run: &Run, gid: u32) -> String {
    let key = (run.font.data.id(), run.font.index, gid, run.size.to_bits());
    if let Some(d) = f.outlines.get(&key) {
        return d.clone();
    }
    let mut pen = Pen(String::new());
    if let Ok(font) = FontRef::from_index(run.font.data.data(), run.font.index)
        && let Some(g) = font.outline_glyphs().get(GlyphId::new(gid))
    {
        let _ = g.draw(
            DrawSettings::unhinted(Size::new(run.size), LocationRef::default()),
            &mut pen,
        );
    }
    f.outlines.insert(key, pen.0.clone());
    pen.0
}

/// Glyph runs as filled paths.
pub(crate) fn runs_svg(fonts: &Fonts, runs: &[Run]) -> String {
    let mut f = fonts.inner.borrow_mut();
    let mut out = String::new();
    for run in runs {
        let c = run.color;
        let mut paint = format!("fill=\"{}\"", hex(c));
        if c[3] != 255 {
            let _ = write!(paint, " fill-opacity=\"{}\"", num(c[3] as f64 / 255.0));
        }
        if run.embolden {
            let _ = write!(
                paint,
                " stroke=\"{}\" stroke-width=\"{}\"",
                hex(c),
                num(run.size as f64 / 40.0)
            );
        }
        let _ = write!(out, "<g {paint}>");
        let mut pen = run.x;
        for g in &run.glyphs {
            let d = outline(&mut f, run, g.id);
            if !d.is_empty() {
                let (gx, gy) = (pen + g.dx as f64, run.y + g.dy as f64);
                let skew = if run.skew != 0.0 {
                    format!(" skewX({})", num(-run.skew as f64))
                } else {
                    String::new()
                };
                let _ = write!(
                    out,
                    "<path transform=\"translate({} {}){skew}\" d=\"{d}\" />",
                    num(gx),
                    num(gy)
                );
            }
            pen += g.advance as f64;
        }
        out.push_str("</g>");
    }
    out
}

pub(crate) fn matrix_attr(m: &[f64; 6]) -> String {
    format!(
        "matrix({} {} {} {} {} {})",
        num(m[0]),
        num(m[1]),
        num(m[2]),
        num(m[3]),
        num(m[4]),
        num(m[5])
    )
}

/// The whole scene as one SVG with text as outlines: no fonts needed to draw it.
pub(crate) fn to_svg(scene: &Scene, fonts: &Fonts) -> String {
    let (w, h) = (scene.width, scene.height);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{}",
        scene.background
    );
    for (i, it) in scene.items.iter().enumerate() {
        let op = if it.opacity != 1.0 {
            format!(" opacity=\"{}\"", num(it.opacity))
        } else {
            String::new()
        };
        let _ = write!(
            out,
            "<g transform=\"{}\"{op}>{}",
            matrix_attr(&it.matrix),
            it.svg
        );
        if !it.runs.is_empty() {
            match it.clip {
                Some((cw, ch)) => {
                    let _ = write!(
                        out,
                        "<clipPath id=\"smd-c{i}\"><rect width=\"{}\" height=\"{}\" /></clipPath><g clip-path=\"url(#smd-c{i})\">{}</g>",
                        num(cw),
                        num(ch),
                        runs_svg(fonts, &it.runs)
                    );
                }
                None => out.push_str(&runs_svg(fonts, &it.runs)),
            }
        }
        out.push_str("</g>");
    }
    out.push_str("</svg>");
    out
}
