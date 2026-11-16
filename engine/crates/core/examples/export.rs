//! Writes an HTML presentation from a deck file, for trying exports before the CLI.
//! cargo run -p s-md-slides --example export -- deck.md out.html [--embed]

use s_md_slides::export::{Options, presentation_html};
use s_md_slides::{Asset, AssetResolver, Deck};
use std::path::{Path, PathBuf};

/// Reads assets from the deck's folder only.
struct Folder(PathBuf);

impl AssetResolver for Folder {
    fn resolve(&self, path: &str) -> Option<Asset> {
        let root = self.0.canonicalize().ok()?;
        let full = root.join(path).canonicalize().ok()?;
        if !full.starts_with(&root) {
            return None;
        }
        Some(Asset {
            bytes: std::fs::read(full).ok()?,
            mime: String::new(),
        })
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(deck), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("usage: export <deck.md> <out.html> [--embed]");
        std::process::exit(2);
    };
    let text = std::fs::read_to_string(deck).expect("deck readable");
    let mut d = Deck::parse(&text).deck;
    let folder = Folder(
        Path::new(deck)
            .parent()
            .unwrap_or(Path::new("."))
            .to_path_buf(),
    );
    let mut diags = d.load_theme(&folder);
    let r = presentation_html(
        &d,
        &Options {
            embed: args.iter().any(|a| a == "--embed"),
            resolver: Some(&folder),
            ..Default::default()
        },
    );
    diags.extend(d.diagnostics().iter().cloned());
    diags.extend(r.diagnostics);
    for x in &diags {
        eprintln!(
            "{deck}:{}:{}: {:?} {} {}",
            x.line, x.column, x.severity, x.code, x.message
        );
    }
    std::fs::write(out, r.text).expect("output writable");
}
