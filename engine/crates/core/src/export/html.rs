//! Slide HTML: each element absolutely positioned on a canvas-sized section.

use super::sanitize::{esc_attr, esc_text, markdown};
use super::{Assets, Frame, Options, Rendered, removed_html};
use crate::attrs::fmt_num;
use crate::deck::{Deck, Element, Kind};
use crate::diag::{Diagnostic, LineIndex};
use crate::kinds::{Content, parse_gradient};
use crate::theme::{Background, ResolvedStyle};
use crate::timeline::State;
use std::collections::BTreeMap;

/// Shared state while rendering one or more slides.
pub(crate) struct Ctx<'a, 'o> {
    pub deck: &'a Deck,
    pub assets: Assets<'o>,
    pub diagnostics: Vec<Diagnostic>,
    lines: LineIndex,
}

impl<'a, 'o> Ctx<'a, 'o> {
    pub(crate) fn new(deck: &'a Deck, opts: Options<'o>) -> Self {
        Ctx {
            deck,
            assets: Assets::new(opts),
            diagnostics: Vec::new(),
            lines: LineIndex::new(deck.text()),
        }
    }

    /// Diagnostics found so far, with asset problems placed at the elements using them.
    pub(crate) fn finish(mut self) -> Vec<Diagnostic> {
        self.diagnostics.append(&mut self.assets.diagnostics);
        self.diagnostics
    }

    fn at(&self, e: &Element, mut d: Diagnostic) -> Diagnostic {
        let (line, column) = self.lines.pos(self.deck.text(), e.src.start());
        d.line = line;
        d.column = column;
        d
    }

    /// Markdown body to safe HTML, reporting removed HTML against `e`.
    pub(crate) fn md(&mut self, e: &Element, text: &str, slide: Option<usize>) -> String {
        let assets = &mut self.assets;
        let (html, removed) = markdown(text, &mut |u| assets.url(u));
        if let Some(d) = removed_html(&removed, &e.id, slide) {
            let d = self.at(e, d);
            self.diagnostics.push(d);
        }
        html
    }
}

pub(crate) fn px(v: f64) -> String {
    format!("{}px", fmt_num(v))
}

fn font_family(f: &str) -> String {
    format!("'{}',sans-serif", f.replace(['\'', '"', ';', '<', '>'], ""))
}

/// CSS custom properties for the theme, set on each slide.
pub(crate) fn theme_vars(deck: &Deck) -> String {
    let t = deck.theme();
    let c = |r: &str| t.color(r);
    format!(
        "--smd-heading:{};--smd-body:{};--smd-code:'{}',monospace;--smd-text:{};--smd-accent1:{};--smd-code-bg:{};--smd-code-fg:{}",
        font_family(&t.fonts.heading),
        font_family(&t.fonts.body),
        t.fonts.code.replace(['\'', '"', ';'], ""),
        c("text"),
        c("accent1"),
        c("code-bg"),
        c("code")
    )
}

/// CSS `background` for a slide.
pub(crate) fn background_css(cx: &mut Ctx, index: usize) -> String {
    match cx.deck.background(index) {
        Background::Color { color } => format!("background:{color}"),
        Background::Gradient { angle, stops } => {
            format!(
                "background:linear-gradient({}deg,{})",
                fmt_num(angle),
                stops.join(",")
            )
        }
        Background::Image { path } => match cx.assets.url(&path) {
            Some(u) => format!(
                "background:#d0d4da url(\"{}\") center/cover no-repeat",
                esc_attr(&u)
            ),
            None => "background:#d0d4da".into(),
        },
    }
}

/// CSS `linear-gradient` or a color from a resolved paint value.
fn css_paint(v: &str) -> String {
    match parse_gradient(v) {
        Some((a, stops)) => format!("linear-gradient({}deg,{})", fmt_num(a), stops.join(",")),
        None => v.to_string(),
    }
}

/// Inline style for the element box: position, rotation, opacity and animation state.
pub(crate) fn box_style(e: &Element, st: Option<State>, z: usize) -> String {
    let s = st.unwrap_or(State::VISIBLE);
    let mut t = String::new();
    if s.dx != 0.0 || s.dy != 0.0 {
        t.push_str(&format!("translate({},{}) ", px(s.dx), px(s.dy)));
    }
    if e.rot != 0.0 {
        t.push_str(&format!("rotate({}deg) ", fmt_num(e.rot)));
    }
    if s.scale != 1.0 {
        t.push_str(&format!("scale({}) ", fmt_num(s.scale)));
    }
    let mut out = format!(
        "left:{};top:{};width:{};height:{};z-index:{z}",
        px(e.x),
        px(e.y),
        px(e.w),
        px(e.h)
    );
    if !t.is_empty() {
        out.push_str(&format!(";transform:{}", t.trim_end()));
    }
    let opacity = e.opacity * s.opacity;
    if opacity != 1.0 {
        out.push_str(&format!(";opacity:{}", fmt_num(opacity)));
    }
    out
}

