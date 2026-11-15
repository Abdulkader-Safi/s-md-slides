//! M1 acceptance checks from the build spec, one test per requirement.

use s_md_slides::{Canvas, Deck, Dir, Op, Severity};

fn mv(id: &str, x: f64) -> Op {
    Op::SetGeometry {
        id: id.into(),
        x: Some(x),
        y: None,
        w: None,
        h: None,
        rot: None,
    }
}

fn changed_lines(a: &str, b: &str) -> usize {
    let (la, lb): (Vec<_>, Vec<_>) = (a.lines().collect(), b.lines().collect());
    assert_eq!(la.len(), lb.len(), "line count changed");
    la.iter().zip(&lb).filter(|(x, y)| x != y).count()
}

#[test]
fn fr_fmt_01_parses_every_part() {
    let src = "---\ns-md-slides: 1\ntitle: Q3\n---\n\n<!-- master id=brand -->\n<!-- image id=logo x=1720 y=40 w=160 h=60 -->\n![logo](assets/logo.svg)\n\n<!-- layout id=title-body master=brand -->\n<!-- placeholder id=title x=120 y=80 w=1680 h=200 -->\n\n---\n\n<!-- slide layout=title-body transition=fade -->\n\n<!-- text id=t1 x=120 y=80 w=1680 h=200 enter=fade -->\n# Quarterly review\n\n<!-- notes -->\nMention Kuwait first.\n";
    let p = Deck::parse(src);
    assert!(p.diagnostics.is_empty(), "{:?}", p.diagnostics);
    let d = p.deck;
    assert_eq!(d.front.title.as_deref(), Some("Q3"));
    assert_eq!(d.masters.len(), 1);
    assert_eq!(d.masters[0].elements[0].id, "logo");
    assert_eq!(d.layouts[0].placeholders[0].id, "title");
    assert_eq!(d.layouts[0].master.as_deref(), Some("brand"));
    assert_eq!(d.slides.len(), 1);
    let s = &d.slides[0];
    assert_eq!(s.layout.as_deref(), Some("title-body"));
    assert_eq!(s.transition.as_deref(), Some("fade"));
    assert_eq!(s.elements.len(), 1);
    assert_eq!(s.elements[0].body, "# Quarterly review\n");
    assert_eq!(s.elements[0].attr("enter"), Some("fade"));
    assert_eq!(s.notes.as_deref(), Some("Mention Kuwait first.\n"));
}

#[test]
fn fr_fmt_02_unchanged_save_is_identical() {
    for src in [
        "",
        "\u{feff}# Hi\r\n---\r\nBye",
        "<!-- text x=1 y=2 w=3 h=4 -->\nno newline",
    ] {
        assert_eq!(Deck::parse(src).deck.text(), src);
    }
}

#[test]
fn fr_fmt_03_one_move_one_line_in_50_slides() {
    let mut src = String::from("---\ntitle: Big\n---\n");
    for i in 0..50 {
        if i > 0 {
            src.push_str("\n---\n\n");
        }
        src.push_str(&format!(
            "<!-- text id=a{i} x=100 y=100 w=800 h=200 -->\n# Slide {i}\n\n<!-- shape id=b{i} kind=rect x=10 y=10 w=50 h=50 -->\n"
        ));
    }
    let mut d = Deck::parse(&src).deck;
    d.apply(mv("a27", 333.0)).unwrap();
    assert_eq!(changed_lines(&src, d.text()), 1);
}

#[test]
fn fr_fmt_04_unknowns_survive_a_move() {
    let src = "---\nfoo: bar\n---\n<!-- text id=t1 x=1 y=2 w=3 h=4 foo=bar -->\nHi\n<!-- widget id=w1 spin=fast -->\n";
    let mut d = Deck::parse(src).deck;
    assert_eq!(d.text(), src);
    d.apply(mv("t1", 9.0)).unwrap();
    assert_eq!(
        d.text(),
        "---\nfoo: bar\n---\n<!-- text id=t1 x=9 y=2 w=3 h=4 foo=bar -->\nHi\n<!-- widget id=w1 spin=fast -->\n"
    );
    assert_eq!(d.element("w1").unwrap().attr("spin"), Some("fast"));
}

#[test]
fn fr_fmt_05_newer_version_is_read_only() {
    let p = Deck::parse("---\ns-md-slides: 2\n---\n<!-- text id=t1 x=1 y=1 w=1 h=1 -->\n");
    assert!(p.deck.is_read_only());
    assert!(p.diagnostics.iter().any(|d| d.code == "E-VERSION"));
    let mut d = p.deck;
    let before = d.text().to_string();
    assert_eq!(d.apply(mv("t1", 5.0)).unwrap_err().code, "E-READ-ONLY");
    assert_eq!(d.text(), before);
    assert!(!Deck::parse("# no version\n").deck.is_read_only());
}

