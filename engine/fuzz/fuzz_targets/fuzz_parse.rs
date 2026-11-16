//! Parsing must never panic, and an unchanged deck must save byte for byte (NFR-05).
//! Run: cargo +nightly fuzz run fuzz_parse -- -runs=1000000

#![no_main]

use libfuzzer_sys::fuzz_target;
use s_md_slides::Deck;

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else { return };
    let p = Deck::parse(src);
    assert_eq!(p.deck.text(), src);
    let lines = src.lines().count().max(1) as u32 + 1;
    for d in &p.diagnostics {
        assert!(d.line >= 1 && d.line <= lines && d.column >= 1, "{d:?}");
    }
    for e in p.deck.elements() {
        let _ = p.deck.resolved_dir(e);
    }
    // Export never panics, and no raw HTML in a deck becomes a live script tag.
    let html = s_md_slides::export::presentation_html(&p.deck, &Default::default()).text;
    let slides = html.split("<script type=\"application/json\"").next().unwrap_or("");
    assert!(!slides.to_ascii_lowercase().contains("<script"), "script tag in slide markup");
    for i in 0..p.deck.slides.len().min(3) {
        let _ = s_md_slides::export::slide_svg(&p.deck, i, s_md_slides::export::Frame::At(250), &Default::default());
    }
});
