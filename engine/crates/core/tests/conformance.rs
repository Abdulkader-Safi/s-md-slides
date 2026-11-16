//! Conformance vectors (FR-PKG-03): one JSON file per case in `engine/conformance/`.
//! Any implementation (the npm WASM wrapper, a TypeScript port) proves it matches the
//! engine by passing every file. This test runs them against the Rust core.
//!
//! Regenerate after a deliberate change, then review the diff:
//! UPDATE_CONFORMANCE=1 cargo test -p s-md-slides --test conformance
//!
//! Each file:
//! - `name`, `covers` (requirement ids), `input` (deck text)
//! - `ops`: applied one by one; every inverse, applied in reverse, must restore `input`
//! - `expect_text`: the text after the ops (the input itself when there are none)
//! - `expect_error`: when set, the ops run as one batch and must fail with this code,
//!   leaving the text unchanged
//! - `expect_read_only`, `expect_diagnostics` (severity, code, line, column)
//! - `expect_model` (optional): the parsed deck as JSON
//! - `queries` (optional): `elements_at` and `search` with their expected answers
//! - `timeline_samples` (optional): `state_at(t)` on a slide

use s_md_slides::{Deck, Op};
use serde_json::{Value, json};
use std::path::PathBuf;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../conformance")
}

/// Run a case's input and ops through the engine and record every expectation.
fn compute(name: &str, covers: &[&str], input: &str, ops: Value, extra: Value) -> Value {
    let p = Deck::parse(input);
    let deck0 = p.deck.clone();
    let diags: Vec<Value> = p
        .diagnostics
        .iter()
        .map(|d| json!({ "severity": d.severity, "code": d.code, "line": d.line, "column": d.column }))
        .collect();
    let ops_list: Vec<Op> = serde_json::from_value(ops.clone()).expect("ops parse");
    let mut deck = p.deck;
    let mut out = json!({
        "name": name,
        "covers": covers,
        "input": input,
        "ops": ops,
        "expect_read_only": deck.is_read_only(),
        "expect_diagnostics": diags,
    });
    if extra.get("expect_error").is_some() {
        let err = deck.apply_batch(ops_list).expect_err("this case must fail");
        out["expect_error"] = json!(err.code);
        out["expect_text"] = json!(deck.text());
    } else {
        for op in ops_list {
            deck.apply(op).expect("op applies");
        }
        out["expect_text"] = json!(deck.text());
    }
    if extra.get("model").is_some() {
        out["expect_model"] = serde_json::to_value(&deck0).unwrap();
    }
    if let Some(qs) = extra.get("queries").and_then(Value::as_array) {
        let answered: Vec<Value> = qs
            .iter()
            .map(|q| {
                if let Some(a) = q.get("elements_at") {
                    let a: Vec<f64> = serde_json::from_value(a.clone()).unwrap();
                    json!({ "elements_at": a, "expect": deck0.elements_at(a[0] as usize, a[1], a[2]) })
                } else {
                    let s = q["search"].as_str().unwrap();
                    json!({ "search": s, "expect": deck0.search(s) })
                }
            })
            .collect();
        out["queries"] = json!(answered);
    }
    if let Some(ts) = extra.get("timeline").and_then(Value::as_array) {
        let samples: Vec<Value> = ts
            .iter()
            .map(|t| {
                let (slide, at) = (t[0].as_u64().unwrap() as usize, t[1].as_f64().unwrap());
                json!({ "slide": slide, "t": at, "states": deck0.timeline(slide).unwrap().state_at(at) })
            })
            .collect();
        out["timeline_samples"] = json!(samples);
    }
    out
}

