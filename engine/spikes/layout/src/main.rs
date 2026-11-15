//! M0 spike: lay out English, Arabic and mixed text with parley, flatten to SVG with
//! one `<text>` per glyph run, then render PNG with resvg and PDF with krilla-svg.
//! Writes `lines.json` (parley line breaks) for `compare.py` to check against Chrome.
//!
//! Usage: cargo run -p spike-layout --release -- <out-dir>

use parley::{
    Alignment, AlignmentOptions, BaseDirection, FontContext, FontFamily, Layout, LayoutContext,
    LineHeight, PositionedLayoutItem, StyleProperty,
};
use std::fmt::Write;
use std::sync::Arc;

#[derive(serde::Deserialize)]
struct Slide {
    width: f32,
    height: f32,
    font_size: f32,
    line_height: f32,
    boxes: Vec<TextBox>,
}

#[derive(serde::Deserialize)]
struct TextBox {
    id: String,
    x: f32,
    y: f32,
    w: f32,
    dir: String,
    text: String,
}

const FONTS: &[(&str, &[u8])] = &[
    ("Noto Sans", include_bytes!("../fonts/NotoSans-Regular.ttf")),
    (
        "Noto Sans Arabic",
        include_bytes!("../fonts/NotoSansArabic-Regular.ttf"),
    ),
];
const STACK: &str = "'Noto Sans', 'Noto Sans Arabic'";

