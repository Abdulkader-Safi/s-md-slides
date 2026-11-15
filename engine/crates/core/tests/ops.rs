//! M2: every op, its exact output, its inverse, and its errors (FR-OPS-01 to 04, FR-EL-10).

use s_md_slides::{Deck, Op, ZStep, ZTo};
use serde_json::json;

const DECK: &str = "---\ntitle: Ops\n---\n\n<!-- master id=brand -->\n<!-- image id=logo x=1720 y=40 w=160 h=60 -->\n![logo](a.png)\n\n<!-- layout id=tb master=brand -->\n<!-- placeholder id=title x=120 y=80 w=1680 h=200 -->\n<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->\n\n---\n\n<!-- text id=t1 x=100 y=100 w=800 h=200 -->\n# One\n\n<!-- shape id=s1 kind=rect x=10 y=10 w=50 h=50 -->\n\n---\n\n<!-- slide bg=#000 -->\n<!-- text id=t2 x=0 y=0 w=100 h=100 -->\nTwo\n\n<!-- notes -->\nSay two.\n";

fn op(v: serde_json::Value) -> Op {
    serde_json::from_value(v).unwrap()
}

/// Apply, check the result, undo, check the bytes are back.
fn run(src: &str, o: Op) -> String {
    let mut d = Deck::parse(src).deck;
    let json = serde_json::to_string(&o).unwrap();
    assert_eq!(
        serde_json::from_str::<Op>(&json).unwrap(),
        o,
        "JSON round-trip"
    );
    let undo = d.apply(o.clone()).unwrap_or_else(|e| panic!("{o:?}: {e}"));
    let after = d.text().to_string();
    let again = Deck::parse(&after);
    assert_eq!(again.deck.text(), after);
    d.apply(undo).unwrap();
    assert_eq!(d.text(), src, "undo of {o:?}");
    after
}

fn fails(src: &str, o: Op, code: &str) {
    let mut d = Deck::parse(src).deck;
    assert_eq!(d.apply(o).unwrap_err().code, code);
    assert_eq!(d.text(), src);
}

#[test]
fn add_slide_empty_layout_raw() {
    let out = run(DECK, op(json!({"op": "add_slide", "index": 1})));
    let d = Deck::parse(&out).deck;
    assert_eq!(d.slides.len(), 3);
    assert_eq!(d.slides[1].elements.len(), 0);
    assert_eq!(d.slides[2].elements[0].id, "t2");

    let out = run(
        DECK,
        op(json!({"op": "add_slide", "index": 2, "layout": "tb"})),
    );
    let d = Deck::parse(&out).deck;
    assert_eq!(d.slides[2].layout.as_deref(), Some("tb"));
    let els = &d.slides[2].elements;
    assert_eq!(els.len(), 2, "FR-STYLE-04: one element per placeholder");
    assert_eq!((els[0].x, els[0].y, els[1].y), (120.0, 80.0, 320.0));
    assert!(out.ends_with("Say two.\n---\n<!-- slide layout=tb -->\n\n<!-- text id=t3 x=120 y=80 w=1680 h=200 -->\n\n<!-- text id=t4 x=120 y=320 w=1680 h=640 -->\n"));

    let out = run(
        DECK,
        op(json!({"op": "add_slide", "index": 0, "raw": "# Raw slide"})),
    );
    let d = Deck::parse(&out).deck;
    assert_eq!(d.slides[0].elements[0].body, "# Raw slide\n");
    fails(
        DECK,
        op(json!({"op": "add_slide", "index": 0, "raw": "a\n---\nb"})),
        "E-RAW",
    );
    fails(
        DECK,
        op(json!({"op": "add_slide", "index": 0, "layout": "nope"})),
        "E-NOT-FOUND",
    );
    let out = run("", op(json!({"op": "add_slide", "index": 0})));
    assert_eq!(out, "<!-- slide -->\n");
    let out = run("# a", op(json!({"op": "add_slide", "index": 5})));
    assert_eq!(out, "# a\n---\n<!-- slide -->\n");
}

