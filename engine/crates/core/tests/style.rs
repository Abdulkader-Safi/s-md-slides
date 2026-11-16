//! M3: element kinds, themes, masters, layouts and backgrounds (FR-EL-01 to 09, FR-STYLE-01 to 05).

use s_md_slides::{Asset, AssetResolver, Background, Content, Deck, Theme};
use std::collections::HashMap;

fn codes(src: &str) -> Vec<&'static str> {
    Deck::parse(src)
        .diagnostics
        .iter()
        .map(|d| d.code)
        .collect()
}

struct Files(HashMap<&'static str, &'static str>);

impl AssetResolver for Files {
    fn resolve(&self, path: &str) -> Option<Asset> {
        self.0.get(path).map(|s| Asset {
            bytes: s.as_bytes().to_vec(),
            mime: "application/json".into(),
        })
    }
}

#[test]
fn default_json_matches_built_in_theme() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../themes/default.json");
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let (t, problems) = Theme::from_json(&v);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(t, Theme::default());
}

#[test]
fn fr_el_01_text_style() {
    let d = Deck::parse(
        "<!-- text id=t1 x=0 y=0 w=100 h=100 size=48 align=center color=#fff -->\n# Hi *there*\n",
    )
    .deck;
    let e = d.element("t1").unwrap();
    assert_eq!(
        (
            e.style.size.as_deref(),
            e.style.align.as_deref(),
            e.style.color.as_deref()
        ),
        (Some("48"), Some("center"), Some("#fff"))
    );
    let r = d.resolved_style("t1").unwrap();
    assert_eq!(
        (r.size, r.align.as_str(), r.color.as_str(), r.font.as_str()),
        (48.0, "center", "#fff", "Noto Sans")
    );
    assert_eq!(
        codes("<!-- text id=t x=0 y=0 w=1 h=1 align=middle size=-3 weight=450 -->\n"),
        ["E-ATTR-VALUE"; 3]
    );
}

#[test]
fn fr_el_02_image() {
    let d = Deck::parse("<!-- image id=i1 x=0 y=0 w=10 h=10 fit=cover crop=0,0,50,50 -->\n![Revenue chart](assets/chart.png)\n").deck;
    assert_eq!(
        d.element("i1").unwrap().content,
        Some(Content::Image {
            path: "assets/chart.png".into(),
            alt: "Revenue chart".into(),
            fit: "cover".into(),
            crop: Some([0.0, 0.0, 50.0, 50.0])
        })
    );
    assert_eq!(
        codes("<!-- image id=i x=0 y=0 w=1 h=1 -->\n![](a.png)\n"),
        ["W-NO-ALT"]
    );
    assert_eq!(
        codes("<!-- image id=i x=0 y=0 w=1 h=1 -->\nNo image here\n"),
        ["E-IMAGE-BODY"]
    );
    assert_eq!(
        codes("<!-- image id=i x=0 y=0 w=1 h=1 fit=stretch -->\n![a](b.png)\n"),
        ["E-ATTR-VALUE"]
    );
}

#[test]
fn fr_el_03_shapes() {
    for kind in ["rect", "ellipse", "line", "arrow"] {
        let src = format!(
            "<!-- shape id=s x=0 y=0 w=100 h=50 kind={kind} fill=accent2 stroke=#e11 stroke-width=3 radius=8 -->\n"
        );
        let d = Deck::parse(&src).deck;
        assert!(d.diagnostics().is_empty(), "{kind}: {:?}", d.diagnostics());
        assert!(
            matches!(&d.element("s").unwrap().content, Some(Content::Shape { shape, .. }) if shape == kind)
        );
        let r = d.resolved_style("s").unwrap();
        assert_eq!(
            (
                r.fill.as_deref(),
                r.stroke.as_deref(),
                r.stroke_width,
                r.radius
            ),
            (Some("#db2777"), Some("#e11"), 3.0, 8.0)
        );
    }
    assert_eq!(
        codes("<!-- shape id=s x=0 y=0 w=1 h=1 kind=star -->\n"),
        ["E-ATTR-VALUE"]
    );
    assert_eq!(
        codes("<!-- shape id=s x=0 y=0 w=1 kind=line -->\nlabel\n"),
        ["W-SHAPE-TEXT"]
    );
    assert_eq!(
        codes("<!-- shape id=s x=0 y=0 w=1 h=1 kind=ellipse -->\nInside text is fine\n"),
        Vec::<&str>::new()
    );
}