#[test]
fn fr_fmt_06_bad_attribute_keeps_the_deck() {
    let src = "<!-- text id=a x=1 y=1 w=1 h=1 -->\nA\n---\n<!-- text id=b x=1 y=1 w=1 h=1 -->\nB\n---\n<!-- text id=c x=abc y=1 w=1 h=1 -->\nC\n---\nD\n";
    let p = Deck::parse(src);
    assert_eq!(p.deck.slides.len(), 4);
    let errors: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].code, "E-ATTR-VALUE");
    assert_eq!((errors[0].line, errors[0].column), (7, 18));
    assert_eq!(errors[0].slide, Some(2));
}

#[test]
fn fr_fmt_07_ids_assigned_and_written_on_edit() {
    let src = "<!-- text x=1 y=1 w=100 h=100 -->\nA\n<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nB\n";
    let mut d = Deck::parse(src).deck;
    let id = d.slides[0].elements[0].id.clone();
    assert_eq!(id, "t2", "skips t1, which is taken");
    assert!(!d.slides[0].elements[0].id_written());
    assert_eq!(d.text(), src, "no edit, nothing new");
    d.apply(Op::SetBody {
        id: id.clone(),
        text: "A2".into(),
    })
    .unwrap();
    assert_eq!(
        d.text(),
        "<!-- text x=1 y=1 w=100 h=100 id=t2 -->\nA2\n<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nB\n"
    );
}

#[test]
fn fr_fmt_07_in_memory_ids_survive_other_edits() {
    let src = "<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nA\n<!-- text x=1 y=1 w=1 h=1 -->\nB\n";
    let mut d = Deck::parse(src).deck;
    assert_eq!(d.slides[0].elements[1].id, "t2");
    d.apply(Op::RemoveElement { id: "t1".into() }).unwrap();
    assert_eq!(d.slides[0].elements[0].id, "t2", "id did not drift to t1");
}

#[test]
fn fr_fmt_08_plain_markdown_opens_as_slides() {
    let d = Deck::parse("# One\n\ntext\n\n---\n\n# Two\n\n---\n\n# Three\n").deck;
    assert_eq!(d.slides.len(), 3);
    for s in &d.slides {
        assert_eq!(s.elements.len(), 1);
        let e = &s.elements[0];
        assert!(e.implicit);
        assert_eq!((e.x, e.y, e.w, e.h), (96.0, 54.0, 1728.0, 972.0));
    }
    assert_eq!(d.slides[1].elements[0].body, "# Two\n");
    assert!(Deck::parse("").deck.slides.is_empty());
}

#[test]
fn fr_fmt_08_editing_implicit_text_writes_a_directive() {
    let src = "# One\n---\n# Two\n";
    let mut d = Deck::parse(src).deck;
    let id = d.slides[1].elements[0].id.clone();
    let undo = d.apply(mv(&id, 200.0)).unwrap();
    assert_eq!(
        d.text(),
        format!("# One\n---\n<!-- text id={id} x=200 y=54 w=1728 h=972 -->\n# Two\n")
    );
    assert!(!d.slides[1].elements[0].implicit);
    d.apply(undo).unwrap();
    assert_eq!(d.text(), src);
}

#[test]
fn fr_canvas_01_three_forms_and_default() {
    let c = |fm: &str| Deck::parse(&format!("---\n{fm}\n---\n")).deck.canvas();
    assert_eq!(
        c("canvas: 16:9"),
        Canvas {
            width: 1920,
            height: 1080
        }
    );
    assert_eq!(
        c("canvas: 4:5"),
        Canvas {
            width: 1080,
            height: 1350
        }
    );
    assert_eq!(
        c("canvas: 1200x800"),
        Canvas {
            width: 1200,
            height: 800
        }
    );
    assert_eq!(
        c("title: none"),
        Canvas {
            width: 1920,
            height: 1080
        }
    );
}

#[test]
fn fr_canvas_02_off_canvas_kept_with_one_warning() {
    let src = "<!-- text id=t1 x=-200 y=10 w=100 h=100 -->\nHi\n";
    let p = Deck::parse(src);
    assert_eq!(p.deck.text(), src);
    assert_eq!(p.deck.slides[0].elements[0].x, -200.0);
    let w: Vec<_> = p
        .diagnostics
        .iter()
        .filter(|d| d.code == "W-OFF-CANVAS")
        .collect();
    assert_eq!(w.len(), 1);
    assert_eq!(p.diagnostics.len(), 1);
}

#[test]
fn fr_notes_01_notes_parse_and_round_trip() {
    let src = "<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nOn slide\n\n<!-- notes -->\nSay *this*.\n";
    let mut d = Deck::parse(src).deck;
    assert_eq!(d.slides[0].notes.as_deref(), Some("Say *this*.\n"));
    assert_eq!(d.slides[0].elements[0].body, "On slide\n");
    let undo = d.apply(mv("t1", 5.0)).unwrap();
    d.apply(undo).unwrap();
    assert_eq!(d.text(), src);
}

