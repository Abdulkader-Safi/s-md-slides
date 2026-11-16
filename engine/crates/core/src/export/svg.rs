//! Browser SVG per slide: shapes and images as SVG, text as HTML inside
//! `foreignObject`, so the browser does text layout and bidi (FR-EXP-01).

use super::html::{Ctx, element_inner, frame_states, text_style, theme_vars};
use super::sanitize::esc_attr;
use super::{BASE_CSS, Frame, Options, Rendered};
use crate::attrs::fmt_num;
use crate::deck::{Deck, Element, Kind};
use crate::kinds::Content;
use crate::theme::Background;
use crate::timeline::State;

fn group_open(e: &Element, st: Option<State>) -> String {
    let s = st.unwrap_or(State::VISIBLE);
    let (cx, cy) = (e.w / 2.0, e.h / 2.0);
    let mut t = format!("translate({} {})", fmt_num(e.x + s.dx), fmt_num(e.y + s.dy));
    if e.rot != 0.0 {
        t.push_str(&format!(
            " rotate({} {} {})",
            fmt_num(e.rot),
            fmt_num(cx),
            fmt_num(cy)
        ));
    }
    if s.scale != 1.0 {
        t.push_str(&format!(
            " translate({} {}) scale({}) translate({} {})",
            fmt_num(cx),
            fmt_num(cy),
            fmt_num(s.scale),
            fmt_num(-cx),
            fmt_num(-cy)
        ));
    }
    let opacity = e.opacity * s.opacity;
    let op = if opacity != 1.0 {
        format!(" opacity=\"{}\"", fmt_num(opacity))
    } else {
        String::new()
    };
    format!("<g data-id=\"{}\" transform=\"{t}\"{op}>", esc_attr(&e.id))
}

fn foreign(e: &Element, style: &str, class: &str, html: &str, dir: &str) -> String {
    format!(
        "<foreignObject x=\"0\" y=\"0\" width=\"{}\" height=\"{}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"smd-el {class}\" dir=\"{dir}\" style=\"position:absolute;inset:0;width:100%;height:100%;{}\">{html}</div></foreignObject>",
        fmt_num(e.w),
        fmt_num(e.h),
        esc_attr(style)
    )
}

fn element_svg(cx: &mut Ctx, e: &Element, slide: usize, master: bool, st: Option<State>) -> String {
    let Some((class, inner)) = element_inner(cx, e, slide, master) else {
        return String::new();
    };
    let r = cx.deck.resolved_style(&e.id).expect("element exists");
    let dir = if cx.deck.resolved_dir(e) == crate::frontmatter::Dir::Rtl {
        "rtl"
    } else {
        "ltr"
    };
    let (w, h) = (fmt_num(e.w), fmt_num(e.h));
    let body = match (&e.kind, &e.content) {
        (Kind::Shape, Some(Content::Shape { .. })) => {
            // The shape's own svg nests fine; text inside goes into a foreignObject.
            match inner.split_once("</svg>") {
                Some((svg, text)) => {
                    let svg = svg.replacen(
                        "<svg ",
                        &format!("<svg width=\"{w}\" height=\"{h}\" overflow=\"visible\" "),
                        1,
                    );
                    let text = if text.is_empty() {
                        String::new()
                    } else {
                        foreign(e, "", "smd-shape", text, dir)
                    };
                    format!("{svg}</svg>{text}")
                }
                None => inner,
            }
        }
        (
            Kind::Image,
            Some(Content::Image {
                path,
                alt,
                fit,
                crop: None,
            }),
        ) => match cx.assets.url(path) {
            Some(u) => {
                let par = match fit.as_str() {
                    "cover" => "xMidYMid slice",
                    "fill" => "none",
                    _ => "xMidYMid meet",
                };
                format!(
                    "<image href=\"{}\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"{par}\"><title>{}</title></image>",
                    esc_attr(&u),
                    esc_attr(alt)
                )
            }
            None => foreign(e, "", &class, &inner, dir),
        },
        (Kind::Image, _) => {
            inner.replacen("<svg ", &format!("<svg width=\"{w}\" height=\"{h}\" "), 1)
        }
        (Kind::Video | Kind::Embed, content) => {
            // No playback in SVG: the poster, or a gray box with the source.
            let (src, poster) = match content {
                Some(Content::Video { src, poster, .. }) | Some(Content::Embed { src, poster }) => {
                    (src.clone(), poster.clone())
                }
                _ => (String::new(), None),
            };
            match poster.and_then(|p| cx.assets.url(&p)) {
                Some(u) => format!(
                    "<image href=\"{}\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"xMidYMid slice\" />",
                    esc_attr(&u)
                ),
                None => foreign(
                    e,
                    "",
                    "smd-missing-box",
                    &format!("<div class=\"smd-missing\">{}</div>", esc_attr(&src)),
                    "ltr",
                ),
            }
        }
        (Kind::Text | Kind::Table, _) => foreign(e, &text_style(&r), &class, &inner, dir),
        (Kind::Diagram | Kind::Math, _) => {
            let label = if e.kind == Kind::Diagram {
                "Mermaid diagram (rendered in native export)"
            } else {
                "Math (rendered in native export)"
            };
            format!(
                "<foreignObject x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\"><pre xmlns=\"http://www.w3.org/1999/xhtml\" class=\"smd-el {class}\" data-label=\"{label}\" style=\"position:absolute;inset:0\">{inner}</pre></foreignObject>"
            )
        }
        _ => foreign(e, "", &class, &inner, dir),
    };
    format!("{}{body}</g>", group_open(e, st))
}

