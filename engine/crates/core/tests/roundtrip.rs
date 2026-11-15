//! Random decks, random op sequences, undo everything, compare bytes (NFR-03).

use proptest::prelude::*;
use s_md_slides::{Deck, Op};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

static OK: AtomicUsize = AtomicUsize::new(0);

fn attr_value() -> impl Strategy<Value = String> {
    prop_oneof![
        (-300i32..2500).prop_map(|n| n.to_string()),
        (0u32..100000).prop_map(|n| format!("{}.{:02}", n / 100, n % 100)),
        Just("10.50".to_string()),
        Just("\"20\"".to_string()),
        Just("\"a \\\" b\"".to_string()),
        Just("#fff".to_string()),
        Just("abc".to_string()),
    ]
}

fn directive(kind: &'static str, id: Option<String>) -> impl Strategy<Value = String> {
    (
        prop::collection::vec(
            (
                prop::sample::select(vec!["x", "y", "w", "h", "rot", "foo", "size", "title"]),
                attr_value(),
            ),
            0..5,
        ),
        any::<bool>(),
        prop::sample::select(vec![" ", "  "]),
        any::<bool>(),
    )
        .prop_map(move |(attrs, flag, gap, tight)| {
            let mut s = format!("<!--{gap}{kind}");
            if let Some(id) = &id {
                s.push_str(&format!(" id={id}"));
            }
            for (k, v) in attrs {
                s.push_str(&format!(" {k}={v}"));
            }
            if flag {
                s.push_str(" locked");
            }
            s.push_str(if tight { "-->" } else { " -->" });
            s
        })
}

fn body_line() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("# Heading".to_string()),
        Just("Some *text* here.".to_string()),
        Just("- item".to_string()),
        Just(String::new()),
        Just("<!-- fix this later -->".to_string()),
        Just("***".to_string()),
        Just("```js\n---\n<!-- text id=inside -->\n```".to_string()),
        Just("~~~~\n```\n---\n~~~~".to_string()),
        Just("مرحبا بالعالم".to_string()),
        "[a-zA-Z ]{0,20}",
    ]
}

fn element(n: usize) -> impl Strategy<Value = String> {
    (
        prop::sample::select(vec!["text", "text", "widget", "shape"]),
        any::<bool>(),
        prop::collection::vec(body_line(), 0..4),
    )
        .prop_flat_map(move |(kind, has_id, body)| {
            // Unknown kinds only count as directives when they carry an id.
            let id = (has_id || kind == "widget").then(|| format!("e{n}"));
            directive(kind, id).prop_map(move |d| {
                let mut s = d;
                for l in &body {
                    s.push('\n');
                    s.push_str(l);
                }
                s
            })
        })
}

fn slide(k: usize) -> impl Strategy<Value = String> {
    (
        prop::option::of(prop::collection::vec(body_line(), 1..3)),
        any::<bool>(),
        prop::collection::vec(0usize..1, 0..5),
    )
        .prop_flat_map(move |(pre, opener, els)| {
            let els: Vec<_> = (0..els.len()).map(|i| element(k * 10 + i)).collect();
            els.prop_map(move |els| {
                let mut lines = Vec::new();
                if opener {
                    lines.push("<!-- slide bg=#0b1020 -->".to_string());
                }
                if let Some(p) = &pre {
                    lines.extend(p.iter().cloned());
                }
                lines.extend(els);
                lines.join("\n")
            })
        })
}

