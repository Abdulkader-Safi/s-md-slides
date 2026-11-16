//! PNG and JPEG through resvg (FR-EXP-04).

use crate::fonts::Fonts;
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;

/// Draw an SVG at `scale`. Fonts are only needed for text inside Mermaid and math
/// images; slide text is already outlines.
pub(crate) fn pixmap(svg: &str, scale: f32, fonts: &Fonts) -> Result<Pixmap, String> {
    let opt = usvg::Options {
        fontdb: fonts.inner.borrow().db.clone(),
        ..Default::default()
    };
    let tree = usvg::Tree::from_str(svg, &opt).map_err(|e| e.to_string())?;
    let size = tree.size();
    let (w, h) = (
        (size.width() * scale).round() as u32,
        (size.height() * scale).round() as u32,
    );
    let mut pm = Pixmap::new(w.max(1), h.max(1)).ok_or("image too large")?;
    resvg::render(&tree, Transform::from_scale(scale, scale), &mut pm.as_mut());
    Ok(pm)
}

pub(crate) fn png(pm: &Pixmap) -> Result<Vec<u8>, String> {
    pm.encode_png().map_err(|e| e.to_string())
}

/// RGB on white (JPEG has no alpha).
pub(crate) fn jpeg(pm: &Pixmap, quality: u8) -> Result<Vec<u8>, String> {
    let mut rgb = Vec::with_capacity((pm.width() * pm.height() * 3) as usize);
    for p in pm.pixels() {
        let c = p.demultiply();
        let a = c.alpha() as u32;
        for v in [c.red(), c.green(), c.blue()] {
            rgb.push(((v as u32 * a + 255 * (255 - a)) / 255) as u8);
        }
    }
    let mut out = Vec::new();
    let enc = jpeg_encoder::Encoder::new(&mut out, quality.clamp(1, 100));
    enc.encode(
        &rgb,
        pm.width() as u16,
        pm.height() as u16,
        jpeg_encoder::ColorType::Rgb,
    )
    .map_err(|e| e.to_string())?;
    Ok(out)
}
