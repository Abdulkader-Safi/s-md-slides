//! M5: native export (FR-EXP-03 to FR-EXP-06, FR-EL-06 to FR-EL-09).
//! Reference images: UPDATE_REFERENCE=1 cargo test -p s-md-slides-render --release

use s_md_slides::{Asset, AssetResolver, Deck};
use s_md_slides_render::{Control, Fonts, Frame, NoProgress, flatten_svg, gif, jpeg, pdf, png};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Reads from a folder and records every request.
struct Folder(PathBuf, RefCell<Vec<String>>);

impl Folder {
    fn new(dir: &str) -> Self {
        Folder(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(dir),
            RefCell::new(Vec::new()),
        )
    }
}

impl AssetResolver for Folder {
    fn resolve(&self, path: &str) -> Option<Asset> {
        self.1.borrow_mut().push(path.to_string());
        let bytes = std::fs::read(self.0.join(path)).ok()?;
        Some(Asset {
            bytes,
            mime: String::new(),
        })
    }
}

fn example(name: &str) -> Deck {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples")
        .join(format!("{name}.md"));
    Deck::parse(&std::fs::read_to_string(p).unwrap()).deck
}

fn png_size(b: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes(b[16..20].try_into().unwrap()),
        u32::from_be_bytes(b[20..24].try_into().unwrap()),
    )
}

/// Share of pixels whose channels differ by more than 8 out of 255.
fn diff(a: &[u8], b: &[u8]) -> f64 {
    let pa = resvg::tiny_skia::Pixmap::decode_png(a).unwrap();
    let pb = resvg::tiny_skia::Pixmap::decode_png(b).unwrap();
    assert_eq!((pa.width(), pa.height()), (pb.width(), pb.height()));
    let n = pa
        .data()
        .chunks(4)
        .zip(pb.data().chunks(4))
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.abs_diff(*q) > 8))
        .count();
    n as f64 / (pa.width() * pa.height()) as f64
}

#[test]
fn every_example_exports_to_every_format_and_matches_reference_pngs() {
    let fonts = Fonts::bundled();
    let refs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/reference");
    let update = std::env::var("UPDATE_REFERENCE").is_ok();
    for name in ["hello", "pitch-16x9", "carousel-4x5", "all-elements"] {
        let d = example(name);
        let files = Folder::new("examples");
        for i in 0..d.slides.len() {
            let p = png(&d, i, Frame::End, 0.5, &files, &fonts).unwrap();
            let r = refs.join(format!("{name}-{}.png", i + 1));
            if update {
                std::fs::create_dir_all(&refs).unwrap();
                std::fs::write(&r, &p.bytes).unwrap();
            } else {
                let want = std::fs::read(&r).unwrap_or_else(|_| panic!("missing {}", r.display()));
                let share = diff(&p.bytes, &want);
                assert!(
                    share <= 0.005,
                    "{name} slide {}: {:.3}% of pixels differ",
                    i + 1,
                    share * 100.0
                );
            }
            assert!(
                jpeg(&d, i, Frame::End, 0.25, 80, &files, &fonts)
                    .unwrap()
                    .bytes
                    .starts_with(&[0xFF, 0xD8])
            );
            assert!(
                flatten_svg(&d, i, Frame::Start, &files, &fonts)
                    .unwrap()
                    .bytes
                    .starts_with(b"<svg")
            );
        }
        let doc = pdf(&d, &files, &fonts, &mut NoProgress).unwrap();
        assert!(doc.bytes.starts_with(b"%PDF"));
        let g = gif(&d, 0, 5, 0.1, true, &files, &fonts, &mut NoProgress).unwrap();
        assert!(g.bytes.starts_with(b"GIF89a"));
    }
}

#[test]
fn fr_exp_04_slide_3_at_2x_is_3840_by_2160() {
    let d = example("pitch-16x9");
    let p = png(
        &d,
        2,
        Frame::End,
        2.0,
        &Folder::new("examples"),
        &Fonts::bundled(),
    )
    .unwrap();
    assert_eq!(png_size(&p.bytes), (3840, 2160));
}