fn deck() -> impl Strategy<Value = String> {
    (
        any::<bool>(),
        prop::option::of(prop::collection::vec(
            prop::sample::select(vec![
                "title: Q3 review",
                "canvas: 16:9",
                "dir: rtl",
                "foo: bar  # unknown key",
                "s-md-slides: 1",
            ]),
            0..4,
        )),
        prop::collection::vec(0usize..1, 1..5),
        prop::sample::select(vec!["lf", "crlf", "mixed"]),
        any::<bool>(),
        prop::sample::select(vec!["---", "--- "]),
        any::<bool>(),
    )
        .prop_flat_map(|(bom, front, slides, eol, final_nl, sep, defs)| {
            let slides: Vec<_> = (0..slides.len()).map(slide).collect();
            slides.prop_map(move |slides| {
                let mut s = String::new();
                if let Some(f) = &front {
                    s.push_str("---\n");
                    for l in f {
                        s.push_str(l);
                        s.push('\n');
                    }
                    s.push_str("---\n");
                }
                if defs {
                    s.push_str("<!-- master id=brand -->\n<!-- image id=logo x=1720 y=40 w=160 h=60 -->\n![logo](a.png)\n\n<!-- layout id=tb master=brand -->\n<!-- placeholder id=title x=1 y=1 w=1 h=1 -->\n\n---\n");
                }
                s.push_str(&slides.join(&format!("\n{sep}\n")));
                if final_nl {
                    s.push('\n');
                }
                let s = match eol {
                    "crlf" => s.replace('\n', "\r\n"),
                    "mixed" => s
                        .split('\n')
                        .enumerate()
                        .map(|(i, l)| {
                            if i % 3 == 0 {
                                format!("{l}\r")
                            } else {
                                l.to_string()
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim_end_matches('\r')
                        .to_string(),
                    _ => s,
                };
                if bom { format!("\u{feff}{s}") } else { s }
            })
        })
}

#[derive(Debug, Clone)]
enum Pick {
    Geometry(usize, Option<i32>, Option<i32>, Option<u16>, Option<u16>),
    Body(usize, Vec<String>),
    Remove(usize),
    Missing,
}

fn pick() -> impl Strategy<Value = Pick> {
    prop_oneof![
        4 => (any::<usize>(), prop::option::of(-500i32..3000), prop::option::of(-500i32..3000),
              prop::option::of(0u16..2000), prop::option::of(0u16..2000))
            .prop_map(|(i, x, y, w, h)| Pick::Geometry(i, x, y, w, h)),
        3 => (any::<usize>(), prop::collection::vec(body_line(), 0..3)).prop_map(|(i, b)| Pick::Body(i, b)),
        2 => any::<usize>().prop_map(Pick::Remove),
        1 => Just(Pick::Missing),
    ]
}

fn ids(d: &Deck) -> Vec<String> {
    d.elements().map(|e| e.id.clone()).collect()
}

fn diff_lines(a: &str, b: &str) -> Option<usize> {
    let (la, lb): (Vec<_>, Vec<_>) = (a.split('\n').collect(), b.split('\n').collect());
    (la.len() == lb.len()).then(|| la.iter().zip(&lb).filter(|(x, y)| x != y).count())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn apply_then_undo_is_byte_identical(src in deck(), picks in prop::collection::vec(pick(), 1..8)) {
        let mut d = Deck::parse(&src).deck;
        prop_assert_eq!(d.text(), src.as_str());
        let mut undo = Vec::new();
        for p in picks {
            let all = ids(&d);
            let id_at = |i: usize| (!all.is_empty()).then(|| all[i % all.len()].clone());
            let op = match p {
                Pick::Geometry(i, x, y, w, h) => match id_at(i) {
                    Some(id) => Op::SetGeometry {
                        id,
                        x: x.map(|v| v as f64 / 2.0),
                        y: y.map(f64::from),
                        w: w.map(f64::from),
                        h: h.map(f64::from),
                        rot: None,
                    },
                    None => continue,
                },
                Pick::Body(i, b) => match id_at(i) {
                    Some(id) => Op::SetBody { id, text: b.join("\n") },
                    None => continue,
                },
                Pick::Remove(i) => match id_at(i) {
                    Some(id) => Op::RemoveElement { id },
                    None => continue,
                },
                Pick::Missing => Op::RemoveElement { id: "nope".into() },
            };
            let before = d.text().to_string();
            // Moving implicit text writes its directive line first, which adds a line.
            let is_move = matches!(&op, Op::SetGeometry { id, .. } if !d.element(id).is_some_and(|e| e.implicit));
            match d.apply(op.clone()) {
                Ok(inv) => {
                    if is_move {
                        // One move changes at most one line (zero when values are unchanged).
                        prop_assert!(diff_lines(&before, d.text()).is_some_and(|n| n <= 1),
                            "move changed more than one line: {:?}", op);
                    }
                    OK.fetch_add(1, Relaxed);
                    undo.push(inv);
                }
                Err(_) => {
                    prop_assert_eq!(d.text(), before.as_str(), "failed op changed the deck")
                }
            }
        }
        for inv in undo.into_iter().rev() {
            d.apply(inv.clone()).map_err(|e| TestCaseError::fail(format!("{inv:?}: {e:?}")))?;
        }
        prop_assert_eq!(d.text(), src.as_str());
        if std::env::var("ROUNDTRIP_STATS").is_ok() {
            eprintln!("successful ops so far: {}", OK.load(Relaxed));
        }
    }

    #[test]
    fn batch_undo_is_one_step(src in deck()) {
        let mut d = Deck::parse(&src).deck;
        let ops: Vec<Op> = ids(&d).into_iter().map(|id| Op::SetGeometry {
            id, x: Some(1.25), y: Some(-3.0), w: None, h: Some(40.0), rot: Some(15.5),
        }).collect();
        let inv = d.apply(Op::Batch { ops }).unwrap();
        d.apply(inv).unwrap();
        prop_assert_eq!(d.text(), src.as_str());
    }
}