#[test]
fn remove_duplicate_move_slides() {
    let out = run(DECK, op(json!({"op": "remove_slide", "index": 0})));
    assert_eq!(Deck::parse(&out).deck.slides.len(), 1);
    let out = run(DECK, op(json!({"op": "remove_slide", "index": 1})));
    assert!(out.ends_with("<!-- shape id=s1 kind=rect x=10 y=10 w=50 h=50 -->\n\n"));

    let out = run(DECK, op(json!({"op": "duplicate_slide", "index": 0})));
    let d = Deck::parse(&out).deck;
    assert!(d.diagnostics().is_empty(), "{:?}", d.diagnostics());
    let ids: Vec<_> = d.slides[1].elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["t3", "s2"]);

    let out = run(DECK, op(json!({"op": "move_slide", "from": 1, "to": 0})));
    let d = Deck::parse(&out).deck;
    assert_eq!(d.slides[0].elements[0].id, "t2");
    assert_eq!(d.slides[0].notes.as_deref(), Some("Say two.\n"));
    assert_eq!(d.slides[1].elements[0].id, "t1");
    let three = "A\n---\nB\n---\nC";
    assert_eq!(
        run(three, op(json!({"op": "move_slide", "from": 2, "to": 0}))),
        "C\n---\nA\n---\nB\n"
    );
    fails(
        DECK,
        op(json!({"op": "move_slide", "from": 0, "to": 9})),
        "E-NOT-FOUND",
    );
}

#[test]
fn add_element_structured_and_raw() {
    let o = op(json!({"op": "add_element", "slide": 0, "kind": "text",
        "attrs": {"x": 5, "y": 6, "w": 70, "h": 80, "color": "accent1"}, "body": "Hello"}));
    let out = run(DECK, o);
    assert!(out.contains("<!-- shape id=s1 kind=rect x=10 y=10 w=50 h=50 -->\n\n<!-- text id=t3 x=5 y=6 w=70 h=80 color=accent1 -->\nHello\n---"));

    let o = op(
        json!({"op": "add_element", "slide": 1, "index": 0, "raw": "<!-- shape id=sq kind=rect x=1 y=1 w=1 h=1 -->"}),
    );
    let out = run(DECK, o);
    let d = Deck::parse(&out).deck;
    assert_eq!(d.slides[1].elements[0].id, "sq");
    assert_eq!(d.slides[1].notes.as_deref(), Some("Say two.\n"));

    let o = op(
        json!({"op": "add_element", "slide": 1, "kind": "text", "attrs": {"x": 1, "y": 1, "w": 1, "h": 1}}),
    );
    let out = run(DECK, o);
    assert!(
        out.contains("Two\n\n<!-- text id=t3 x=1 y=1 w=1 h=1 -->\n<!-- notes -->"),
        "{out}"
    );

    fails(
        DECK,
        op(json!({"op": "add_element", "slide": 0, "kind": "text", "attrs": {"x": 1}})),
        "E-ATTR-MISSING",
    );
    fails(
        DECK,
        op(json!({"op": "add_element", "slide": 0, "raw": "<!-- text id=t1 x=1 y=1 w=1 h=1 -->"})),
        "E-DUP-ID",
    );
    fails(
        DECK,
        op(json!({"op": "add_element", "slide": 0, "raw": "two\n---\nslides"})),
        "E-RAW",
    );
}

#[test]
fn duplicate_element_and_z_order() {
    let out = run(
        DECK,
        op(json!({"op": "duplicate_element", "id": "t1", "dx": 20, "dy": 20})),
    );
    assert!(
        out.contains("# One\n\n<!-- text id=t3 x=120 y=120 w=800 h=200 -->\n# One\n\n<!-- shape")
    );

    let out = run(DECK, op(json!({"op": "set_z", "id": "t1", "to": "front"})));
    let d = Deck::parse(&out).deck;
    let ids: Vec<_> = d.slides[0].elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["s1", "t1"]);
    let mut d = Deck::parse(DECK).deck;
    d.apply(Op::SetZ {
        id: "s1".into(),
        to: ZTo::Step(ZStep::Back),
    })
    .unwrap();
    assert_eq!(d.slides[0].elements[0].id, "s1");
    d.apply(Op::SetZ {
        id: "s1".into(),
        to: ZTo::Index(9),
    })
    .unwrap();
    assert_eq!(d.slides[0].elements[1].id, "s1");
}

