//! `s-md-slides-render`: native export for s-md-slides decks. Text is laid out with
//! parley using bundled Noto fonts (plus system fonts if asked), then drawn as PNG,
//! JPEG and GIF through resvg, or as PDF with selectable text through krilla.
//!
//! ```no_run
//! use s_md_slides::{Asset, AssetResolver, Deck};
//! use s_md_slides_render::{Fonts, Frame, NoProgress, pdf, png};
//!
//! struct Folder;
//! impl AssetResolver for Folder {
//!     fn resolve(&self, path: &str) -> Option<Asset> {
//!         std::fs::read(path).ok().map(|bytes| Asset { bytes, mime: String::new() })
//!     }
//! }
//!
//! let deck = Deck::parse("# Hello\n").deck;
//! let fonts = Fonts::bundled();
//! let image = png(&deck, 0, Frame::End, 2.0, &Folder, &fonts).unwrap();
//! let doc = pdf(&deck, &Folder, &fonts, &mut NoProgress).unwrap();
//! # let _ = (image, doc);
//! ```

mod flatten;
mod fonts;
mod gif;
mod graphics;
mod layout;
mod pdf;
mod raster;

pub use fonts::Fonts;
pub use s_md_slides::export::Frame;

use s_md_slides::{AssetResolver, Deck, Diagnostic, EngineError, Severity};

/// What a progress callback asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    Continue,
    Cancel,
}

/// Progress for long exports (PDF pages, GIF frames). Returning `Cancel` stops the
/// export within one page or frame with `E-CANCELLED` (FR-EXP-06).
pub trait Progress {
    fn report(&mut self, done: u32, total: u32) -> Control;
}

impl<F: FnMut(u32, u32) -> Control> Progress for F {
    fn report(&mut self, done: u32, total: u32) -> Control {
        self(done, total)
    }
}

/// Progress that never cancels.
pub struct NoProgress;

impl Progress for NoProgress {
    fn report(&mut self, _: u32, _: u32) -> Control {
        Control::Continue
    }
}

/// Exported bytes plus the problems found on the way (missing assets, diagrams that
/// could not be drawn).
#[derive(Debug, Clone)]
pub struct Output {
    pub bytes: Vec<u8>,
    pub diagnostics: Vec<Diagnostic>,
}

fn err(code: &'static str, message: impl Into<String>) -> EngineError {
    EngineError {
        code,
        message: message.into(),
        element: None,
        slide: None,
    }
}

fn no_slide(index: usize) -> EngineError {
    err("E-NOT-FOUND", format!("No slide {index}."))
}

