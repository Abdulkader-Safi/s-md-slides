//! M0 spike: measures the WASM size of a core-shaped crate. Exercises the scanner,
//! serde-saphyr, pulldown-cmark, serde_json and an embedded runtime string so the
//! linker keeps them all.

use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

static RUNTIME: &str = include_str!("runtime-placeholder.js");

#[derive(serde::Deserialize, serde::Serialize, Default)]
struct Front {
    title: Option<String>,
    author: Option<String>,
    canvas: Option<String>,
    dir: Option<String>,
    #[serde(flatten)]
    extra: BTreeMap<String, serde_json::Value>,
}

#[wasm_bindgen]
pub fn parse(src: &str) -> String {
    let deck = spike_core::Deck::parse(src);
    let front: Front = deck
        .front
        .clone()
        .and_then(|r| serde_saphyr::from_str(&src[r]).ok())
        .unwrap_or_default();
    let html: Vec<String> = deck
        .slides
        .iter()
        .flat_map(|s| &s.elements)
        .map(|e| {
            let mut out = String::new();
            let parser = pulldown_cmark::Parser::new_ext(
                &src[e.body.clone()],
                pulldown_cmark::Options::ENABLE_TABLES
                    | pulldown_cmark::Options::ENABLE_STRIKETHROUGH
                    | pulldown_cmark::Options::ENABLE_TASKLISTS
                    | pulldown_cmark::Options::ENABLE_MATH,
            );
            pulldown_cmark::html::push_html(&mut out, parser);
            out
        })
        .collect();
    serde_json::json!({ "front": front, "deck": deck, "html": html }).to_string()
}

#[wasm_bindgen]
pub fn apply(src: &str, op_json: &str) -> String {
    let mut deck = spike_core::Deck::parse(src);
    let result = serde_json::from_str::<spike_core::Op>(op_json)
        .map_err(|e| e.to_string())
        .and_then(|op| deck.apply(op).map_err(|e| e.message));
    match result {
        Ok(inv) => serde_json::json!({ "text": deck.text(), "inverse": inv }).to_string(),
        Err(e) => serde_json::json!({ "error": e }).to_string(),
    }
}

#[wasm_bindgen]
pub fn runtime() -> String {
    RUNTIME.to_string()
}
