//! Ops must never panic. A failed op leaves the deck unchanged, and a successful op's
//! inverse restores the exact bytes (NFR-05, FR-OPS-02, FR-OPS-03).
//! Input: deck text, a 0 byte, then 3 bytes per op. Run:
//! cargo +nightly fuzz run fuzz_ops -- -runs=1000000

#![no_main]

use libfuzzer_sys::fuzz_target;
use s_md_slides::{Deck, Op, ZStep, ZTo};
use serde_json::{Map, Value, json};

const BODIES: &[&str] = &["", "# Title", "a\n---\nb", "<!-- text id=x -->", "```\n---", "مرحبا", "- item\n\n"];
const KEYS: &[&str] = &["x", "w", "color", "locked", "dir", "id", "bad key", "kind"];
const VALS: &[&str] = &["10", "-3.5", "#fff", "rtl", "a b", "abc", "", "line"];

fn op_from(d: &Deck, k: u8, a: u8, b: u8) -> Op {
    let ids: Vec<String> = d.elements().map(|e| e.id.clone()).collect();
    let id = if ids.is_empty() || a == 255 { "nope".to_string() } else { ids[a as usize % ids.len()].clone() };
    let slide = a as usize % (d.slides.len() + 1);
    let n = b as f64 - 64.0;
    let text = BODIES[b as usize % BODIES.len()].to_string();
    let mut set = Map::new();
    set.insert(KEYS[b as usize % KEYS.len()].into(), Value::from(VALS[a as usize % VALS.len()]));
    match k % 20 {
        0 => Op::SetGeometry { id, x: Some(n), y: None, w: Some(n.abs()), h: None, rot: (b % 3 == 0).then_some(n) },
        1 => Op::SetBody { id, text },
        2 => Op::RemoveElement { id },
        3 => Op::SetAttrs { id, set, remove: vec![KEYS[a as usize % KEYS.len()].into()] },
        4 => Op::DuplicateElement { id, dx: Some(n), dy: None },
        5 => Op::SetZ { id, to: if b % 2 == 0 { ZTo::Index(b as usize % 5) } else { ZTo::Step(ZStep::Forward) } },
        6 => Op::AddElement { slide, index: (b % 2 == 0).then_some(b as usize % 4), kind: Some("text".into()),
            attrs: serde_json::from_value(json!({"x": 1, "y": 2, "w": 3, "h": 4})).ok(), body: Some(text), raw: None },
        7 => Op::AddElement { slide, index: None, kind: None, attrs: None, body: None, raw: Some(text) },
        8 => Op::SetNotes { slide, text: (b % 3 != 0).then_some(text) },
        9 => Op::SetSlideAttrs { slide, set, remove: Vec::new() },
        10 => Op::SetFrontMatter { key: KEYS[b as usize % KEYS.len()].into(), value: (a % 3 != 0).then(|| VALS[a as usize % VALS.len()].into()) },
        11 => Op::AddSlide { index: slide, layout: d.layouts.first().filter(|_| b % 2 == 0).map(|l| l.id.clone()), raw: None },
        12 => Op::AddSlide { index: slide, layout: None, raw: Some(text) },
        13 => Op::RemoveSlide { index: slide },
        14 => Op::DuplicateSlide { index: slide },
        15 => Op::MoveSlide { from: slide, to: b as usize % (d.slides.len() + 1) },
        16 => Op::Group { ids: vec![id, ids.get(b as usize % ids.len().max(1)).cloned().unwrap_or_default()], id: None },
        17 => Op::Ungroup { id },
        18 => Op::AddDefinition { raw: format!("<!-- master id=m{b} -->\n{text}") },
        _ => Op::Replace { at: a as usize * 7, old: text, new: VALS[b as usize % VALS.len()].into() },
    }
}

fuzz_target!(|data: &[u8]| {
    let split = data.iter().position(|&b| b == 0).unwrap_or(data.len());
    let Ok(src) = std::str::from_utf8(&data[..split]) else { return };
    let mut d = Deck::parse(src).deck;
    for chunk in data.get(split + 1..).unwrap_or(&[]).chunks_exact(3).take(16) {
        let op = op_from(&d, chunk[0], chunk[1], chunk[2]);
        let before = d.text().to_string();
        match d.apply(op.clone()) {
            Ok(inverse) => {
                let mut undo = d.clone();
                undo.apply(inverse).unwrap_or_else(|e| panic!("inverse of {op:?} failed: {e}"));
                assert_eq!(undo.text(), before, "undo of {op:?}");
            }
            Err(_) => assert_eq!(d.text(), before, "failed {op:?} changed the deck"),
        }
    }
});