/// Text styling for text-bearing boxes.
pub(crate) fn text_style(r: &ResolvedStyle) -> String {
    let justify = match r.valign.as_str() {
        "middle" => "center",
        "bottom" => "flex-end",
        _ => "flex-start",
    };
    let p = r.padding.map(px);
    format!(
        "font-family:{};font-size:{};color:{};font-weight:{};text-align:{};line-height:{};padding:{} {} {} {};justify-content:{justify}",
        font_family(&r.font),
        px(r.size),
        r.color,
        r.weight,
        r.align,
        fmt_num(r.line_height),
        p[0],
        p[1],
        p[2],
        p[3]
    )
}

fn missing(path: &str) -> String {
    format!("<div class=\"smd-missing\">{}</div>", esc_text(path))
}

/// YouTube links go to the no-cookie domain (spec: Security).
pub(crate) fn embed_src(src: &str) -> String {
    let id = src
        .strip_prefix("https://www.youtube.com/watch?v=")
        .or_else(|| src.strip_prefix("https://youtube.com/watch?v="))
        .or_else(|| src.strip_prefix("https://youtu.be/"))
        .or_else(|| src.strip_prefix("https://www.youtube.com/embed/"))
        .map(|rest| rest.split(['&', '?', '#']).next().unwrap_or(""));
    match id {
        Some(id) if !id.is_empty() => format!("https://www.youtube-nocookie.com/embed/{id}"),
        _ => src.to_string(),
    }
}

/// SVG markup for a shape, sized `w` by `h` in its own coordinates.
pub(crate) fn shape_svg(e: &Element, r: &ResolvedStyle, shape: &str) -> String {
    let (w, h) = (e.w, e.h);
    let mut defs = String::new();
    let fill = match &r.fill {
        None => "none".to_string(),
        Some(f) => match parse_gradient(f) {
            Some((angle, stops)) => {
                let rad = angle.to_radians();
                let (s, c) = (rad.sin(), rad.cos());
                let gid = format!("smd-g-{}", e.id);
                let n = stops.len().max(2) - 1;
                let stops: String = stops
                    .iter()
                    .enumerate()
                    .map(|(i, col)| {
                        format!(
                            "<stop offset=\"{}\" stop-color=\"{col}\" />",
                            fmt_num(i as f64 / n as f64)
                        )
                    })
                    .collect();
                defs = format!(
                    "<defs><linearGradient id=\"{gid}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{stops}</linearGradient></defs>",
                    fmt_num(0.5 - 0.5 * s),
                    fmt_num(0.5 + 0.5 * c),
                    fmt_num(0.5 + 0.5 * s),
                    fmt_num(0.5 - 0.5 * c)
                );
                format!("url(#{gid})")
            }
            None => f.clone(),
        },
    };
    let stroke = r.stroke.clone().unwrap_or_else(|| "none".into());
    let sw = fmt_num(r.stroke_width);
    let paint = format!(
        "fill=\"{}\" stroke=\"{}\" stroke-width=\"{sw}\"",
        esc_attr(&fill),
        esc_attr(&stroke)
    );
    let body = match shape {
        "ellipse" => format!(
            "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {paint} />",
            fmt_num(w / 2.0),
            fmt_num(h / 2.0),
            fmt_num(w / 2.0),
            fmt_num(h / 2.0)
        ),
        "line" | "arrow" => {
            let marker = if shape == "arrow" {
                let mid = format!("smd-a-{}", e.id);
                defs.push_str(&format!(
                    "<defs><marker id=\"{mid}\" viewBox=\"0 0 10 10\" refX=\"8\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" fill=\"{}\" /></marker></defs>",
                    esc_attr(&stroke)
                ));
                let start = matches!(e.attr("arrow-start"), Some(v) if v != "none");
                let end = !matches!(e.attr("arrow-end"), Some("none"));
                let mut m = String::new();
                if end {
                    m.push_str(&format!(" marker-end=\"url(#{mid})\""));
                }
                if start {
                    m.push_str(&format!(" marker-start=\"url(#{mid})\""));
                }
                m
            } else {
                String::new()
            };
            format!(
                "<line x1=\"0\" y1=\"0\" x2=\"{}\" y2=\"{}\" stroke=\"{}\" stroke-width=\"{sw}\" stroke-linecap=\"round\"{marker} />",
                fmt_num(w),
                fmt_num(h),
                esc_attr(&stroke)
            )
        }
        _ => format!(
            "<rect x=\"0\" y=\"0\" width=\"{}\" height=\"{}\" rx=\"{}\" {paint} />",
            fmt_num(w),
            fmt_num(h),
            fmt_num(r.radius)
        ),
    };
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"none\">{defs}{body}</svg>",
        fmt_num(w.max(1.0)),
        fmt_num(h.max(1.0))
    )
}

