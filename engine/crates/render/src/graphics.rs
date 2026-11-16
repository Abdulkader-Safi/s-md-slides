//! Mermaid diagrams and math to SVG (FR-EL-06, FR-EL-07). Both renderers are young
//! crates, so each call runs behind `catch_unwind`: a panic becomes an error, and the
//! slide shows the source with a warning instead.

use std::panic::{AssertUnwindSafe, catch_unwind};

fn guarded<T>(f: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(f))
        .unwrap_or_else(|_| Err("the renderer stopped unexpectedly".into()))
}

/// A Mermaid source block to SVG.
pub(crate) fn mermaid(source: &str) -> Result<String, String> {
    guarded(|| {
        mermaid_rs_renderer::render(source).map_err(|e| {
            e.to_string()
                .lines()
                .next()
                .unwrap_or("invalid diagram")
                .to_string()
        })
    })
}

/// A LaTeX expression to SVG, glyphs embedded as paths (KaTeX fonts).
pub(crate) fn math(source: &str, color: &str, size: f64) -> Result<String, String> {
    use ratex_layout::{LayoutOptions, layout, to_display_list, validate_layout};
    use ratex_svg::{SvgOptions, render_to_svg};
    guarded(|| {
        let ast = ratex_parser::parser::parse(source).map_err(|e| format!("{e}"))?;
        let mut opts = LayoutOptions::default();
        if let Some(c) = ratex_types::color::Color::from_hex(color) {
            opts = opts.with_color(c);
        }
        let lbox = layout(&ast, &opts);
        validate_layout(&lbox).map_err(|e| e.to_string())?;
        let list = to_display_list(&lbox);
        Ok(render_to_svg(
            &list,
            &SvgOptions {
                font_size: size * 1.5,
                padding: 4.0,
                embed_glyphs: true,
                ..SvgOptions::default()
            },
        ))
    })
}