#[test]
fn fr_el_04_code() {
    let src = "<!-- code id=c x=0 y=0 w=10 h=10 lines=2-3 -->\n```rust\nfn a() {}\nfn b() {}\nfn c() {}\n```\n";
    let d = Deck::parse(src).deck;
    match &d.element("c").unwrap().content {
        Some(Content::Code { lang, lines, code }) => {
            assert_eq!((lang.as_str(), lines.as_slice()), ("rust", &[[2, 3]][..]));
            assert!(code.starts_with("fn a()"));
        }
        other => panic!("{other:?}"),
    }
    let r = d.resolved_style("c").unwrap();
    assert_eq!(
        (r.font.as_str(), r.fill.as_deref()),
        ("Noto Sans Mono", Some("#f3f4f6"))
    );
    assert_eq!(d.text(), src, "language tag round-trips");
    assert_eq!(
        codes("<!-- code id=c x=0 y=0 w=1 h=1 -->\nnot a fence\n"),
        ["E-CODE-BODY"]
    );
    assert_eq!(
        codes("<!-- code id=c x=0 y=0 w=1 h=1 lines=x -->\n```js\n1\n```\n"),
        ["E-ATTR-VALUE"]
    );
}

#[test]
fn fr_el_05_to_09_other_kinds() {
    let table = "<!-- table id=b x=0 y=0 w=1 h=1 header=false -->\n| A | B | C |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n| 7 | 8 | 9 |\n";
    assert_eq!(
        Deck::parse(table).deck.element("b").unwrap().content,
        Some(Content::Table { header: false })
    );
    assert_eq!(
        codes("<!-- table id=b x=0 y=0 w=1 h=1 -->\njust text\n"),
        ["E-TABLE-BODY"]
    );

    let diagram = "<!-- diagram id=d x=0 y=0 w=1 h=1 -->\n```mermaid\ngraph TD\n  A --> B\n```\n";
    assert_eq!(
        Deck::parse(diagram).deck.element("d").unwrap().content,
        Some(Content::Diagram {
            source: "graph TD\n  A --> B\n".into()
        })
    );
    assert_eq!(
        codes("<!-- diagram id=d x=0 y=0 w=1 h=1 -->\n```js\nx\n```\n"),
        ["E-DIAGRAM-BODY"]
    );

    let math = "<!-- math id=m x=0 y=0 w=1 h=1 -->\n```math\n\\int_0^1 x\\,dx\n```\n";
    assert_eq!(
        Deck::parse(math).deck.element("m").unwrap().content,
        Some(Content::Math {
            source: "\\int_0^1 x\\,dx".into()
        })
    );
    assert!(
        codes("<!-- text id=t x=0 y=0 w=1 h=1 -->\nInline $a^2$ math stays text.\n").is_empty()
    );

    assert_eq!(
        codes("<!-- video id=v x=0 y=0 w=1 h=1 -->\n"),
        ["E-ATTR-MISSING"]
    );
    assert_eq!(
        codes("<!-- embed id=e x=0 y=0 w=1 h=1 src=http://example.com -->\n"),
        ["E-ATTR-VALUE"]
    );
    assert!(codes("<!-- embed id=e x=0 y=0 w=1 h=1 src=https://player.vimeo.com/video/1 poster=p.jpg -->\n").is_empty());
}

#[test]
fn fr_style_01_theme_roles_inline_and_from_file() {
    let inline = "---\ntheme:\n  colors:\n    accent1: \"#e11d48\"\n  fonts:\n    heading: Inter\n---\n<!-- text id=t x=0 y=0 w=1 h=1 color=accent1 font=heading -->\nHi\n";
    let d = Deck::parse(inline).deck;
    let r = d.resolved_style("t").unwrap();
    assert_eq!((r.color.as_str(), r.font.as_str()), ("#e11d48", "Inter"));

    let src = "---\ntheme: themes/brand.json\n---\n<!-- text id=t x=0 y=0 w=1 h=1 color=accent1 -->\nHi\n";
    let mut d = Deck::parse(src).deck;
    assert_eq!(
        d.resolved_style("t").unwrap().color,
        "#2563eb",
        "default until the file is loaded"
    );
    let files = Files(HashMap::from([(
        "themes/brand.json",
        r##"{"colors": {"accent1": "#00aa55"}}"##,
    )]));
    assert!(d.load_theme(&files).is_empty());
    assert_eq!(
        d.resolved_style("t").unwrap().color,
        "#00aa55",
        "changing the theme file changes the result"
    );
    // The loaded theme survives an edit elsewhere.
    d.apply(s_md_slides::Op::SetBody {
        id: "t".into(),
        text: "Bye".into(),
    })
    .unwrap();
    assert_eq!(d.resolved_style("t").unwrap().color, "#00aa55");
    let missing = Files(HashMap::new());
    assert_eq!(
        Deck::parse(src).deck.load_theme(&missing)[0].code,
        "W-MISSING-ASSET"
    );
}

