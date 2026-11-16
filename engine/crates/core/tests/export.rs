//! M4: slide HTML, browser SVG, the presentation and assets (FR-EXP-01, 02, 07,
//! FR-ASSET-01 to 03, and the export halves of FR-EL and FR-STYLE checks).

use s_md_slides::export::{Frame, Options, presentation_html, slide_html, slide_svg};
use s_md_slides::{Asset, AssetResolver, Deck};
use std::cell::RefCell;
use std::collections::HashMap;

/// A resolver that counts every request.
struct Files {
    files: HashMap<&'static str, (&'static [u8], &'static str)>,
    asked: RefCell<Vec<String>>,
}

impl Files {
    fn new(files: &[(&'static str, &'static [u8], &'static str)]) -> Self {
        Files {
            files: files.iter().map(|(p, b, m)| (*p, (*b, *m))).collect(),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl AssetResolver for Files {
    fn resolve(&self, path: &str) -> Option<Asset> {
        self.asked.borrow_mut().push(path.to_string());
        self.files.get(path).map(|(b, m)| Asset {
            bytes: b.to_vec(),
            mime: m.to_string(),
        })
    }
}

fn html(src: &str) -> String {
    slide_html(&Deck::parse(src).deck, 0, Frame::End, &Options::default())
        .unwrap()
        .text
}

#[test]
fn fr_exp_07_script_and_onerror_removed_with_warning() {
    let src = "<!-- text id=t1 x=0 y=0 w=500 h=500 -->\nHello <script>alert('x')</script>\n\n<img src=\"a.png\" onerror=\"alert(1)\">\n";
    let d = Deck::parse(src).deck;
    for out in [
        slide_html(&d, 0, Frame::End, &Options::default()).unwrap(),
        slide_svg(&d, 0, Frame::End, &Options::default()).unwrap(),
        presentation_html(&d, &Options::default()),
    ] {
        assert!(!out.text.contains("alert"), "script or handler survived");
        let w: Vec<_> = out
            .diagnostics
            .iter()
            .filter(|x| x.code == "W-HTML-REMOVED")
            .collect();
        assert_eq!(w.len(), 1, "{:?}", out.diagnostics);
        assert!(
            w[0].message.contains("<script>") && w[0].message.contains("onerror"),
            "{}",
            w[0].message
        );
        assert_eq!((w[0].line, w[0].element.as_deref()), (1, Some("t1")));
    }
}

#[test]
fn fr_exp_01_svg_end_state_places_every_element() {
    let src = "<!-- text id=a x=100 y=200 w=400 h=100 enter=fly-left order=1 -->\nA\n<!-- shape id=s kind=rect x=700 y=50 w=300 h=300 enter=fade order=2 -->\n<!-- image id=i x=1200 y=600 w=400 h=300 -->\n![Pic](p.png)\n";
    let d = Deck::parse(src).deck;
    let svg = slide_svg(&d, 0, Frame::End, &Options::default())
        .unwrap()
        .text;
    let doc = roxmltree::Document::parse(&svg).expect("well-formed XML");
    let g = |id: &str| {
        doc.descendants()
            .find(|n| n.attribute("data-id") == Some(id))
            .map(|n| {
                (
                    n.attribute("transform").unwrap_or("").to_string(),
                    n.attribute("opacity").map(str::to_string),
                )
            })
    };
    assert_eq!(g("a"), Some(("translate(100 200)".into(), None)));
    assert_eq!(g("s"), Some(("translate(700 50)".into(), None)));
    assert_eq!(g("i"), Some(("translate(1200 600)".into(), None)));
    // At the start, a waits off-canvas to the left and s is transparent.
    let start = slide_svg(&d, 0, Frame::Start, &Options::default())
        .unwrap()
        .text;
    assert!(
        start.contains("data-id=\"a\" transform=\"translate(-400 200)\""),
        "{start}"
    );
    assert!(start.contains("data-id=\"s\" transform=\"translate(700 50)\" opacity=\"0\""));
    roxmltree::Document::parse(
        &slide_svg(
            &Deck::parse(include_str!("../../../tests/decks/15-all-kinds.md")).deck,
            0,
            Frame::End,
            &Options::default(),
        )
        .unwrap()
        .text,
    )
    .expect("all kinds give well-formed SVG");
}

#[test]
fn fr_el_01_html_renders_text_style() {
    let h = html(
        "<!-- text id=t1 x=10 y=20 w=500 h=300 size=48 align=center color=#fff -->\n# Big *title*\n",
    );
    assert!(
        h.contains("font-size:48px") && h.contains("text-align:center") && h.contains("color:#fff"),
        "{h}"
    );
    assert!(h.contains("left:10px;top:20px;width:500px;height:300px"));
    assert!(h.contains("<h1>Big <em>title</em></h1>"));
}

#[test]
fn fr_el_03_04_05_shapes_code_tables() {
    let svg = slide_svg(
        &Deck::parse(include_str!("../../../tests/decks/18-shapes.md")).deck,
        0,
        Frame::End,
        &Options::default(),
    )
    .unwrap()
    .text;
    assert!(
        svg.contains(
            "<rect x=\"0\" y=\"0\" width=\"400\" height=\"200\" rx=\"16\" fill=\"#2563eb\""
        ),
        "rect"
    );
    assert!(
        svg.contains("<ellipse cx=\"150\" cy=\"150\" rx=\"150\" ry=\"150\""),
        "ellipse"
    );
    assert!(
        svg.contains(
            "<line x1=\"0\" y1=\"0\" x2=\"800\" y2=\"0\" stroke=\"#ea580c\" stroke-width=\"6\""
        ),
        "line"
    );
    assert!(svg.contains("marker-end=\"url(#smd-a-s4)\""), "arrow head");

    let h = html(
        "<!-- code id=c x=0 y=0 w=900 h=500 lines=2 -->\n```rust\nfn a() {}\nfn b() {}\n```\n",
    );
    assert!(
        h.contains("data-lang=\"rust\"")
            && h.contains("font-family:'Noto Sans Mono'")
            && h.contains("background:#f3f4f6"),
        "{h}"
    );
    assert!(h.contains(
        "<span class=\"smd-line\">fn a() {}</span><span class=\"smd-line smd-hl\">fn b() {}</span>"
    ));

    let h = html(
        "<!-- table id=b x=0 y=0 w=900 h=500 -->\n| A | B | C |\n|:--|:-:|--:|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n| 7 | 8 | 9 |\n",
    );
    assert_eq!(h.matches("<tr>").count(), 4, "header plus three rows");
    assert_eq!(h.matches("<td").count(), 9);
    assert!(
        h.contains("<th style=\"text-align: center\">B</th>")
            && h.contains("<td style=\"text-align: right\">3</td>"),
        "{h}"
    );
}

#[test]
fn fr_el_08_09_video_and_embed() {
    let h = html(
        "<!-- video id=v x=0 y=0 w=10 h=10 src=assets/demo.mp4 poster=assets/p.jpg muted loop -->\n<!-- embed id=e x=0 y=0 w=10 h=10 src=https://www.youtube.com/watch?v=abc123&t=5 -->\n",
    );
    assert!(
        h.contains("<video src=\"assets/demo.mp4\"")
            && h.contains("poster=\"assets/p.jpg\"")
            && h.contains("loop=\"\" muted=\"\""),
        "{h}"
    );
    assert!(
        h.contains("<iframe src=\"https://www.youtube-nocookie.com/embed/abc123\""),
        "{h}"
    );
}

#[test]
fn fr_asset_01_02_03_resolver_embed_and_missing() {
    let src = "<!-- slide bg=assets/bg.png -->\n<!-- image id=i1 x=0 y=0 w=10 h=10 -->\n![Chart](assets/chart.png)\n<!-- text id=t1 x=0 y=0 w=10 h=10 -->\nAgain ![Chart](assets/chart.png) and ![Gone](assets/gone.png)\n";
    let d = Deck::parse(src).deck;
    let files = Files::new(&[
        ("assets/chart.png", b"PNG!", "image/png"),
        ("assets/bg.png", b"BG", ""),
    ]);
    let out = presentation_html(
        &d,
        &Options {
            embed: true,
            resolver: Some(&files),
        },
    );
    let asked = files.asked.borrow();
    assert_eq!(
        asked.iter().filter(|p| *p == "assets/chart.png").count(),
        1,
        "asked once: {asked:?}"
    );
    assert!(
        out.text.contains("data:image/png;base64,UE5HIQ=="),
        "chart embedded"
    );
    assert!(
        out.text.contains("data:image/png;base64,Qkc="),
        "mime guessed from the extension"
    );
    assert!(
        !out.text.contains("assets/chart.png"),
        "no relative path left"
    );
    let missing: Vec<_> = out
        .diagnostics
        .iter()
        .filter(|x| x.code == "W-MISSING-ASSET")
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].message.contains("assets/gone.png"));
    assert_eq!(d.text(), src, "the deck keeps relative paths");
}

#[test]
fn fr_notes_01_notes_in_presenter_data_not_on_slide() {
    let d = Deck::parse(
        "<!-- text id=t x=0 y=0 w=10 h=10 -->\nOn slide\n<!-- notes -->\nSecret *speaker* note\n",
    )
    .deck;
    let out = presentation_html(&d, &Options::default()).text;
    let (body, data) = out.split_once("id=\"smd-data\"").unwrap();
    assert!(!body.contains("Secret"));
    assert!(data.contains("Secret <em>speaker<\\/em> note"), "{data}");
}

#[test]
fn fr_style_exports_masters_layouts_backgrounds_and_themes() {
    let mut src = String::from(
        "---\ntitle: T\n---\n<!-- master id=m -->\n<!-- image id=logo x=0 y=0 w=10 h=10 -->\n![Logo](l.png)\n<!-- text id=foot x=0 y=1000 w=400 h=40 -->\n{slide} / {total}\n---\n",
    );
    for i in 0..12 {
        if i > 0 {
            src.push_str("---\n");
        }
        src.push_str(if i == 1 {
            "<!-- slide master=none bg=#0b1020 -->\nx\n"
        } else {
            "x\n"
        });
    }
    let d = Deck::parse(&src).deck;
    let s3 = slide_html(&d, 2, Frame::End, &Options::default())
        .unwrap()
        .text;
    assert!(s3.contains("<p>3 / 12</p>"), "FR-STYLE-03");
    let s2 = slide_html(&d, 1, Frame::End, &Options::default())
        .unwrap()
        .text;
    assert!(
        !s2.contains("data-id=\"logo\""),
        "FR-STYLE-04: master=none drops the logo"
    );
    assert!(s2.contains("background:#0b1020"), "FR-STYLE-05");
    let img = Deck::parse("<!-- slide bg=assets/hero.jpg -->\nx\n").deck;
    assert!(
        slide_html(&img, 0, Frame::End, &Options::default())
            .unwrap()
            .text
            .contains("url(&quot;assets/hero.jpg&quot;)")
    );

    let themed = "---\ntheme: t.json\n---\n<!-- text id=t x=0 y=0 w=1 h=1 color=accent1 -->\nx\n";
    let mut d = Deck::parse(themed).deck;
    let before = slide_html(&d, 0, Frame::End, &Options::default())
        .unwrap()
        .text;
    d.load_theme(&Files::new(&[(
        "t.json",
        br##"{"colors":{"accent1":"#00aa55"}}"##,
        "",
    )]));
    let after = slide_html(&d, 0, Frame::End, &Options::default())
        .unwrap()
        .text;
    assert!(
        before.contains("color:#2563eb") && after.contains("color:#00aa55"),
        "FR-STYLE-01"
    );
}

#[test]
fn presentation_has_csp_runtime_and_data() {
    let d = Deck::parse(include_str!("../../../tests/decks/15-all-kinds.md")).deck;
    let out = presentation_html(&d, &Options::default()).text;
    assert!(out.starts_with("<!doctype html>"));
    assert!(
        out.contains("http-equiv=\"Content-Security-Policy\"")
            && out.contains("frame-src https://www.youtube-nocookie.com https://player.vimeo.com")
    );
    assert!(out.contains("<script type=\"application/json\" id=\"smd-data\">"));
    assert_eq!(
        out.matches("<section class=\"smd-slide\"").count(),
        d.slides.len()
    );
}

#[test]
fn unsafe_urls_never_reach_the_output() {
    let src = "---\ntitle: 'x\" onmouseover=\"alert(1)'\n---\n<!-- slide bg=javascript:alert(1).png -->\n<!-- embed id=e x=0 y=0 w=10 h=10 src=javascript:alert(2) -->\n<!-- video id=v x=0 y=0 w=10 h=10 src=javascript:alert(3) poster=javascript:alert(4) -->\n";
    let d = Deck::parse(src).deck;
    for out in [
        presentation_html(&d, &Options::default()).text,
        slide_svg(&d, 0, Frame::End, &Options::default())
            .unwrap()
            .text,
    ] {
        // Placeholders may show the bad source as escaped text; no attribute or CSS
        // URL may carry it.
        assert!(
            !out.contains("=\"javascript:") && !out.contains("url(&quot;javascript"),
            "{out}"
        );
        assert!(
            !out.contains("=\"x\" onmouseover"),
            "title escaped in attributes"
        );
    }
    let page = presentation_html(&d, &Options::default()).text;
    assert!(page.contains("aria-label=\"x&quot; onmouseover=&quot;alert(1)\""));
}