/// One slide as a standalone SVG at a moment of its animation (FR-EXP-01).
pub fn slide_svg(deck: &Deck, index: usize, frame: Frame, opts: &Options) -> Option<Rendered> {
    deck.slides.get(index)?;
    let mut cx = Ctx::new(deck, *opts);
    let states = frame_states(deck, index, frame);
    let c = deck.canvas();
    let (w, h) = (c.width, c.height);
    let bg = match deck.background(index) {
        Background::Color { color } => {
            format!("<rect width=\"{w}\" height=\"{h}\" fill=\"{color}\" />")
        }
        Background::Gradient { angle, stops } => {
            let rad = angle.to_radians();
            let (s, co) = (rad.sin(), rad.cos());
            let n = stops.len().max(2) - 1;
            let st: String = stops
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    format!(
                        "<stop offset=\"{}\" stop-color=\"{col}\" />",
                        fmt_num(i as f64 / n as f64)
                    )
                })
                .collect();
            format!(
                "<defs><linearGradient id=\"smd-bg\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">{st}</linearGradient></defs><rect width=\"{w}\" height=\"{h}\" fill=\"url(#smd-bg)\" />",
                fmt_num(0.5 - 0.5 * s),
                fmt_num(0.5 + 0.5 * co),
                fmt_num(0.5 + 0.5 * s),
                fmt_num(0.5 - 0.5 * co)
            )
        }
        Background::Image { path } => match cx.assets.url(&path) {
            Some(u) => format!(
                "<image href=\"{}\" width=\"{w}\" height=\"{h}\" preserveAspectRatio=\"xMidYMid slice\" />",
                esc_attr(&u)
            ),
            None => format!("<rect width=\"{w}\" height=\"{h}\" fill=\"#d0d4da\" />"),
        },
    };
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\"><style>svg{{{}}}{}</style>{bg}",
        theme_vars(deck),
        BASE_CSS.replace(".smd-slide ", "svg ")
    );
    for e in deck.master_elements(index) {
        out.push_str(&element_svg(&mut cx, e, index, true, None));
    }
    for e in &deck.slides[index].elements {
        out.push_str(&element_svg(
            &mut cx,
            e,
            index,
            false,
            states.get(&e.id).copied(),
        ));
    }
    out.push_str("</svg>");
    Some(Rendered {
        text: out,
        diagnostics: cx.finish(),
    })
}