#[test]
fn fr_dir_01_auto_resolves_from_first_strong_char() {
    let d = Deck::parse(
        "<!-- text id=ar x=1 y=1 w=1 h=1 -->\n# مرحبا\n<!-- text id=en x=1 y=1 w=1 h=1 -->\n# Hello\n<!-- text id=f x=1 y=1 w=1 h=1 dir=ltr -->\nمرحبا\n",
    )
    .deck;
    assert_eq!(d.resolved_dir(d.element("ar").unwrap()), Dir::Rtl);
    assert_eq!(d.resolved_dir(d.element("en").unwrap()), Dir::Ltr);
    assert_eq!(d.resolved_dir(d.element("f").unwrap()), Dir::Ltr);
    let rtl = Deck::parse("---\ndir: rtl\n---\n<!-- text id=en x=1 y=1 w=1 h=1 -->\nHello\n").deck;
    assert_eq!(rtl.resolved_dir(rtl.element("en").unwrap()), Dir::Rtl);
}

#[test]
fn limits_make_the_deck_read_only() {
    let src = "x\n---\n".repeat(s_md_slides::MAX_SLIDES + 1);
    let p = Deck::parse(&src);
    assert!(p.deck.is_read_only());
    assert!(p.diagnostics.iter().any(|d| d.code == "E-LIMIT"));
    assert_eq!(p.deck.text(), src);

    let long = "a".repeat(s_md_slides::MAX_ATTR_VALUE_BYTES + 1);
    let p = Deck::parse(&format!(
        "<!-- text id=t1 x=1 y=1 w=1 h=1 title={long} -->\n"
    ));
    assert!(p.diagnostics.iter().any(|d| d.code == "E-LIMIT"));
}

#[test]
fn diagnostics_for_common_mistakes() {
    let codes = |src: &str| {
        Deck::parse(src)
            .diagnostics
            .iter()
            .map(|d| d.code)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes("<!-- text id=a x=1 y=1 w=1 h=1 -->\n<!-- text id=a x=1 y=1 w=1 h=1 -->\n"),
        ["E-DUP-ID"]
    );
    assert_eq!(
        codes("<!-- text id=a y=1 w=1 h=1 -->\n"),
        ["E-ATTR-MISSING"]
    );
    assert_eq!(
        codes("<!-- text id=a x=1 x=2 y=1 w=1 h=1 -->\n"),
        ["W-DUP-ATTR"]
    );
    assert_eq!(
        codes("<!-- text id=a x=1 y=1 w=-1 h=1 -->\n"),
        ["E-ATTR-VALUE"]
    );
    assert_eq!(codes("```\n---\n"), ["E-UNCLOSED-FENCE"]);
    assert_eq!(codes("---\ntitle: x\n"), ["E-FRONTMATTER"]);
    assert_eq!(
        codes("<!-- notes -->\nn\n<!-- text id=a x=1 y=1 w=1 h=1 -->\n"),
        ["W-NOTES-ORDER"]
    );
    assert_eq!(
        codes("<!-- shape id=s kind=arrow x=1 y=1 w=10 -->\n"),
        Vec::<&str>::new()
    );
    assert_eq!(
        codes("<!-- text id=a x=1 =y w=1 h=1 -->\n"),
        [
            "E-ATTR-SYNTAX",
            "E-ATTR-MISSING",
            "E-ATTR-MISSING",
            "E-ATTR-MISSING"
        ]
    );
}

#[test]
fn body_edits_refuse_structure_changes() {
    let src = "<!-- text id=t1 x=1 y=1 w=1 h=1 -->\nA\n";
    let mut d = Deck::parse(src).deck;
    for bad in ["a\n---\nb", "<!-- text id=x x=1 y=1 w=1 h=1 -->"] {
        assert_eq!(
            d.apply(Op::SetBody {
                id: "t1".into(),
                text: bad.into()
            })
            .unwrap_err()
            .code,
            "E-STRUCTURE"
        );
        assert_eq!(d.text(), src);
    }
    assert_eq!(
        d.apply(Op::RemoveElement { id: "nope".into() })
            .unwrap_err()
            .code,
        "E-NOT-FOUND"
    );
    let stale = Op::Replace {
        at: 0,
        old: "zzz".into(),
        new: String::new(),
    };
    assert_eq!(d.apply(stale).unwrap_err().code, "E-STALE");
}

#[test]
fn ops_round_trip_through_json() {
    let ops = vec![
        mv("t1", 3.5),
        Op::SetBody {
            id: "t1".into(),
            text: "x".into(),
        },
        Op::RemoveElement { id: "t1".into() },
        Op::Replace {
            at: 4,
            old: "a".into(),
            new: "b".into(),
        },
        Op::Batch {
            ops: vec![mv("a", 1.0)],
        },
    ];
    for op in ops {
        let json = serde_json::to_string(&op).unwrap();
        assert_eq!(serde_json::from_str::<Op>(&json).unwrap(), op, "{json}");
    }
    let parsed: Op = serde_json::from_str(r#"{"op":"set_geometry","id":"t1","x":10}"#).unwrap();
    assert_eq!(parsed, mv("t1", 10.0));
}