#[test]
fn set_attrs_set_and_remove() {
    let o = op(
        json!({"op": "set_attrs", "id": "t1", "set": {"color": "#fff", "locked": true, "size": 48}, "remove": ["w"]}),
    );
    fails(DECK, o, "E-ATTR-MISSING");
    let o = op(
        json!({"op": "set_attrs", "id": "t1", "set": {"color": "#fff", "locked": true, "size": 48.5}}),
    );
    let out = run(DECK, o);
    assert!(
        out.contains("<!-- text id=t1 x=100 y=100 w=800 h=200 color=#fff locked size=48.5 -->")
    );
    let o = op(json!({"op": "set_attrs", "id": "s1", "remove": ["kind"], "set": {"title": "a b"}}));
    let out = run(DECK, o);
    assert!(out.contains("<!-- shape id=s1 x=10 y=10 w=50 h=50 title=\"a b\" -->"));
    fails(
        DECK,
        op(json!({"op": "set_attrs", "id": "t1", "set": {"x": "abc"}})),
        "E-ATTR-VALUE",
    );
    fails(
        DECK,
        op(json!({"op": "set_attrs", "id": "t1", "set": {"id": "zz"}})),
        "E-ATTR-ID",
    );
    fails(
        DECK,
        op(json!({"op": "set_attrs", "id": "t1", "set": {"Bad": 1}})),
        "E-ATTR-KEY",
    );
}

#[test]
fn notes_add_replace_remove() {
    let out = run(
        DECK,
        op(json!({"op": "set_notes", "slide": 0, "text": "New notes"})),
    );
    assert!(out.contains(
        "<!-- shape id=s1 kind=rect x=10 y=10 w=50 h=50 -->\n\n<!-- notes -->\nNew notes\n---"
    ));
    let out = run(
        DECK,
        op(json!({"op": "set_notes", "slide": 1, "text": "Changed"})),
    );
    assert!(out.ends_with("<!-- notes -->\nChanged"));
    let out = run(
        DECK,
        op(json!({"op": "set_notes", "slide": 1, "text": null})),
    );
    assert!(out.ends_with("Two\n\n"));
    assert!(Deck::parse(&out).deck.slides[1].notes.is_none());
}

#[test]
fn slide_attrs_edit_or_create_directive() {
    let out = run(
        DECK,
        op(
            json!({"op": "set_slide_attrs", "slide": 1, "set": {"bg": "#111", "transition": "fade"}}),
        ),
    );
    assert!(out.contains("<!-- slide bg=#111 transition=fade -->"));
    let out = run(
        DECK,
        op(json!({"op": "set_slide_attrs", "slide": 0, "set": {"master": "none"}})),
    );
    assert!(out.contains("---\n<!-- slide master=none -->\n\n<!-- text id=t1"));
    let out = run(
        DECK,
        op(json!({"op": "set_slide_attrs", "slide": 1, "remove": ["bg"]})),
    );
    assert!(out.contains("<!-- slide -->"));
}

#[test]
fn front_matter_set_add_remove() {
    let out = run(
        DECK,
        op(json!({"op": "set_front_matter", "key": "title", "value": "New: title"})),
    );
    assert!(out.starts_with("---\ntitle: \"New: title\"\n---"));
    let out = run(
        DECK,
        op(json!({"op": "set_front_matter", "key": "canvas", "value": "4:5"})),
    );
    assert!(out.starts_with("---\ntitle: Ops\ncanvas: \"4:5\"\n---"));
    assert_eq!(Deck::parse(&out).deck.canvas().height, 1350);
    let out = run(
        DECK,
        op(json!({"op": "set_front_matter", "key": "title", "value": null})),
    );
    assert!(out.starts_with("---\n---\n"));
    let out = run(
        "# a\n",
        op(json!({"op": "set_front_matter", "key": "dir", "value": "rtl"})),
    );
    assert_eq!(out, "---\ndir: rtl\n---\n# a\n");
    let nested = "---\ntheme:\n  colors:\n    accent1: \"#e11\"\ntitle: x\n---\n";
    let out = run(
        nested,
        op(json!({"op": "set_front_matter", "key": "theme", "value": "t.json"})),
    );
    assert_eq!(out, "---\ntheme: t.json\ntitle: x\n---\n");
    fails(
        DECK,
        op(json!({"op": "set_front_matter", "key": "dir", "value": "sideways"})),
        "E-STRUCTURE",
    );
}