fn family_of(data: &[u8]) -> &'static str {
    FONTS
        .iter()
        .find(|(_, d)| d.len() == data.len())
        .map_or("Noto Sans", |(n, _)| n)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn main() {
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: spike-layout <out-dir>"),
    );
    std::fs::create_dir_all(&out).unwrap();
    let here = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let slide: Slide =
        serde_json::from_str(&std::fs::read_to_string(here.join("slide.json")).unwrap()).unwrap();

    // Bundled fonts only, so native output does not depend on the machine.
    let mut fcx = FontContext {
        collection: parley::fontique::Collection::new(parley::fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        }),
        source_cache: Default::default(),
    };
    for (_, data) in FONTS {
        fcx.collection.register_fonts(data.to_vec().into(), None);
    }
    let mut lcx: LayoutContext<()> = LayoutContext::new();

    let mut svg = String::new();
    let mut lines_json = serde_json::Map::new();
    let mut layouts = Vec::new();
    writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
        w = slide.width,
        h = slide.height
    )
    .unwrap();
    writeln!(
        svg,
        r##"<rect width="100%" height="100%" fill="#ffffff"/>"##
    )
    .unwrap();

    for b in &slide.boxes {
        let mut builder = lcx.ranged_builder(&mut fcx, &b.text, 1.0, false);
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(STACK.into())));
        builder.push_default(StyleProperty::FontSize(slide.font_size));
        builder.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            slide.line_height,
        )));
        builder.set_base_direction(if b.dir == "rtl" {
            BaseDirection::Rtl
        } else {
            BaseDirection::Ltr
        });
        let mut layout: Layout<()> = builder.build(&b.text);
        layout.break_all_lines(Some(b.w));
        layout.set_alignment(Alignment::Start, AlignmentOptions::default());

        let mut lines = Vec::new();
        writeln!(svg, r#"<g id="{}">"#, b.id).unwrap();
        for line in layout.lines() {
            lines.push(serde_json::Value::String(
                b.text[line.text_range()].trim().to_string(),
            ));
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                // Text of this run on this line: the clusters it covers.
                let range = run
                    .run()
                    .visual_clusters()
                    .map(|c| c.text_range())
                    .reduce(|a, c| a.start.min(c.start)..a.end.max(c.end))
                    .unwrap_or(0..0);
                let range =
                    range.start.max(line.text_range().start)..range.end.min(line.text_range().end);
                let full = &b.text[range.clone()];
                let text = full.trim();
                if text.is_empty() {
                    continue;
                }
                let rtl = run.run().is_rtl();
                // Bidi rule L1 sends edge whitespace to the base level even inside an
                // override, so drop it and move x past whatever was on the visual left.
                let lead = full.len() - full.trim_start().len();
                let trail = full.len() - full.trim_end().len();
                let left_trim = if rtl {
                    range.end - trail..range.end
                } else {
                    range.start..range.start + lead
                };
                let shift: f32 = run
                    .run()
                    .visual_clusters()
                    .filter(|c| {
                        let r = c.text_range();
                        r.start >= left_trim.start && r.end <= left_trim.end && !r.is_empty()
                    })
                    .map(|c| c.advance())
                    .sum();
                let family = family_of(run.run().font().data.data());
                let y = b.y + run.baseline();
                // Each run has one bidi level, so x is its left edge. usvg 0.47 runs bidi
                // with a fixed LTR base and ignores `direction` and `unicode-bidi`, so RTL
                // runs get RLO ... PDF to keep parley's order, neutrals included. Keep the
                // spaces so the run keeps its parley width.
                let x = b.x + run.offset() + shift;
                writeln!(
                    svg,
                    r##"<text x="{x:.2}" y="{y:.2}" font-family="{family}" font-size="{fs}" xml:space="preserve" fill="#111111">{t}</text>"##,
                    fs = slide.font_size,
                    t = if rtl { format!("\u{202E}{}\u{202C}", esc(text)) } else { esc(text) },
                )
                .unwrap();
            }
        }
        writeln!(svg, "</g>").unwrap();
        lines_json.insert(b.id.clone(), serde_json::Value::Array(lines));
        layouts.push(layout);
    }
    svg.push_str("</svg>\n");
    std::fs::write(out.join("slide.svg"), &svg).unwrap();
    std::fs::write(
        out.join("lines.json"),
        serde_json::to_string_pretty(&lines_json).unwrap(),
    )
    .unwrap();

    // One usvg tree feeds both rasterizer and PDF writer.
    let mut opt = usvg::Options::default();
    for (_, data) in FONTS {
        opt.fontdb_mut().load_font_data(data.to_vec());
    }
    opt.fontdb = Arc::new(Arc::unwrap_or_clone(opt.fontdb));
    let tree = usvg::Tree::from_str(&svg, &opt).expect("svg parses");

    let t = std::time::Instant::now();
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(slide.width as u32, slide.height as u32).unwrap();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
        &mut pixmap.as_mut(),
    );
    pixmap.save_png(out.join("slide.png")).unwrap();
    eprintln!("png: {:?}", t.elapsed());

    let t = std::time::Instant::now();
    let mut doc = krilla::Document::new();
    let mut page = doc
        .start_page_with(krilla::page::PageSettings::from_wh(slide.width, slide.height).unwrap());
    let mut surface = page.surface();
    krilla_svg::SurfaceExt::draw_svg(
        &mut surface,
        &tree,
        krilla::geom::Size::from_wh(slide.width, slide.height).unwrap(),
        krilla_svg::SvgSettings::default(),
    )
    .expect("svg draws");
    surface.finish();
    page.finish();
    std::fs::write(out.join("slide.pdf"), doc.finish().expect("pdf builds")).unwrap();
    eprintln!("pdf: {:?}", t.elapsed());

    // Second PDF path: parley glyphs straight into krilla, with parley's cluster
    // ranges, so the PDF text layer matches the source text.
    let t = std::time::Instant::now();
    let kfonts: Vec<krilla::text::Font> = FONTS
        .iter()
        .map(|(_, d)| krilla::text::Font::new(d.to_vec().into(), 0).unwrap())
        .collect();
    let mut doc = krilla::Document::new();
    let mut page = doc
        .start_page_with(krilla::page::PageSettings::from_wh(slide.width, slide.height).unwrap());
    let mut surface = page.surface();
    for (b, layout) in slide.boxes.iter().zip(&layouts) {
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let size = run.run().font_size();
                let data = run.run().font().data.data();
                let font = FONTS
                    .iter()
                    .position(|(_, d)| d.len() == data.len())
                    .unwrap_or(0);
                // Ligatures: the start cluster holds the glyph, continuation clusters hold
                // none. Merge their ranges so the glyph maps to all of its text.
                let mut clusters: Vec<(std::ops::Range<usize>, Vec<parley::Glyph>)> = run
                    .run()
                    .visual_clusters()
                    .map(|c| (c.text_range(), c.glyphs().collect()))
                    .filter(|(r, _)| {
                        r.start >= line.text_range().start && r.end <= line.text_range().end
                    })
                    .collect();
                clusters.sort_by_key(|(r, _)| r.start);
                let mut merged: Vec<(std::ops::Range<usize>, Vec<parley::Glyph>)> = Vec::new();
                for (r, g) in clusters {
                    match merged.last_mut() {
                        Some(last) if g.is_empty() => last.0.end = r.end,
                        _ => merged.push((r, g)),
                    }
                }
                if run.run().is_rtl() {
                    merged.reverse();
                }
                let glyphs: Vec<krilla::text::KrillaGlyph> = merged
                    .into_iter()
                    .flat_map(|(r, gs)| {
                        gs.into_iter().map(move |g| {
                            krilla::text::KrillaGlyph::new(
                                krilla::text::GlyphId::new(g.id),
                                g.advance / size,
                                g.x / size,
                                -g.y / size,
                                0.0,
                                r.clone(),
                                None,
                            )
                        })
                    })
                    .collect();
                surface.draw_glyphs(
                    krilla::geom::Point::from_xy(b.x + run.offset(), b.y + run.baseline()),
                    &glyphs,
                    kfonts[font].clone(),
                    &b.text,
                    size,
                    false,
                );
            }
        }
    }
    surface.finish();
    page.finish();
    std::fs::write(
        out.join("slide-direct.pdf"),
        doc.finish().expect("pdf builds"),
    )
    .unwrap();
    eprintln!("pdf direct: {:?}", t.elapsed());
}
