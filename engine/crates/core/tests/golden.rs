//! Golden decks in `tests/decks/`: every one round-trips byte for byte, and its parsed
//! model and diagnostics match the insta snapshot (FR-FMT-01, FR-FMT-02, NFR-03).

use s_md_slides::{Deck, Op};
use std::path::PathBuf;

fn decks() -> Vec<(String, String)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/decks");
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .expect("tests/decks exists")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "md"))
        .map(|p| {
            let name = p.file_stem().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

#[test]
fn golden_decks_round_trip_and_match_snapshots() {
    let all = decks();
    assert!(all.len() >= 15, "expected at least 15 golden decks, found {}", all.len());
    for (name, src) in all {
        let p = Deck::parse(&src);
        assert_eq!(p.deck.text(), src, "{name} did not round-trip");
        insta::assert_json_snapshot!(name.clone(), serde_json::json!({
            "deck": p.deck,
            "diagnostics": p.diagnostics,
        }));
    }
}

#[test]
fn golden_decks_undo_every_move() {
    for (name, src) in decks() {
        let mut d = Deck::parse(&src).deck;
        let ids: Vec<String> = d.elements().map(|e| e.id.clone()).collect();
        for id in ids {
            let op = Op::SetGeometry { id: id.clone(), x: Some(7.0), y: Some(9.5), w: None, h: None, rot: None };
            let undo = d.apply(op).unwrap_or_else(|e| panic!("{name} {id}: {e}"));
            d.apply(undo).unwrap();
            assert_eq!(d.text(), src, "{name}: undo of a move on {id} changed bytes");
        }
    }
}