/// The cases. Every FR-FMT, FR-OPS and FR-ANIM-04 acceptance check has at least one.
fn cases() -> Vec<Value> {
    let mut v = Vec::new();
    let mut add = |name: &str, covers: &[&str], input: &str, ops: Value, extra: Value| {
        v.push(compute(name, covers, input, ops, extra));
    };
    let sample = "---\ns-md-slides: 1\ntitle: Q3 review\ncanvas: 16:9\ndir: auto\nteam: growth\n---\n\n<!-- master id=brand -->\n<!-- image id=logo x=1720 y=40 w=160 h=60 -->\n![Brand logo](assets/logo.svg)\n\n<!-- layout id=title-body master=brand -->\n<!-- placeholder id=title x=120 y=80 w=1680 h=200 -->\n<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->\n\n---\n\n<!-- slide layout=title-body transition=fade -->\n\n<!-- text id=t1 x=120 y=80 w=1680 h=200 enter=fade -->\n# Quarterly review\n\n<!-- text id=t2 x=120 y=320 w=760 h=560 dir=rtl enter=fly-right delay=300 -->\n- مبيعات أعلى بنسبة ١٢٪\n- Second point\n\n<!-- shape id=s1 kind=arrow x=900 y=600 w=200 h=0 stroke=#e11 -->\n\n<!-- notes -->\nMention the Kuwait numbers first.\n";
    let none = json!([]);
    let mv = |id: &str, x: f64| json!({ "op": "set_geometry", "id": id, "x": x });

    add(
        "fmt-01-sample-deck-model",
        &["FR-FMT-01"],
        sample,
        none.clone(),
        json!({ "model": true }),
    );
    for (n, input) in [
        ("lf", "<!-- text id=t1 x=1 y=2 w=3 h=4 -->\nHello\n"),
        (
            "crlf",
            "---\r\ntitle: Windows\r\n---\r\n<!-- text id=t1 x=1 y=2 w=3 h=4 -->\r\nHello\r\n",
        ),
        (
            "bom",
            "\u{feff}<!-- text id=t1 x=1 y=2 w=3 h=4 -->\nHello\n",
        ),
        (
            "no-final-newline",
            "<!-- text id=t1 x=1 y=2 w=3 h=4 -->\nHello",
        ),
        ("empty", ""),
        (
            "fences-and-rules",
            "<!-- code id=c1 x=0 y=0 w=10 h=10 -->\n```md\n---\n<!-- text id=no x=1 y=1 w=1 h=1 -->\n```\n<!-- text id=t1 x=0 y=0 w=10 h=10 -->\n***\n___\n",
        ),
    ] {
        add(
            &format!("fmt-02-round-trip-{n}"),
            &["FR-FMT-02"],
            input,
            none.clone(),
            json!({}),
        );
    }
    let mut big = String::from("---\ntitle: Big\n---\n");
    for i in 0..50 {
        if i > 0 {
            big.push_str("\n---\n\n");
        }
        big.push_str(&format!("<!-- text id=a{i} x=100 y=100 w=800 h=200 -->\n# Slide {i}\n\n<!-- shape id=b{i} kind=rect x=10 y=10 w=50 h=50 -->\n"));
    }
    add(
        "fmt-03-one-move-one-line",
        &["FR-FMT-03"],
        &big,
        json!([mv("a27", 333.0)]),
        json!({}),
    );
    add(
        "fmt-04-unknowns-survive-a-move",
        &["FR-FMT-04"],
        "---\nfoo: bar\n---\n<!-- text id=t1 x=1 y=2 w=3 h=4 foo=bar -->\nHi\n<!-- widget id=w1 spin=fast -->\n",
        json!([mv("t1", 9.0), mv("w1", 5.0)]),
        json!({}),
    );
    add(
        "fmt-05-missing-version-is-1",
        &["FR-FMT-05"],
        "# no version\n",
        none.clone(),
        json!({}),
    );
    add(
        "fmt-05-newer-version-read-only",
        &["FR-FMT-05"],
        "---\ns-md-slides: 2\n---\n<!-- text id=t1 x=1 y=1 w=1 h=1 -->\n",
        json!([mv("t1", 5.0)]),
        json!({ "expect_error": true }),
    );
    add(
        "fmt-06-bad-attribute-keeps-the-deck",
        &["FR-FMT-06"],
        "<!-- text id=a x=1 y=1 w=1 h=1 -->\nA\n---\n<!-- text id=b x=1 y=1 w=1 h=1 -->\nB\n---\n<!-- text id=c x=abc y=1 w=1 h=1 -->\nC\n---\nD\n",
        none.clone(),
        json!({ "model": true }),
    );
    add(
        "fmt-06-many-mistakes",
        &["FR-FMT-06"],
        "---\ntitle: [unclosed\n---\n<!-- text id=a x=1 =y w=1 h=1 -->\n<!-- text id=a x=1 y=1 w=-1 h=1 -->\n<!-- notes -->\nn\n<!-- text id=z x=1 x=2 y=1 w=1 h=1 -->\n```\n---\n",
        none.clone(),
        json!({}),
    );
    let ids = "<!-- text x=1 y=1 w=100 h=100 -->\nA\n<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nB\n";
    add(
        "fmt-07-ids-assigned-on-load",
        &["FR-FMT-07"],
        ids,
        none.clone(),
        json!({ "model": true }),
    );
    add(
        "fmt-07-id-written-on-edit",
        &["FR-FMT-07"],
        ids,
        json!([{ "op": "set_body", "id": "t2", "text": "A2" }]),
        json!({}),
    );
    add(
        "fmt-08-plain-markdown-slides",
        &["FR-FMT-08"],
        "# One\n\ntext\n\n---\n\n# Two\n\n---\n\n# Three\n",
        none.clone(),
        json!({ "model": true }),
    );
    add(
        "fmt-08-edit-implicit-text",
        &["FR-FMT-08", "FR-FMT-07"],
        "# One\n---\n# Two\n",
        json!([mv("t2", 200.0)]),
        json!({}),
    );

    let ops_deck = "---\ntitle: Ops\n---\n\n<!-- master id=brand -->\n<!-- image id=logo x=1720 y=40 w=160 h=60 -->\n![logo](a.png)\n\n<!-- layout id=tb master=brand -->\n<!-- placeholder id=title x=120 y=80 w=1680 h=200 -->\n<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->\n\n---\n\n<!-- text id=t1 x=100 y=100 w=800 h=200 -->\n# One\n\n<!-- shape id=s1 kind=rect x=10 y=10 w=50 h=50 -->\n\n---\n\n<!-- slide bg=#000 -->\n<!-- text id=t2 x=0 y=0 w=100 h=100 -->\nTwo\n\n<!-- notes -->\nSay two.\n";
    let each: Vec<(&str, Value)> = vec![
        ("add-slide-empty", json!({"op": "add_slide", "index": 1})),
        (
            "add-slide-from-layout",
            json!({"op": "add_slide", "index": 2, "layout": "tb"}),
        ),
        (
            "add-slide-raw",
            json!({"op": "add_slide", "index": 0, "raw": "# Raw slide"}),
        ),
        ("remove-slide", json!({"op": "remove_slide", "index": 0})),
        (
            "duplicate-slide",
            json!({"op": "duplicate_slide", "index": 0}),
        ),
        (
            "move-slide",
            json!({"op": "move_slide", "from": 1, "to": 0}),
        ),
        (
            "add-element",
            json!({"op": "add_element", "slide": 0, "kind": "text", "attrs": {"x": 5, "y": 6, "w": 70, "h": 80, "color": "accent1"}, "body": "Hello"}),
        ),
        (
            "add-element-raw",
            json!({"op": "add_element", "slide": 1, "index": 0, "raw": "<!-- shape id=sq kind=rect x=1 y=1 w=1 h=1 -->"}),
        ),
        (
            "remove-element",
            json!({"op": "remove_element", "id": "s1"}),
        ),
        (
            "duplicate-element",
            json!({"op": "duplicate_element", "id": "t1", "dx": 20, "dy": 20}),
        ),
        (
            "move-element",
            json!({"op": "set_geometry", "id": "t1", "x": 300, "y": 40}),
        ),
        (
            "resize-element",
            json!({"op": "set_geometry", "id": "t1", "w": 900, "h": 300}),
        ),
        (
            "rotate-element",
            json!({"op": "set_geometry", "id": "s1", "rot": 45}),
        ),
        (
            "restyle-element",
            json!({"op": "set_attrs", "id": "t1", "set": {"color": "#fff", "locked": true, "size": 48}, "remove": []}),
        ),
        (
            "set-body",
            json!({"op": "set_body", "id": "t1", "text": "# New title"}),
        ),
        (
            "set-z-front",
            json!({"op": "set_z", "id": "t1", "to": "front"}),
        ),
        (
            "set-notes",
            json!({"op": "set_notes", "slide": 0, "text": "New notes"}),
        ),
        (
            "remove-notes",
            json!({"op": "set_notes", "slide": 1, "text": null}),
        ),
        (
            "set-slide-bg",
            json!({"op": "set_slide_attrs", "slide": 1, "set": {"bg": "#111"}}),
        ),
        (
            "set-slide-layout",
            json!({"op": "set_slide_attrs", "slide": 0, "set": {"layout": "tb"}}),
        ),
        (
            "set-slide-transition",
            json!({"op": "set_slide_attrs", "slide": 1, "set": {"transition": "push", "dur": 500}}),
        ),
        (
            "set-front-matter",
            json!({"op": "set_front_matter", "key": "canvas", "value": "4:5"}),
        ),
        (
            "remove-front-matter",
            json!({"op": "set_front_matter", "key": "title", "value": null}),
        ),
        (
            "add-definition",
            json!({"op": "add_definition", "raw": "<!-- master id=plain -->\n<!-- text id=foot x=0 y=1000 w=1920 h=80 -->\nFooter"}),
        ),
        (
            "remove-definition",
            json!({"op": "remove_definition", "id": "tb"}),
        ),
        ("group", json!({"op": "group", "ids": ["t1", "s1"]})),
    ];
    for (n, op) in each {
        add(
            &format!("ops-01-{n}"),
            &["FR-OPS-01", "FR-OPS-02"],
            ops_deck,
            json!([op]),
            json!({}),
        );
    }
    add(
        "ops-01-ungroup-and-group-move",
        &["FR-OPS-01", "FR-OPS-02", "FR-EL-10"],
        "<!-- text id=a x=100 y=100 w=100 h=100 -->\nA\n<!-- text id=b x=300 y=100 w=100 h=100 -->\nB\n<!-- group id=g1 items=a,b -->\n",
        json!([{ "op": "set_geometry", "id": "g1", "x": 150 }, { "op": "ungroup", "id": "g1" }]),
        json!({}),
    );
    add(
        "ops-03-unknown-id",
        &["FR-OPS-03"],
        ops_deck,
        json!([mv("nope", 1.0)]),
        json!({ "expect_error": true }),
    );
    add(
        "ops-03-negative-width",
        &["FR-OPS-03"],
        ops_deck,
        json!([{ "op": "set_geometry", "id": "t1", "w": -5 }]),
        json!({ "expect_error": true }),
    );
    add(
        "ops-03-body-adds-separator",
        &["FR-OPS-03"],
        ops_deck,
        json!([{ "op": "set_body", "id": "t1", "text": "a\n---\nb" }]),
        json!({ "expect_error": true }),
    );
    add(
        "ops-03-failed-batch-changes-nothing",
        &["FR-OPS-03", "FR-OPS-04"],
        ops_deck,
        json!([mv("t1", 1.0), { "op": "remove_element", "id": "nope" }]),
        json!({ "expect_error": true }),
    );
    let mut twenty = String::new();
    for i in 0..20 {
        twenty.push_str(&format!("<!-- text id=e{i} x=0 y=0 w=10 h=10 -->\n{i}\n"));
    }
    add(
        "ops-04-batch-of-20-moves",
        &["FR-OPS-04"],
        &twenty,
        json!([{ "op": "batch", "ops": (0..20).map(|i| mv(&format!("e{i}"), (i * 10) as f64)).collect::<Vec<_>>() }]),
        json!({}),
    );
    add(
        "ops-05-queries",
        &["FR-OPS-05"],
        "<!-- shape id=low kind=rect x=0 y=0 w=200 h=200 -->\n<!-- shape id=high kind=rect x=100 y=100 w=200 h=200 -->\n<!-- shape id=spin kind=rect x=500 y=500 w=200 h=20 rot=90 -->\n<!-- text id=t1 x=0 y=0 w=10 h=10 -->\nFind the Chart\n<!-- notes -->\nChart notes\n",
        none.clone(),
        json!({ "queries": [{ "elements_at": [0, 150, 150] }, { "elements_at": [0, 600, 590] }, { "elements_at": [0, 690, 510] }, { "search": "chart" }] }),
    );
    add(
        "anim-04-timeline",
        &["FR-ANIM-04", "FR-ANIM-01", "FR-ANIM-02"],
        "<!-- text id=title x=100 y=100 w=800 h=100 -->\n# Always there\n\n<!-- text id=a x=100 y=300 w=400 h=100 enter=fly-left delay=300 dur=500 order=1 -->\nA\n<!-- text id=b x=600 y=300 w=400 h=100 enter=fade order=1 -->\nB\n<!-- shape id=c kind=rect x=100 y=500 w=100 h=100 enter=scale order=2 exit=fade -->\n<!-- text id=auto x=0 y=0 w=10 h=10 enter=appear -->\nauto\n",
        none,
        json!({ "timeline": [[0, 0], [0, 300], [0, 550], [0, 800], [0, 1000], [0, 1200], [0, 1400], [0, 1600]] }),
    );
    v
}