/// Slide `index` at `frame` as one SVG with every glyph as an outline path, ready for
/// any SVG renderer without fonts.
pub fn flatten_svg(
    deck: &Deck,
    index: usize,
    frame: Frame,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Result<Output, EngineError> {
    let scene =
        flatten::scene(deck, index, frame, resolver, fonts).ok_or_else(|| no_slide(index))?;
    let svg = flatten::to_svg(&scene, fonts);
    Ok(Output {
        bytes: svg.into_bytes(),
        diagnostics: scene.diagnostics,
    })
}

fn pixmap(
    deck: &Deck,
    index: usize,
    frame: Frame,
    scale: f32,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Result<(resvg::tiny_skia::Pixmap, Vec<Diagnostic>), EngineError> {
    pixmap_with(
        deck,
        index,
        &flatten::states(deck, index, frame),
        scale,
        resolver,
        fonts,
    )
}

fn pixmap_with(
    deck: &Deck,
    index: usize,
    states: &std::collections::BTreeMap<String, s_md_slides::State>,
    scale: f32,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Result<(resvg::tiny_skia::Pixmap, Vec<Diagnostic>), EngineError> {
    if !(scale.is_finite() && scale > 0.0 && scale <= 8.0) {
        return Err(err("E-ATTR-VALUE", "scale must be above 0 and at most 8."));
    }
    let scene =
        flatten::scene_with(deck, index, states, resolver, fonts).ok_or_else(|| no_slide(index))?;
    let svg = flatten::to_svg(&scene, fonts);
    let pm = raster::pixmap(&svg, scale, fonts).map_err(|e| err("E-RENDER", e))?;
    Ok((pm, scene.diagnostics))
}

/// Slide `index` as a PNG at `scale` (2.0 gives 3840 x 2160 for 16:9) (FR-EXP-04).
pub fn png(
    deck: &Deck,
    index: usize,
    frame: Frame,
    scale: f32,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Result<Output, EngineError> {
    let (pm, diagnostics) = pixmap(deck, index, frame, scale, resolver, fonts)?;
    Ok(Output {
        bytes: raster::png(&pm).map_err(|e| err("E-RENDER", e))?,
        diagnostics,
    })
}

/// Slide `index` as a JPEG at `scale` and `quality` (1 to 100) (FR-EXP-04).
pub fn jpeg(
    deck: &Deck,
    index: usize,
    frame: Frame,
    scale: f32,
    quality: u8,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
) -> Result<Output, EngineError> {
    let (pm, diagnostics) = pixmap(deck, index, frame, scale, resolver, fonts)?;
    Ok(Output {
        bytes: raster::jpeg(&pm, quality).map_err(|e| err("E-RENDER", e))?,
        diagnostics,
    })
}

/// Pause after each step in a GIF, in milliseconds.
pub const GIF_HOLD_MS: f64 = 600.0;

/// Slide `index`'s whole animation as a GIF at `fps` frames per second (FR-EXP-05),
/// with a [`GIF_HOLD_MS`] pause after each step.
/// A slide with no animation gives one frame and a `W-GIF-STATIC` warning.
#[allow(clippy::too_many_arguments)]
pub fn gif(
    deck: &Deck,
    index: usize,
    fps: u32,
    scale: f32,
    looped: bool,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
    progress: &mut dyn Progress,
) -> Result<Output, EngineError> {
    let t = deck.timeline(index).ok_or_else(|| no_slide(index))?;
    let fps = fps.clamp(1, 60);
    let step_ms = 1000.0 / fps as f64;
    // A GIF has no clicks, so each step is followed by a pause. Without it, steps made
    // of `appear` (zero duration) would all show in the first frame.
    let mut plan: Vec<(f64, usize, f64)> = Vec::new(); // (gif start, step, length played)
    let mut v = 0.0;
    for (k, st) in t.steps.iter().enumerate() {
        plan.push((v, k, st.duration as f64));
        v += st.duration as f64 + GIF_HOLD_MS;
    }
    let length = if t.duration == 0 { 0.0 } else { v };
    let at = |g: f64| {
        let (gs, k, len) = plan
            .iter()
            .rev()
            .find(|(gs, _, _)| *gs <= g)
            .copied()
            .unwrap_or((0.0, 0, 0.0));
        t.state_at_step(k, (g - gs).min(len))
    };
    let total = ((length / step_ms).ceil() as u32 + 1).min(10_000);
    let mut diagnostics = Vec::new();
    if t.duration == 0 {
        diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: "W-GIF-STATIC",
            message: format!(
                "Slide {} has no animation, so the GIF has one frame.",
                index + 1
            ),
            line: 1,
            column: 1,
            element: None,
            slide: Some(index),
        });
    }
    let mut frames = Vec::with_capacity(total as usize);
    let (mut w, mut h) = (0, 0);
    for k in 0..total {
        if progress.report(k, total) == Control::Cancel {
            return Err(err("E-CANCELLED", "The export was cancelled."));
        }
        let (pm, d) = pixmap_with(deck, index, &at(k as f64 * step_ms), scale, resolver, fonts)?;
        if k == 0 {
            diagnostics.extend(d);
        }
        (w, h) = (pm.width(), pm.height());
        if w > u16::MAX as u32 || h > u16::MAX as u32 {
            return Err(err(
                "E-RENDER",
                "The GIF would be larger than 65535 pixels.",
            ));
        }
        // GIF wants straight (not premultiplied) RGBA.
        frames.push(
            pm.pixels()
                .iter()
                .flat_map(|p| {
                    let c = p.demultiply();
                    [c.red(), c.green(), c.blue(), c.alpha()]
                })
                .collect(),
        );
    }
    if progress.report(total, total) == Control::Cancel {
        return Err(err("E-CANCELLED", "The export was cancelled."));
    }
    let delay = (100 / fps).max(1) as u16;
    let bytes =
        gif::encode(frames, w as u16, h as u16, delay, looped).map_err(|e| err("E-RENDER", e))?;
    Ok(Output { bytes, diagnostics })
}

/// The whole deck as a PDF, one page per slide at its end state, with selectable and
/// searchable text (FR-EXP-03).
pub fn pdf(
    deck: &Deck,
    resolver: &dyn AssetResolver,
    fonts: &Fonts,
    progress: &mut dyn Progress,
) -> Result<Output, EngineError> {
    let mut doc = krilla::Document::new();
    let mut cache = pdf::FontCache::new();
    let mut diagnostics = Vec::new();
    let total = deck.slides.len() as u32;
    for i in 0..deck.slides.len() {
        if progress.report(i as u32, total) == Control::Cancel {
            return Err(err("E-CANCELLED", "The export was cancelled."));
        }
        let scene =
            flatten::scene(deck, i, Frame::End, resolver, fonts).ok_or_else(|| no_slide(i))?;
        let settings = krilla::page::PageSettings::from_wh(scene.width as f32, scene.height as f32)
            .ok_or_else(|| err("E-RENDER", "bad page size"))?;
        let mut page = doc.start_page_with(settings);
        let mut surface = page.surface();
        pdf::draw_scene(&mut surface, &scene, fonts, &mut cache).map_err(|e| err("E-RENDER", e))?;
        surface.finish();
        page.finish();
        diagnostics.extend(scene.diagnostics);
    }
    if progress.report(total, total) == Control::Cancel {
        return Err(err("E-CANCELLED", "The export was cancelled."));
    }
    let bytes = doc
        .finish()
        .map_err(|e| err("E-RENDER", format!("{e:?}")))?;
    Ok(Output { bytes, diagnostics })
}