#[test]
fn definitions_add_and_remove() {
    let raw = "<!-- master id=plain -->\n<!-- text id=foot x=0 y=1000 w=1920 h=80 -->\nFooter";
    let out = run(DECK, op(json!({"op": "add_definition", "raw": raw})));
    let d = Deck::parse(&out).deck;
    assert_eq!(
        d.masters.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["brand", "plain"]
    );
    fails(
        DECK,
        op(json!({"op": "add_definition", "raw": "<!-- master id=brand -->"})),
        "E-DUP-ID",
    );
    fails(
        DECK,
        op(json!({"op": "remove_definition", "id": "brand"})),
        "E-IN-USE",
    );
    let out = run(DECK, op(json!({"op": "remove_definition", "id": "tb"})));
    let d = Deck::parse(&out).deck;
    assert!(d.layouts.is_empty());
    let mut d = Deck::parse(&out).deck;
    d.apply(op(json!({"op": "remove_definition", "id": "brand"})))
        .unwrap();
    assert!(d.masters.is_empty());
    assert!(
        d.text()
            .starts_with("---\ntitle: Ops\n---\n\n<!-- text id=t1"),
        "{}",
        d.text()
    );
    let out = run("# a\n", op(json!({"op": "add_definition", "raw": raw})));
    assert!(out.starts_with("<!-- master id=plain -->"));
}

#[test]
fn group_moves_members_and_writes_only_their_lines() {
    let src = "<!-- text id=a x=100 y=100 w=100 h=100 -->\nA\n\n<!-- text x=300 y=100 w=100 h=100 -->\nB\n\n<!-- text id=c x=0 y=0 w=10 h=10 -->\nC\n";
    let mut d = Deck::parse(src).deck;
    let b = d.slides[0].elements[1].id.clone();
    let undo_group = d
        .apply(Op::Group {
            ids: vec!["a".into(), b.clone()],
            id: None,
        })
        .unwrap();
    let grouped = d.text().to_string();
    assert!(
        grouped.contains(&format!("<!-- group id=g1 items=a,{b} -->")),
        "{grouped}"
    );
    // FR-EL-10: moving the group moves every member and writes only their lines.
    let undo_move = d
        .apply(Op::SetGeometry {
            id: "g1".into(),
            x: Some(150.0),
            y: None,
            w: None,
            h: None,
            rot: None,
        })
        .unwrap();
    let moved = d.text();
    let changed: Vec<_> = grouped
        .lines()
        .zip(moved.lines())
        .filter(|(x, y)| x != y)
        .map(|(_, y)| y)
        .collect();
    assert_eq!(
        changed,
        [
            "<!-- text id=a x=150 y=100 w=100 h=100 -->".to_string(),
            format!("<!-- text x=350 y=100 w=100 h=100 id={b} -->"),
        ]
    );
    d.apply(undo_move).unwrap();
    assert_eq!(d.text(), grouped);
    let undo_ungroup = d.apply(Op::Ungroup { id: "g1".into() }).unwrap();
    d.apply(undo_ungroup).unwrap();
    d.apply(undo_group).unwrap();
    assert_eq!(d.text(), src);
    fails(
        src,
        Op::Group {
            ids: vec!["a".into()],
            id: None,
        },
        "E-GROUP",
    );
    fails(src, Op::Ungroup { id: "a".into() }, "E-NOT-GROUP");
}

#[test]
fn batch_of_20_moves_undoes_in_one_step() {
    let mut src = String::new();
    for i in 0..20 {
        src.push_str(&format!("<!-- text id=e{i} x=0 y=0 w=10 h=10 -->\n{i}\n"));
    }
    let mut d = Deck::parse(&src).deck;
    let ops = (0..20)
        .map(|i| op(json!({"op": "set_geometry", "id": format!("e{i}"), "x": i * 10})))
        .collect();
    let undo = d.apply_batch(ops).unwrap();
    assert!(matches!(&undo, Op::Batch { ops } if ops.len() == 20));
    assert_ne!(d.text(), src);
    d.apply(undo).unwrap();
    assert_eq!(d.text(), src);
}

#[test]
fn failed_batch_changes_nothing() {
    let mut d = Deck::parse(DECK).deck;
    let ops = vec![
        op(json!({"op": "set_geometry", "id": "t1", "x": 1})),
        op(json!({"op": "remove_element", "id": "nope"})),
    ];
    assert_eq!(d.apply_batch(ops).unwrap_err().code, "E-NOT-FOUND");
    assert_eq!(d.text(), DECK);
}

