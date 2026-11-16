//! NFR-01: one 1920 x 1080 PNG.

use criterion::{Criterion, criterion_group, criterion_main};
use s_md_slides::{Asset, AssetResolver, Deck};
use s_md_slides_render::{Fonts, Frame, png};
use std::hint::black_box;

struct NoAssets;
impl AssetResolver for NoAssets {
    fn resolve(&self, _: &str) -> Option<Asset> {
        None
    }
}

fn benches(c: &mut Criterion) {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/pitch-16x9.md"
    ))
    .unwrap();
    let deck = Deck::parse(&text).deck;
    let fonts = Fonts::bundled();
    c.bench_function("one 1080p png", |b| {
        b.iter(|| {
            black_box(
                png(&deck, 0, Frame::End, 1.0, &NoAssets, &fonts)
                    .unwrap()
                    .bytes
                    .len(),
            )
        })
    });
}

criterion_group!(group, benches);
criterion_main!(group);
