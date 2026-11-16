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
    Attrs(usize, &'static str, Option<&'static str>),
    Duplicate(usize, i16),
    Z(usize, u8),
    AddElement(usize, Option<usize>, Vec<String>),
    Notes(usize, Option<Vec<String>>),
    SlideAttrs(usize, &'static str, Option<&'static str>),
    Front(&'static str, Option<&'static str>),
    AddSlide(usize, bool),
    RemoveSlide(usize),
    DuplicateSlide(usize),
    MoveSlide(usize, usize),
    Group(usize, usize),
    Definition(bool),
}

fn pick() -> impl Strategy<Value = Pick> {
    let i = || any::<usize>();
    let key = || {
        prop::sample::select(vec![
            "color", "size", "x", "w", "locked", "name", "foo", "dir",
        ])
    };
    let val = || {
        prop::option::of(prop::sample::select(vec![
            "12", "#fff", "a b", "rtl", "-5", "1.5",
        ]))
    };
    prop_oneof![
        4 => (i(), prop::option::of(-500i32..3000), prop::option::of(-500i32..3000),
              prop::option::of(0u16..2000), prop::option::of(0u16..2000))
            .prop_map(|(i, x, y, w, h)| Pick::Geometry(i, x, y, w, h)),
        3 => (i(), prop::collection::vec(body_line(), 0..3)).prop_map(|(i, b)| Pick::Body(i, b)),
        2 => i().prop_map(Pick::Remove),
        1 => Just(Pick::Missing),
        2 => (i(), key(), val()).prop_map(|(i, k, v)| Pick::Attrs(i, k, v)),
        2 => (i(), -50i16..50).prop_map(|(i, d)| Pick::Duplicate(i, d)),
        2 => (i(), 0u8..6).prop_map(|(i, z)| Pick::Z(i, z)),
        2 => (i(), prop::option::of(0usize..4), prop::collection::vec(body_line(), 0..2))
            .prop_map(|(s, at, b)| Pick::AddElement(s, at, b)),
        2 => (i(), prop::option::of(prop::collection::vec(body_line(), 0..3))).prop_map(|(s, t)| Pick::Notes(s, t)),
        2 => (i(), prop::sample::select(vec!["bg", "transition", "layout", "foo"]), val())
            .prop_map(|(s, k, v)| Pick::SlideAttrs(s, k, v)),
        2 => (prop::sample::select(vec!["title", "canvas", "dir", "foo", "lang"]),
              prop::option::of(prop::sample::select(vec!["Q3: review", "4:5", "rtl", "bar", "en"])))
            .prop_map(|(k, v)| Pick::Front(k, v)),
        2 => (0usize..8, any::<bool>()).prop_map(|(i, l)| Pick::AddSlide(i, l)),
        1 => i().prop_map(Pick::RemoveSlide),
        1 => i().prop_map(Pick::DuplicateSlide),
        2 => (i(), i()).prop_map(|(a, b)| Pick::MoveSlide(a, b)),
        1 => (i(), i()).prop_map(|(a, b)| Pick::Group(a, b)),
        1 => any::<bool>().prop_map(Pick::Definition),
    ]
}

/// What a successful op may change: the bytes of one element, one slide, or anything.
enum Scope {
    Element(std::ops::Range<usize>),
    Slide(std::ops::Range<usize>),
    Any,
}

fn kv(k: &str, v: Option<&str>) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    m.insert(k.into(), v.map_or(serde_json::Value::Null, |v| v.into()));
    m
}

