//! PDF with selectable text (FR-EXP-03): krilla-svg draws shapes and images, and
//! parley's glyph runs go straight into krilla with their source text, as approved
//! after the M0 spike.

use crate::flatten::{Scene, matrix_attr};
use crate::fonts::Fonts;
use crate::layout::Run;
use krilla::color::rgb;
use krilla::geom::{PathBuilder, Point, Rect, Size, Transform};
use krilla::num::NormalizedF32;
use krilla::paint::{Fill, FillRule};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla_svg::{SurfaceExt, SvgSettings};
use resvg::usvg;
use std::collections::HashMap;

/// krilla fonts, one per font file and index.
pub(crate) type FontCache = HashMap<(u64, u32), Font>;

fn draw_svg(
    surface: &mut Surface,
    svg: &str,
    w: f32,
    h: f32,
    opt: &usvg::Options,
) -> Result<(), String> {
    let tree = usvg::Tree::from_str(svg, opt).map_err(|e| e.to_string())?;
    surface
        .draw_svg(
            &tree,
            Size::from_wh(w, h).ok_or("bad size")?,
            SvgSettings::default(),
        )
        .ok_or("svg did not draw")?;
    Ok(())
}

fn draw_runs(surface: &mut Surface, runs: &[Run], fonts: &mut FontCache) {
    for run in runs {
        let key = (run.font.data.id(), run.font.index);
        let font = match fonts.get(&key) {
            Some(f) => f.clone(),
            None => {
                let Some(f) = Font::new(run.font.data.data().to_vec().into(), run.font.index)
                else {
                    continue;
                };
                fonts.insert(key, f.clone());
                f
            }
        };
        let c = run.color;
        surface.set_fill(Some(Fill {
            paint: rgb::Color::new(c[0], c[1], c[2]).into(),
            opacity: NormalizedF32::new(c[3] as f32 / 255.0).unwrap_or(NormalizedF32::ONE),
            rule: FillRule::NonZero,
        }));
        let size = run.size;
        let glyphs: Vec<KrillaGlyph> = run
            .glyphs
            .iter()
            .map(|g| {
                KrillaGlyph::new(
                    GlyphId::new(g.id),
                    g.advance / size,
                    g.dx / size,
                    -g.dy / size,
                    0.0,
                    g.range.clone(),
                    None,
                )
            })
            .collect();
        let skewed = run.skew != 0.0;
        if skewed {
            let k = (run.skew.to_radians()).tan();
            // Skew about the run's baseline origin.
            surface.push_transform(&Transform::from_row(
                1.0,
                0.0,
                -k,
                1.0,
                k * run.y as f32,
                0.0,
            ));
        }
        surface.draw_glyphs(
            Point::from_xy(run.x as f32, run.y as f32),
            &glyphs,
            font,
            &run.text,
            size,
            false,
        );
        if skewed {
            surface.pop();
        }
    }
    surface.set_fill(None);
}

/// Draw one scene onto a PDF page surface.
pub(crate) fn draw_scene(
    surface: &mut Surface,
    scene: &Scene,
    fonts: &Fonts,
    cache: &mut FontCache,
) -> Result<(), String> {
    let (w, h) = (scene.width as f32, scene.height as f32);
    let opt = usvg::Options {
        fontdb: fonts.inner.borrow().db.clone(),
        ..Default::default()
    };
    let wrap = |inner: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">{inner}</svg>"
        )
    };
    draw_svg(surface, &wrap(&scene.background), w, h, &opt)?;
    for it in &scene.items {
        let op = if it.opacity != 1.0 {
            format!(" opacity=\"{}\"", it.opacity)
        } else {
            String::new()
        };
        if !it.svg.is_empty() {
            let g = format!(
                "<g transform=\"{}\"{op}>{}</g>",
                matrix_attr(&it.matrix),
                it.svg
            );
            draw_svg(surface, &wrap(&g), w, h, &opt)?;
        }
        if it.runs.is_empty() {
            continue;
        }
        let m = it.matrix;
        surface.push_transform(&Transform::from_row(
            m[0] as f32,
            m[1] as f32,
            m[2] as f32,
            m[3] as f32,
            m[4] as f32,
            m[5] as f32,
        ));
        let mut pushed = 1;
        if it.opacity < 1.0 {
            surface.push_opacity(
                NormalizedF32::new(it.opacity.clamp(0.0, 1.0) as f32).unwrap_or(NormalizedF32::ONE),
            );
            pushed += 1;
        }
        if let Some((cw, ch)) = it.clip {
            let mut pb = PathBuilder::new();
            if let Some(r) = Rect::from_xywh(0.0, 0.0, cw.max(0.01) as f32, ch.max(0.01) as f32) {
                pb.push_rect(r);
            }
            if let Some(path) = pb.finish() {
                surface.push_clip_path(&path, &FillRule::NonZero);
                pushed += 1;
            }
        }
        draw_runs(surface, &it.runs, cache);
        for _ in 0..pushed {
            surface.pop();
        }
    }
    Ok(())
}