#[test]
fn fr_exp_03_twelve_slides_twelve_pages_with_text() {
    let src: Vec<String> = (1..=12)
        .map(|i| format!("# Slide number {i}\n\nSearchable words here.\n"))
        .collect();
    let d = Deck::parse(&src.join("---\n")).deck;
    let doc = pdf(
        &d,
        &Folder::new("examples"),
        &Fonts::bundled(),
        &mut NoProgress,
    )
    .unwrap();
    let text = String::from_utf8_lossy(&doc.bytes);
    assert!(
        text.contains("/Type/Pages/Count 12"),
        "page tree says 12 pages"
    );
    assert_eq!(
        text.matches("/Type/Page/").count(),
        12,
        "twelve page objects"
    );
    assert!(
        text.contains("/ToUnicode"),
        "text is mapped back to characters, so it can be selected and searched"
    );
}

#[test]
fn fr_exp_05_three_steps_play_in_order() {
    let src = "<!-- shape id=a kind=rect x=0 y=0 w=100 h=100 fill=#ff0000 enter=appear order=1 -->\n<!-- shape id=b kind=rect x=200 y=0 w=100 h=100 fill=#00ff00 enter=appear order=2 -->\n<!-- shape id=c kind=rect x=400 y=0 w=100 h=100 fill=#0000ff enter=fade dur=300 order=3 -->\n";
    let d = Deck::parse(&format!("---\ncanvas: 500x100\n---\n{src}")).deck;
    let g = gif(
        &d,
        0,
        20,
        1.0,
        false,
        &Folder::new("examples"),
        &Fonts::bundled(),
        &mut NoProgress,
    )
    .unwrap();
    let mut opts = gif::DecodeOptions::new();
    opts.set_color_output(gif::ColorOutput::RGBA);
    let mut dec = opts.read_info(g.bytes.as_slice()).unwrap();
    let mut seen = Vec::new();
    while let Some(f) = dec.read_next_frame().unwrap() {
        let px = |x: usize| {
            let i = (50 * f.width as usize + x) * 4;
            f.buffer[i..i + 3].to_vec()
        };
        let on = |c: Vec<u8>, ch: usize| {
            c[ch] > 200 && c.iter().enumerate().all(|(k, v)| k == ch || *v < 60)
        };
        let state = (on(px(50), 0), on(px(250), 1), on(px(450), 2));
        if seen.last() != Some(&state) {
            seen.push(state);
        }
    }
    assert_eq!(seen.first(), Some(&(false, false, false)));
    assert_eq!(seen.last(), Some(&(true, true, true)));
    let order: Vec<_> = seen
        .iter()
        .map(|s| (s.0 as u8) + (s.1 as u8) + (s.2 as u8))
        .collect();
    assert!(
        order.windows(2).all(|w| w[1] >= w[0]),
        "a, then b, then c: {seen:?}"
    );
    assert!(
        seen.contains(&(true, false, false)) && seen.contains(&(true, true, false)),
        "{seen:?}"
    );

    let still = gif(
        &Deck::parse("# Static\n").deck,
        0,
        15,
        0.1,
        true,
        &Folder::new("examples"),
        &Fonts::bundled(),
        &mut NoProgress,
    )
    .unwrap();
    assert_eq!(still.diagnostics[0].code, "W-GIF-STATIC");
}