/// Code with one span per line; highlighted lines get `smd-hl`.
pub(crate) fn code_html(lang: &str, code: &str, lines: &[[u32; 2]], r: &ResolvedStyle) -> String {
    let body: String = code
        .trim_end_matches('\n')
        .split('\n')
        .enumerate()
        .map(|(i, l)| {
            let n = i as u32 + 1;
            let hl = lines.iter().any(|[a, b]| (*a..=*b).contains(&n));
            format!(
                "<span class=\"smd-line{}\">{}</span>",
                if hl { " smd-hl" } else { "" },
                esc_text(l)
            )
        })
        .collect();
    let fill = r
        .fill
        .as_deref()
        .map(css_paint)
        .unwrap_or_else(|| "transparent".into());
    format!(
        "<pre class=\"smd-code\" data-lang=\"{}\" style=\"{};background:{fill};width:100%;height:100%\"><code>{body}</code></pre>",
        esc_attr(lang),
        text_style(r)
    )
}

/// The inner HTML and class of one element, or `None` for kinds that draw nothing.
pub(crate) fn element_inner(
    cx: &mut Ctx,
    e: &Element,
    slide: usize,
    master: bool,
) -> Option<(String, String)> {
    let r = cx.deck.resolved_style(&e.id)?;
    let body = if master {
        cx.deck.master_text(&e.body, slide)
    } else {
        e.body.clone()
    };
    let flow = |cx: &mut Ctx, cls: &str| {
        let html = cx.md(e, &body, Some(slide));
        (cls.to_string(), html)
    };
    Some(match (&e.kind, &e.content) {
        (Kind::Text, _) => flow(cx, "smd-text"),
        (
            Kind::Image,
            Some(Content::Image {
                path,
                alt,
                fit,
                crop,
            }),
        ) => {
            let inner = match cx.assets.url(path) {
                None => missing(path),
                Some(u) => match crop {
                    Some([x, y, w, h]) => {
                        let par = match fit.as_str() {
                            "cover" => "xMidYMid slice",
                            "fill" => "none",
                            _ => "xMidYMid meet",
                        };
                        format!(
                            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" preserveAspectRatio=\"{par}\" role=\"img\" aria-label=\"{}\"><image href=\"{}\" /></svg>",
                            fmt_num(*x),
                            fmt_num(*y),
                            fmt_num(*w),
                            fmt_num(*h),
                            esc_attr(alt),
                            esc_attr(&u)
                        )
                    }
                    None => format!(
                        "<img src=\"{}\" alt=\"{}\" style=\"object-fit:{}\" />",
                        esc_attr(&u),
                        esc_attr(alt),
                        esc_attr(fit)
                    ),
                },
            };
            ("smd-image".into(), inner)
        }
        (Kind::Image, _) => ("smd-image".into(), missing("image")),
        (Kind::Shape, Some(Content::Shape { shape, .. })) => {
            let mut inner = shape_svg(e, &r, shape);
            if matches!(shape.as_str(), "rect" | "ellipse") && !body.trim().is_empty() {
                let html = cx.md(e, &body, Some(slide));
                inner.push_str(&format!(
                    "<div class=\"smd-shape-text\" style=\"{}\">{html}</div>",
                    text_style(&r)
                ));
            }
            ("smd-shape".into(), inner)
        }
        (Kind::Code, Some(Content::Code { lang, code, lines })) => {
            ("smd-codebox".into(), code_html(lang, code, lines, &r))
        }
        (Kind::Table, Some(Content::Table { header })) => {
            let (cls, html) = flow(cx, "smd-flow smd-table");
            (
                if *header {
                    cls
                } else {
                    format!("{cls} smd-noheader")
                },
                html,
            )
        }
        (Kind::Diagram, Some(Content::Diagram { source })) => {
            ("smd-diagram".into(), esc_text(source))
        }
        (Kind::Math, Some(Content::Math { source })) => ("smd-math".into(), esc_text(source)),
        (
            Kind::Video,
            Some(Content::Video {
                src,
                poster,
                autoplay,
                looped,
                muted,
            }),
        ) => {
            let inner = match cx.assets.url(src) {
                None => missing(src),
                Some(u) => {
                    let poster = poster.as_ref().and_then(|p| cx.assets.url(p));
                    let mut v = format!(
                        "<video src=\"{}\" controls=\"\" playsinline=\"\" preload=\"metadata\"",
                        esc_attr(&u)
                    );
                    if let Some(p) = poster {
                        v.push_str(&format!(" poster=\"{}\"", esc_attr(&p)));
                    }
                    for (on, name) in [
                        (*autoplay, "autoplay"),
                        (*looped, "loop"),
                        (*muted, "muted"),
                    ] {
                        if on {
                            v.push_str(&format!(" {name}=\"\""));
                        }
                    }
                    v.push_str("></video>");
                    v
                }
            };
            ("smd-video".into(), inner)
        }
        (Kind::Embed, Some(Content::Embed { src, .. })) => {
            // Only https embeds become iframes; anything else shows the gray box.
            let url = embed_src(src);
            if !url.starts_with("https://") || !super::sanitize::safe_url(&url) {
                return Some(("smd-embed".into(), missing(src)));
            }
            (
                "smd-embed".into(),
                format!(
                    "<iframe src=\"{}\" title=\"{}\" allow=\"fullscreen; picture-in-picture\" allowfullscreen=\"\" loading=\"lazy\" referrerpolicy=\"strict-origin-when-cross-origin\"></iframe>",
                    esc_attr(&url),
                    esc_attr(e.name.as_deref().unwrap_or("Embedded content"))
                ),
            )
        }
        _ => return None,
    })
}