/// Turn a pick into an op on the current deck, plus the bytes it may touch.
fn to_op(d: &Deck, p: Pick) -> Option<(Op, Scope)> {
    let all = ids(d);
    let id_at = |i: usize| (!all.is_empty()).then(|| all[i % all.len()].clone());
    let n = d.slides.len();
    let slide_at = |i: usize| (n > 0).then(|| i % n);
    let el = |id: String, op: Op| {
        let group = d
            .element(&id)
            .is_some_and(|e| e.kind == s_md_slides::Kind::Group);
        let scope = if group {
            Scope::Any
        } else {
            Scope::Element(d.element_range(&id).unwrap())
        };
        Some((op, scope))
    };
    let slide_scope = |s: usize| Scope::Slide(d.slide_range(s).unwrap());
    match p {
        Pick::Geometry(i, x, y, w, h) => {
            let id = id_at(i)?;
            el(
                id.clone(),
                Op::SetGeometry {
                    id,
                    x: x.map(|v| v as f64 / 2.0),
                    y: y.map(f64::from),
                    w: w.map(f64::from),
                    h: h.map(f64::from),
                    rot: None,
                },
            )
        }
        Pick::Body(i, b) => {
            let id = id_at(i)?;
            el(
                id.clone(),
                Op::SetBody {
                    id,
                    text: b.join("\n"),
                },
            )
        }
        Pick::Remove(i) => {
            let id = id_at(i)?;
            el(id.clone(), Op::RemoveElement { id })
        }
        Pick::Missing => Some((Op::RemoveElement { id: "nope".into() }, Scope::Any)),
        Pick::Attrs(i, k, v) => {
            let id = id_at(i)?;
            el(
                id.clone(),
                Op::SetAttrs {
                    id,
                    set: kv(k, v),
                    remove: Vec::new(),
                },
            )
        }
        Pick::Duplicate(i, dx) => {
            let id = id_at(i)?;
            el(
                id.clone(),
                Op::DuplicateElement {
                    id,
                    dx: Some(dx as f64),
                    dy: None,
                },
            )
        }
        Pick::Z(i, z) => {
            let id = id_at(i)?;
            let to = match z {
                0 => s_md_slides::ZTo::Step(s_md_slides::ZStep::Front),
                1 => s_md_slides::ZTo::Step(s_md_slides::ZStep::Back),
                2 => s_md_slides::ZTo::Step(s_md_slides::ZStep::Forward),
                3 => s_md_slides::ZTo::Step(s_md_slides::ZStep::Backward),
                k => s_md_slides::ZTo::Index(k as usize - 4),
            };
            let scope = d
                .slides
                .iter()
                .position(|s| s.elements.iter().any(|e| e.id == id))
                .map_or(Scope::Any, slide_scope);
            Some((Op::SetZ { id, to }, scope))
        }
        Pick::AddElement(s, at, b) => {
            let s = slide_at(s)?;
            let mut attrs = serde_json::Map::new();
            for (k, v) in [("x", 10), ("y", 20), ("w", 300), ("h", 40)] {
                attrs.insert(k.into(), v.into());
            }
            let op = Op::AddElement {
                slide: s,
                index: at,
                kind: Some("text".into()),
                attrs: Some(attrs),
                body: Some(b.join("\n")),
                raw: None,
            };
            Some((op, slide_scope(s)))
        }
        Pick::Notes(s, t) => {
            let s = slide_at(s)?;
            Some((
                Op::SetNotes {
                    slide: s,
                    text: t.map(|t| t.join("\n")),
                },
                slide_scope(s),
            ))
        }
        Pick::SlideAttrs(s, k, v) => {
            let s = slide_at(s)?;
            Some((
                Op::SetSlideAttrs {
                    slide: s,
                    set: kv(k, v),
                    remove: Vec::new(),
                },
                slide_scope(s),
            ))
        }
        Pick::Front(k, v) => Some((
            Op::SetFrontMatter {
                key: k.into(),
                value: v.map(Into::into),
            },
            Scope::Any,
        )),
        Pick::AddSlide(i, layout) => {
            let layout = (layout && !d.layouts.is_empty()).then(|| d.layouts[0].id.clone());
            Some((
                Op::AddSlide {
                    index: i,
                    layout,
                    raw: None,
                },
                Scope::Any,
            ))
        }
        Pick::RemoveSlide(s) => Some((
            Op::RemoveSlide {
                index: slide_at(s)?,
            },
            Scope::Any,
        )),
        Pick::DuplicateSlide(s) => Some((
            Op::DuplicateSlide {
                index: slide_at(s)?,
            },
            Scope::Any,
        )),
        Pick::MoveSlide(a, b) => Some((
            Op::MoveSlide {
                from: slide_at(a)?,
                to: slide_at(b)?,
            },
            Scope::Any,
        )),
        Pick::Group(a, b) => {
            let s = d.slides.get(slide_at(a)?)?;
            if s.elements.len() < 2 {
                return None;
            }
            let x = &s.elements[b % s.elements.len()];
            let y = &s.elements[(b + 1) % s.elements.len()];
            Some((
                Op::Group {
                    ids: vec![x.id.clone(), y.id.clone()],
                    id: None,
                },
                Scope::Any,
            ))
        }
        Pick::Definition(add) => {
            let op = if add {
                Op::AddDefinition {
                    raw: "<!-- layout id=extra -->\n<!-- placeholder id=p x=1 y=1 w=1 h=1 -->"
                        .into(),
                }
            } else {
                Op::RemoveDefinition { id: "extra".into() }
            };
            Some((op, Scope::Any))
        }
    }
}

/// True if the edit stayed inside `r`: everything before and after `r` in `before`
/// is still there, unchanged, at the start and end of `after`.
fn changed_within(before: &str, after: &str, r: &std::ops::Range<usize>) -> bool {
    let (head, tail) = (&before[..r.start], &before[r.end..]);
    head.len() + tail.len() <= after.len() && after.starts_with(head) && after.ends_with(tail)
}

fn ids(d: &Deck) -> Vec<String> {
    d.elements().map(|e| e.id.clone()).collect()
}

fn diff_lines(a: &str, b: &str) -> Option<usize> {
    let (la, lb): (Vec<_>, Vec<_>) = (a.split('\n').collect(), b.split('\n').collect());
    (la.len() == lb.len()).then(|| la.iter().zip(&lb).filter(|(x, y)| x != y).count())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(10_000))]

    #[test]
    fn apply_then_undo_is_byte_identical(src in deck(), picks in prop::collection::vec(pick(), 1..8)) {
        let mut d = Deck::parse(&src).deck;
        prop_assert_eq!(d.text(), src.as_str());
        let mut undo = Vec::new();
        for p in picks {
            let Some((op, scope)) = to_op(&d, p) else { continue };
            let before = d.text().to_string();
            // Moving implicit text writes its directive line first, which adds a line.
            let is_move = matches!(&op, Op::SetGeometry { id, .. }
                if d.element(id).is_some_and(|e| !e.implicit && e.kind != s_md_slides::Kind::Group));
            match d.apply(op.clone()) {
                Ok(inv) => {
                    OK.fetch_add(1, Relaxed);
                    if is_move {
                        prop_assert!(diff_lines(&before, d.text()).is_some_and(|n| n <= 1),
                            "move changed more than one line: {:?}", op);
                    }
                    match &scope {
                        Scope::Element(r) | Scope::Slide(r) => prop_assert!(changed_within(&before, d.text(), r),
                            "{:?} touched bytes outside its target {:?}", op, r),
                        Scope::Any => {}
                    }
                    undo.push(inv);
                }
                Err(_) => prop_assert_eq!(d.text(), before.as_str(), "failed op changed the deck"),
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