#[test]
fn fr_style_02_literal_and_role_values_round_trip() {
    let src = "<!-- text id=a x=0 y=0 w=1 h=1 color=#e11 font=Inter -->\nA\n<!-- text id=b x=0 y=0 w=1 h=1 color=accent1 font=heading -->\nB\n";
    let d = Deck::parse(src).deck;
    assert_eq!(d.text(), src);
    assert_eq!(d.element("a").unwrap().style.color.as_deref(), Some("#e11"));
    assert_eq!(
        d.element("b").unwrap().style.color.as_deref(),
        Some("accent1")
    );
    assert_eq!(
        codes("<!-- text id=a x=0 y=0 w=1 h=1 color=red -->\n"),
        ["E-ATTR-VALUE"]
    );
}

#[test]
fn fr_style_03_master_footer_numbers() {
    let mut src = String::from(
        "---\ntitle: Deck\n---\n<!-- master id=m -->\n<!-- text id=foot x=0 y=1000 w=400 h=40 -->\n{slide} / {total} · {title}\n\n---\n",
    );
    for i in 0..12 {
        if i > 0 {
            src.push_str("---\n");
        }
        src.push_str(&format!("# Slide {i}\n"));
    }
    let d = Deck::parse(&src).deck;
    let foot = d.master_elements(2)[0];
    assert_eq!(d.master_text(&foot.body, 2), "3 / 12 · Deck\n");
}

#[test]
fn fr_style_04_layouts_master_none_and_hide() {
    let src = "<!-- master id=brand -->\n<!-- image id=logo x=0 y=0 w=10 h=10 -->\n![Logo](l.png)\n<!-- text id=foot x=0 y=0 w=10 h=10 -->\nf\n<!-- layout id=tb master=brand -->\n<!-- placeholder id=title x=120 y=80 w=1680 h=200 size=72 -->\n<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->\n---\n# A\n---\n<!-- slide master=none -->\n# B\n---\n<!-- slide hide=logo -->\n# C\n";
    let mut d = Deck::parse(src).deck;
    assert!(d.diagnostics().is_empty(), "{:?}", d.diagnostics());
    let ids = |d: &Deck, i| {
        d.master_elements(i)
            .iter()
            .map(|e| e.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(&d, 0), ["logo", "foot"]);
    assert!(ids(&d, 1).is_empty(), "master=none drops the logo");
    assert_eq!(ids(&d, 2), ["foot"]);
    d.apply(s_md_slides::Op::AddSlide {
        index: 3,
        layout: Some("tb".into()),
        raw: None,
    })
    .unwrap();
    let s = &d.slides[3];
    assert_eq!(s.elements.len(), 2);
    assert_eq!(
        (s.elements[0].x, s.elements[0].y, s.elements[1].y),
        (120.0, 80.0, 320.0)
    );
    assert_eq!(
        s.elements[0].style.size.as_deref(),
        Some("72"),
        "placeholder style carries over"
    );
    assert_eq!(codes("<!-- slide layout=nope -->\n"), ["E-UNKNOWN-LAYOUT"]);
    assert_eq!(codes("<!-- slide master=nope -->\n"), ["E-UNKNOWN-MASTER"]);
}

#[test]
fn fr_style_05_backgrounds() {
    let src = "---\ntheme:\n  background: accent2\n---\n<!-- slide bg=#0b1020 -->\n---\n<!-- slide bg=assets/hero.jpg -->\n---\n<!-- slide bg=linear(90,accent1,#000) -->\n---\n# theme default\n";
    let d = Deck::parse(src).deck;
    assert!(d.diagnostics().is_empty(), "{:?}", d.diagnostics());
    assert_eq!(
        d.background(0),
        Background::Color {
            color: "#0b1020".into()
        }
    );
    assert_eq!(
        d.background(1),
        Background::Image {
            path: "assets/hero.jpg".into()
        }
    );
    assert_eq!(
        d.background(2),
        Background::Gradient {
            angle: 90.0,
            stops: vec!["#2563eb".into(), "#000".into()]
        }
    );
    assert_eq!(
        d.background(3),
        Background::Color {
            color: "#db2777".into()
        }
    );
    assert_eq!(codes("<!-- slide bg=blue -->\n"), ["E-ATTR-VALUE"]);
}

#[test]
fn groups_need_members_on_the_slide() {
    assert_eq!(
        codes("<!-- text id=a x=0 y=0 w=1 h=1 -->\n<!-- group id=g items=a,zz -->\n"),
        ["E-GROUP"]
    );
}