#[test]
fn every_op_variant_round_trips_through_json() {
    let all = json!([
        {"op": "add_slide", "index": 0, "layout": "tb"},
        {"op": "remove_slide", "index": 0},
        {"op": "duplicate_slide", "index": 0},
        {"op": "move_slide", "from": 0, "to": 1},
        {"op": "add_element", "slide": 0, "index": 1, "kind": "text", "attrs": {"x": 1}, "body": "b"},
        {"op": "add_element", "slide": 0, "raw": "<!-- text -->"},
        {"op": "remove_element", "id": "t1"},
        {"op": "duplicate_element", "id": "t1", "dx": 10},
        {"op": "set_geometry", "id": "t1", "x": 1, "rot": 45},
        {"op": "set_attrs", "id": "t1", "set": {"a": 1, "b": true, "c": ["x", "y"]}, "remove": ["d"]},
        {"op": "set_body", "id": "t1", "text": "hi"},
        {"op": "set_z", "id": "t1", "to": 2},
        {"op": "set_z", "id": "t1", "to": "backward"},
        {"op": "set_notes", "slide": 0, "text": null},
        {"op": "set_slide_attrs", "slide": 0, "set": {"bg": "#fff"}},
        {"op": "set_front_matter", "key": "title", "value": {"nested": [1, 2]}},
        {"op": "add_definition", "raw": "<!-- master id=m -->"},
        {"op": "remove_definition", "id": "m"},
        {"op": "group", "ids": ["a", "b"], "id": "g"},
        {"op": "ungroup", "id": "g"},
        {"op": "replace", "at": 3, "old": "a", "new": "b"},
        {"op": "batch", "ops": [{"op": "remove_slide", "index": 0}]}
    ]);
    for v in all.as_array().unwrap() {
        let o: Op = serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("{v}: {e}"));
        let back: Op = serde_json::from_str(&serde_json::to_string(&o).unwrap()).unwrap();
        assert_eq!(back, o);
    }
}

#[test]
fn queries_elements_at_search_assets() {
    let src = "---\ntheme: themes/brand.json\n---\n<!-- slide bg=assets/hero.jpg -->\n<!-- shape id=low kind=rect x=0 y=0 w=200 h=200 -->\n<!-- shape id=high kind=rect x=100 y=100 w=200 h=200 -->\n<!-- shape id=spin kind=rect x=500 y=500 w=200 h=20 rot=90 -->\n<!-- image id=i1 x=0 y=0 w=10 h=10 -->\n![Chart](assets/chart.png)\n<!-- text id=t1 x=0 y=0 w=10 h=10 -->\nSee ![again](assets/chart.png) and ![web](https://x.com/a.png)\n<!-- video id=v1 x=0 y=0 w=10 h=10 src=assets/demo.mp4 poster=assets/demo.jpg -->\n\n<!-- notes -->\nMention the Chart.\n";
    let d = Deck::parse(src).deck;
    // FR-OPS-05: overlapping shapes come back top first.
    assert_eq!(d.elements_at(0, 150.0, 150.0), ["high", "low"]);
    // Rotated 90 degrees: the 200x20 bar now stands upright around its center (600, 510).
    assert_eq!(d.elements_at(0, 600.0, 590.0), ["spin"]);
    assert!(d.elements_at(0, 690.0, 510.0).is_empty());

    let hits = d.search("chart");
    assert_eq!(
        hits.iter()
            .map(|h| (h.element.as_deref(), h.field))
            .collect::<Vec<_>>(),
        [
            (Some("i1"), s_md_slides::HitField::Body),
            (Some("t1"), s_md_slides::HitField::Body),
            (None, s_md_slides::HitField::Notes),
        ]
    );

    // FR-ASSET-03: every path once, URLs skipped.
    let assets: Vec<_> = d
        .assets()
        .into_iter()
        .map(|a| (a.path, a.kind, a.used_by))
        .collect();
    use s_md_slides::AssetKind::*;
    assert_eq!(
        assets,
        [
            (
                "themes/brand.json".to_string(),
                Theme,
                vec!["front".to_string()]
            ),
            ("assets/hero.jpg".into(), Background, vec!["slide:0".into()]),
            (
                "assets/chart.png".into(),
                Image,
                vec!["i1".into(), "t1".into()]
            ),
            ("assets/demo.mp4".into(), Video, vec!["v1".into()]),
            ("assets/demo.jpg".into(), Poster, vec!["v1".into()]),
        ]
    );
}
