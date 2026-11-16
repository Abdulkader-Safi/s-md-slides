//! Size gate only. Exposes the core through wasm-bindgen the way the npm package will,
//! so `scripts/wasm-size.sh wasm-probe` measures a realistic build.

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn parse(src: &str) -> String {
    let p = s_md_slides::Deck::parse(src);
    serde_json::json!({ "deck": p.deck, "diagnostics": p.diagnostics }).to_string()
}

#[wasm_bindgen]
pub fn apply(src: &str, op: &str) -> String {
    let mut deck = s_md_slides::Deck::parse(src).deck;
    let result = serde_json::from_str::<s_md_slides::Op>(op)
        .map_err(|e| e.to_string())
        .and_then(|op| deck.apply(op).map_err(|e| e.to_string()));
    match result {
        Ok(inv) => serde_json::json!({ "text": deck.text(), "inverse": inv }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

#[wasm_bindgen]
pub fn presentation(src: &str) -> String {
    let deck = s_md_slides::Deck::parse(src).deck;
    s_md_slides::export::presentation_html(&deck, &Default::default()).text
}

#[wasm_bindgen]
pub fn slide_svg(src: &str, slide: usize, at_ms: u32) -> String {
    let deck = s_md_slides::Deck::parse(src).deck;
    let frame = s_md_slides::export::Frame::At(at_ms);
    s_md_slides::export::slide_svg(&deck, slide, frame, &Default::default())
        .map(|r| r.text)
        .unwrap_or_default()
}

#[wasm_bindgen]
pub fn queries(src: &str, id: &str) -> String {
    let deck = s_md_slides::Deck::parse(src).deck;
    serde_json::json!({
        "style": deck.resolved_style(id),
        "timeline": deck.timeline(0),
        "assets": deck.assets(),
        "search": deck.search(id),
        "at": deck.elements_at(0, 1.0, 1.0),
        "html": s_md_slides::export::slide_html(&deck, 0, s_md_slides::export::Frame::End, &Default::default()).map(|r| r.text),
    })
    .to_string()
}