/// One element as a positioned `div`, or an empty string for kinds that draw nothing.
pub(crate) fn element_html(
    cx: &mut Ctx,
    e: &Element,
    slide: usize,
    master: bool,
    st: Option<State>,
    z: usize,
) -> String {
    let Some((class, inner)) = element_inner(cx, e, slide, master) else {
        return String::new();
    };
    let r = cx.deck.resolved_style(&e.id).expect("element exists");
    let mut style = box_style(e, st, z);
    let texty = matches!(e.kind, Kind::Text | Kind::Table);
    if texty {
        style.push(';');
        style.push_str(&text_style(&r));
    }
    if matches!(e.kind, Kind::Diagram | Kind::Math) {
        style.push_str(&format!(";font-size:{}", px(r.size.min(32.0))));
    }
    let label = match e.kind {
        Kind::Diagram => " data-label=\"Mermaid diagram (rendered in native export)\"",
        Kind::Math => " data-label=\"Math (rendered in native export)\"",
        _ => "",
    };
    let dir = match cx.deck.resolved_dir(e) {
        crate::frontmatter::Dir::Rtl => " dir=\"rtl\"",
        _ => " dir=\"ltr\"",
    };
    format!(
        "<div class=\"smd-el {class}\" data-id=\"{}\"{}{dir}{label} style=\"{}\">{inner}</div>",
        esc_attr(&e.id),
        if master { " data-master=\"\"" } else { "" },
        esc_attr(&style)
    )
}

/// The animation state of slide `index` at `frame`, keyed by element id.
pub(crate) fn frame_states(deck: &Deck, index: usize, frame: Frame) -> BTreeMap<String, State> {
    let Some(t) = deck.timeline(index) else {
        return BTreeMap::new();
    };
    let at = match frame {
        Frame::Start => 0.0,
        Frame::End => t.duration as f64,
        Frame::At(ms) => ms as f64,
    };
    // At the start, elements that enter later show their entrance start state.
    if frame == Frame::Start {
        return t.initial.clone();
    }
    t.state_at(at)
}

/// A whole slide as a `section`, sized to the canvas.
pub(crate) fn slide_section(
    cx: &mut Ctx,
    index: usize,
    states: &BTreeMap<String, State>,
) -> String {
    let deck = cx.deck;
    let c = deck.canvas();
    let mut out = format!(
        "<section class=\"smd-slide\" data-slide=\"{index}\" style=\"width:{}px;height:{}px;{};{}\">",
        c.width,
        c.height,
        theme_vars(deck),
        esc_attr(&background_css(cx, index))
    );
    let mut z = 0;
    for e in deck.master_elements(index) {
        z += 1;
        out.push_str(&element_html(cx, e, index, true, None, z));
    }
    for e in &deck.slides[index].elements {
        z += 1;
        out.push_str(&element_html(
            cx,
            e,
            index,
            false,
            states.get(&e.id).copied(),
            z,
        ));
    }
    out.push_str("</section>");
    out
}

/// HTML of one slide at a moment of its animation (FR-EXP-01). Include `BASE_CSS`
/// (see [`slide_css`]) in the page that shows it.
pub fn slide_html(deck: &Deck, index: usize, frame: Frame, opts: &Options) -> Option<Rendered> {
    deck.slides.get(index)?;
    let mut cx = Ctx::new(deck, *opts);
    let states = frame_states(deck, index, frame);
    let text = slide_section(&mut cx, index, &states);
    Some(Rendered {
        text,
        diagnostics: cx.finish(),
    })
}