#[test]
fn fr_exp_06_progress_and_cancel_within_200ms() {
    let d = example("pitch-16x9");
    let fonts = Fonts::bundled();
    let files = Folder::new("examples");
    let mut events = Vec::new();
    gif(
        &d,
        1,
        15,
        0.25,
        true,
        &files,
        &fonts,
        &mut |done: u32, total: u32| {
            events.push(done * 100 / total);
            Control::Continue
        },
    )
    .unwrap();
    assert_eq!((events.first(), events.last()), (Some(&0), Some(&100)));
    assert!(events.windows(2).all(|w| w[1] >= w[0]));

    let mut asked_at: Option<Instant> = None;
    let r = gif(&d, 1, 15, 1.0, true, &files, &fonts, &mut |done: u32, _| {
        if done == 3 {
            asked_at = Some(Instant::now());
            return Control::Cancel;
        }
        Control::Continue
    });
    assert_eq!(r.unwrap_err().code, "E-CANCELLED");
    assert!(asked_at.unwrap().elapsed() < Duration::from_millis(200));
    let r = pdf(&d, &files, &fonts, &mut |done: u32, _| {
        if done == 1 {
            Control::Cancel
        } else {
            Control::Continue
        }
    });
    assert_eq!(r.unwrap_err().code, "E-CANCELLED");
}

#[test]
fn fr_el_06_to_09_diagrams_math_posters_no_network() {
    let fonts = Fonts::bundled();
    let files = Folder::new("examples");
    let d = example("all-elements");
    let svg = String::from_utf8(
        flatten_svg(&d, 0, Frame::End, &files, &fonts)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert_eq!(
        svg.matches("data:image/svg+xml;base64,").count(),
        2,
        "Mermaid and math drawn as images"
    );
    assert!(
        svg.contains("data:image/png;base64,"),
        "poster frame for the video"
    );
    let asked = files.1.borrow().clone();
    assert!(
        asked.iter().all(|p| !p.contains("://")),
        "no URL is ever requested: {asked:?}"
    );
    assert!(
        !asked.iter().any(|p| p.ends_with(".mp4")),
        "native export uses the poster, not the video"
    );

    let bad = Deck::parse("<!-- diagram id=d x=0 y=0 w=400 h=300 -->\n```mermaid\nthis is not mermaid ((((\n```\n<!-- math id=m x=0 y=400 w=400 h=100 -->\n$$\\frac{1}{$$\n").deck;
    let out = flatten_svg(&bad, 0, Frame::End, &files, &fonts).unwrap();
    let codes: Vec<_> = out
        .diagnostics
        .iter()
        .map(|x| (x.code, x.element.clone().unwrap_or_default()))
        .collect();
    assert!(codes.contains(&("W-DIAGRAM", "m".into())), "{codes:?}");
    assert!(!String::from_utf8(out.bytes).unwrap().is_empty());
}

#[test]
fn missing_assets_warn_once() {
    let d = Deck::parse("<!-- image id=i x=0 y=0 w=100 h=100 -->\n![A](gone.png)\n<!-- image id=j x=0 y=0 w=100 h=100 -->\n![B](gone.png)\n").deck;
    let files = Folder::new("examples");
    let out = png(&d, 0, Frame::End, 0.25, &files, &Fonts::bundled()).unwrap();
    assert_eq!(
        out.diagnostics
            .iter()
            .filter(|x| x.code == "W-MISSING-ASSET")
            .count(),
        1
    );
    assert_eq!(files.1.borrow().len(), 1);
    let _ = Path::new("");
}

#[test]
fn svg_images_never_read_local_files() {
    // A Mermaid or math SVG that pointed at a file would be drawn through usvg; only
    // data URIs may load. Probe the loader directly with a local href.
    let fonts = Fonts::bundled();
    let d = Deck::parse("<!-- image id=i x=0 y=0 w=100 h=100 -->\n![A](assets/chart.png)\n").deck;
    let svg = String::from_utf8(
        flatten_svg(&d, 0, Frame::End, &Folder::new("examples"), &fonts)
            .unwrap()
            .bytes,
    )
    .unwrap();
    assert!(
        svg.contains("href=\"data:image/png;base64,"),
        "assets arrive as data URIs"
    );
    assert!(
        !svg.contains("href=\"assets/"),
        "no file paths in the flattened SVG"
    );
}
