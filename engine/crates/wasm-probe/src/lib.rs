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
