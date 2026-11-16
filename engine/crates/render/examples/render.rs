//! Renders a deck to PNG per slide, one PDF and a GIF per animated slide.
//! cargo run -p s-md-slides-render --release --example render -- deck.md out-dir

use s_md_slides::{Asset, AssetResolver, Deck};
use s_md_slides_render::{Fonts, Frame, NoProgress, gif, jpeg, pdf, png};
use std::path::{Path, PathBuf};

/// Reads assets from the deck's folder only.
struct Folder(PathBuf);

impl AssetResolver for Folder {
    fn resolve(&self, path: &str) -> Option<Asset> {
        let root = self.0.canonicalize().ok()?;
        let full = root.join(path).canonicalize().ok()?;
        full.starts_with(&root).then_some(())?;
        Some(Asset {
            bytes: std::fs::read(full).ok()?,
            mime: String::new(),
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(deck), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: render <deck.md> <out-dir>");
        std::process::exit(2);
    };
    let out = Path::new(out);
    std::fs::create_dir_all(out).unwrap();
    let mut d = Deck::parse(&std::fs::read_to_string(deck).unwrap()).deck;
    let folder = Folder(
        Path::new(deck)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf(),
    );
    for x in d.load_theme(&folder) {
        eprintln!("theme: {}", x.message);
    }
    let fonts = Fonts::bundled();
    let report = |what: &str, diags: &[s_md_slides::Diagnostic]| {
        for x in diags {
            eprintln!("{what}: {} {}", x.code, x.message);
        }
    };
    for i in 0..d.slides.len() {
        let t = std::time::Instant::now();
        let p = png(&d, i, Frame::End, 1.0, &folder, &fonts).unwrap();
        eprintln!("slide {} png in {:?}", i + 1, t.elapsed());
        report("png", &p.diagnostics);
        std::fs::write(out.join(format!("slide-{}.png", i + 1)), p.bytes).unwrap();
        if d.timeline(i).is_some_and(|t| t.duration > 0) {
            let g = gif(&d, i, 15, 0.5, true, &folder, &fonts, &mut NoProgress).unwrap();
            std::fs::write(out.join(format!("slide-{}.gif", i + 1)), g.bytes).unwrap();
        }
    }
    let j = jpeg(&d, 0, Frame::End, 1.0, 90, &folder, &fonts).unwrap();
    std::fs::write(out.join("slide-1.jpg"), j.bytes).unwrap();
    let t = std::time::Instant::now();
    let doc = pdf(&d, &folder, &fonts, &mut NoProgress).unwrap();
    eprintln!("pdf in {:?}", t.elapsed());
    report("pdf", &doc.diagnostics);
    std::fs::write(out.join("deck.pdf"), doc.bytes).unwrap();
}
