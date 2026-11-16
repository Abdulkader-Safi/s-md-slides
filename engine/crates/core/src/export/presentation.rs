//! The standalone HTML presentation (FR-EXP-02): every slide, the presentation
//! runtime, its data and a Content Security Policy, in one file.

use super::html::{Ctx, slide_section};
use super::sanitize::{esc_attr, esc_text};
use super::{BASE_CSS, Options, Rendered};
use crate::deck::Deck;
use crate::frontmatter::Dir;
use crate::timeline::Timeline;
use serde::Serialize;
use std::collections::BTreeMap;

/// The presentation runtime, built from `engine/runtime` by esbuild and committed.
pub(crate) const RUNTIME_JS: &str = include_str!("../runtime/runtime.js");
pub(crate) const RUNTIME_CSS: &str = include_str!("../runtime/runtime.css");
/// Base64 SHA-256 of `RUNTIME_JS`, for the CSP `script-src`.
pub(crate) const RUNTIME_SHA256: &str = include_str!("../runtime/runtime.sha256");
pub(crate) const RELOAD_JS: &str = include_str!("../runtime/reload.js");
pub(crate) const RELOAD_SHA256: &str = include_str!("../runtime/reload.sha256");

#[derive(Serialize)]
struct SlideData {
    transition: String,
    dur: u32,
    /// Speaker notes as safe HTML. Shown in the presenter view, never on the slide.
    notes: String,
    timeline: Timeline,
}

#[derive(Serialize)]
struct Data {
    title: String,
    width: u32,
    height: u32,
    rtl: bool,
    slides: Vec<SlideData>,
}

/// Content Security Policy for exported presentations (spec: Security).
pub(crate) fn csp(extra_script_hashes: &[&str]) -> String {
    let mut scripts = format!("'sha256-{}'", RUNTIME_SHA256.trim());
    for h in extra_script_hashes {
        scripts.push_str(&format!(" 'sha256-{}'", h.trim()));
    }
    format!(
        "default-src 'self' data: blob:; img-src 'self' file: data: blob:; media-src 'self' file: data: blob:; style-src 'unsafe-inline'; script-src {scripts}; frame-src https://www.youtube-nocookie.com https://player.vimeo.com; object-src 'none'; base-uri 'none'; form-action 'none'"
    )
}

/// The whole deck as one HTML file. With `opts.embed`, assets become data URIs so the
/// file works offline without the deck's folder (FR-ASSET-02).
pub fn presentation_html(deck: &Deck, opts: &Options) -> Rendered {
    let mut cx = Ctx::new(deck, *opts);
    let title = deck
        .front
        .title
        .clone()
        .unwrap_or_else(|| "Presentation".into());
    let rtl = deck.front.dir == Dir::Rtl;
    let mut sections = String::new();
    let mut slides = Vec::with_capacity(deck.slides.len());
    let deck_transition = deck
        .front
        .transition
        .clone()
        .unwrap_or_else(|| "fade".into());
    for (i, s) in deck.slides.iter().enumerate() {
        // Slides are drawn at their end state; the runtime applies each step's states.
        let end: BTreeMap<_, _> = deck
            .timeline(i)
            .map(|t| t.state_at(t.duration as f64))
            .unwrap_or_default();
        sections.push_str(&slide_section(&mut cx, i, &end));
        let notes = match (
            &s.notes,
            s.elements
                .first()
                .or(deck.master_elements(i).first().copied()),
        ) {
            (Some(n), Some(e)) => cx.md(e, n, Some(i)),
            (Some(n), None) => esc_text(n),
            (None, _) => String::new(),
        };
        slides.push(SlideData {
            transition: s
                .transition
                .clone()
                .unwrap_or_else(|| deck_transition.clone()),
            dur: s.transition_dur.unwrap_or(400),
            notes,
            timeline: deck.timeline(i).expect("slide exists"),
        });
    }
    let c = deck.canvas();
    let data = Data {
        title: title.clone(),
        width: c.width,
        height: c.height,
        rtl,
        slides,
    };
    // `</` inside JSON must not end the script element.
    let json = serde_json::to_string(&data)
        .unwrap_or_default()
        .replace("</", "<\\/");
    let lang = deck.front.lang.clone().unwrap_or_else(|| "en".into());
    let text = format!(
        concat!(
            "<!doctype html>\n<html lang=\"{lang}\" dir=\"{dir}\"><head><meta charset=\"utf-8\">",
            "<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">",
            "<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">",
            "<title>{title}</title><style>{base}{rt}</style></head>",
            "<body class=\"smd-loading\">",
            "<main id=\"smd-stage\" dir=\"ltr\" aria-roledescription=\"presentation\" aria-label=\"{title_attr}\">{sections}",
            "<div id=\"smd-end\" class=\"smd-end\" hidden><p>End of deck</p></div></main>",
            "<nav class=\"smd-controls\" aria-label=\"Slide controls\">",
            "<button type=\"button\" id=\"smd-prev\" aria-label=\"Previous slide\">&#8592;</button>",
            "<span id=\"smd-counter\" aria-live=\"polite\">1 / {total}</span>",
            "<button type=\"button\" id=\"smd-next\" aria-label=\"Next slide\">&#8594;</button>",
            "<button type=\"button\" id=\"smd-full\" aria-label=\"Full screen (F)\">&#9974;</button>",
            "<button type=\"button\" id=\"smd-presenter\" aria-label=\"Presenter view (P)\">&#9998;</button>",
            "</nav><div id=\"smd-progress\" role=\"progressbar\" aria-label=\"Progress\" aria-valuemin=\"0\" aria-valuemax=\"{total}\" aria-valuenow=\"1\"><div></div></div>",
            "<script type=\"application/json\" id=\"smd-data\">{json}</script>",
            "<script>{js}</script>{reload}</body></html>\n"
        ),
        lang = esc_attr(&lang),
        dir = if rtl { "rtl" } else { "ltr" },
        csp = esc_attr(&csp(if opts.live_reload {
            &[RELOAD_SHA256]
        } else {
            &[]
        })),
        title = esc_text(&title),
        title_attr = esc_attr(&title),
        base = BASE_CSS,
        rt = RUNTIME_CSS,
        sections = sections,
        total = deck.slides.len(),
        json = json,
        js = RUNTIME_JS,
        reload = if opts.live_reload {
            format!("<script>{RELOAD_JS}</script>")
        } else {
            String::new()
        },
    );
    Rendered {
        text,
        diagnostics: cx.finish(),
    }
}