#[test]
fn conformance_vectors_pass() {
    if std::env::var("UPDATE_CONFORMANCE").is_ok() {
        let _ = std::fs::remove_dir_all(dir());
        std::fs::create_dir_all(dir()).unwrap();
        for c in cases() {
            let name = c["name"].as_str().unwrap().to_string();
            std::fs::write(
                dir().join(format!("{name}.json")),
                serde_json::to_string_pretty(&c).unwrap() + "\n",
            )
            .unwrap();
        }
    }
    let mut files: Vec<_> = std::fs::read_dir(dir())
        .expect("conformance/ exists")
        .flatten()
        .map(|e| e.path())
        .collect();
    files.sort();
    assert!(!files.is_empty());
    let mut covered = std::collections::BTreeSet::new();
    for f in &files {
        let c: Value = serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        let name = c["name"].as_str().unwrap();
        for r in c["covers"].as_array().unwrap() {
            covered.insert(r.as_str().unwrap().to_string());
        }
        let input = c["input"].as_str().unwrap();
        let p = Deck::parse(input);
        let diags: Vec<Value> = p
            .diagnostics
            .iter()
            .map(|d| json!({ "severity": d.severity, "code": d.code, "line": d.line, "column": d.column }))
            .collect();
        assert_eq!(json!(diags), c["expect_diagnostics"], "{name}: diagnostics");
        assert_eq!(
            json!(p.deck.is_read_only()),
            c["expect_read_only"],
            "{name}: read-only"
        );
        assert_eq!(p.deck.text(), input, "{name}: unchanged save is identical");
        if let Some(m) = c.get("expect_model") {
            assert_eq!(&serde_json::to_value(&p.deck).unwrap(), m, "{name}: model");
        }
        if let Some(qs) = c.get("queries").and_then(Value::as_array) {
            for q in qs {
                let got = match q.get("elements_at") {
                    Some(a) => {
                        let a: Vec<f64> = serde_json::from_value(a.clone()).unwrap();
                        json!(p.deck.elements_at(a[0] as usize, a[1], a[2]))
                    }
                    None => json!(p.deck.search(q["search"].as_str().unwrap())),
                };
                assert_eq!(got, q["expect"], "{name}: query {q}");
            }
        }
        if let Some(ts) = c.get("timeline_samples").and_then(Value::as_array) {
            for t in ts {
                let tl = p
                    .deck
                    .timeline(t["slide"].as_u64().unwrap() as usize)
                    .unwrap();
                assert_eq!(
                    json!(tl.state_at(t["t"].as_f64().unwrap())),
                    t["states"],
                    "{name}: state_at({})",
                    t["t"]
                );
            }
        }
        let ops: Vec<Op> = serde_json::from_value(c["ops"].clone()).unwrap();
        let mut deck = p.deck;
        if let Some(code) = c.get("expect_error").and_then(Value::as_str) {
            assert_eq!(
                deck.apply_batch(ops).unwrap_err().code,
                code,
                "{name}: error code"
            );
            assert_eq!(deck.text(), input, "{name}: a failed op changes nothing");
            continue;
        }
        let mut undo = Vec::new();
        for op in ops {
            let json = serde_json::to_string(&op).unwrap();
            assert_eq!(
                serde_json::from_str::<Op>(&json).unwrap(),
                op,
                "{name}: op JSON round-trip"
            );
            undo.push(deck.apply(op).unwrap_or_else(|e| panic!("{name}: {e}")));
        }
        assert_eq!(
            deck.text(),
            c["expect_text"].as_str().unwrap(),
            "{name}: text after ops"
        );
        for inv in undo.into_iter().rev() {
            deck.apply(inv).unwrap();
        }
        assert_eq!(deck.text(), input, "{name}: undo restores the input");
    }
    for req in [
        "FR-FMT-01",
        "FR-FMT-02",
        "FR-FMT-03",
        "FR-FMT-04",
        "FR-FMT-05",
        "FR-FMT-06",
        "FR-FMT-07",
        "FR-FMT-08",
        "FR-OPS-01",
        "FR-OPS-02",
        "FR-OPS-03",
        "FR-OPS-04",
        "FR-OPS-05",
        "FR-ANIM-04",
    ] {
        assert!(covered.contains(req), "no vector covers {req}");
    }
}
