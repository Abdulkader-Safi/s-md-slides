//! NFR-01: parse plus save of a 200-slide, 2,000-element deck, and one op plus save.

use criterion::{Criterion, criterion_group, criterion_main};
use s_md_slides::{Deck, Op};
use std::hint::black_box;

/// 200 slides of ten elements each: titles, bullets, shapes, code and a table.
pub fn big_deck() -> String {
    let mut s = String::from("---\ns-md-slides: 1\ntitle: Benchmark\n---\n");
    for i in 0..200 {
        if i > 0 {
            s.push_str("\n---\n\n");
        }
        s.push_str(&format!(
            "<!-- text id=a{i} x=120 y=80 w=1680 h=200 size=72 -->\n# Slide {i}\n\n"
        ));
        s.push_str(&format!("<!-- text id=b{i} x=120 y=320 w=800 h=400 enter=fade order=1 -->\n- One point\n- Two **points**\n\n"));
        for j in 0..6 {
            s.push_str(&format!(
                "<!-- shape id=s{i}-{j} kind=rect x={} y=760 w=200 h=200 fill=accent1 -->\n\n",
                120 + j * 260
            ));
        }
        s.push_str(&format!(
            "<!-- code id=c{i} x=1000 y=320 w=800 h=200 -->\n```rust\nlet x = {i};\n```\n\n"
        ));
        s.push_str(&format!("<!-- table id=t{i} x=1000 y=560 w=800 h=160 -->\n| A | B |\n|---|---|\n| 1 | 2 |\n\n<!-- notes -->\nNotes {i}\n"));
    }
    s
}

fn benches(c: &mut Criterion) {
    let text = big_deck();
    let deck = Deck::parse(&text).deck;
    assert_eq!(deck.slides.len(), 200);
    assert_eq!(
        deck.slides.iter().map(|s| s.elements.len()).sum::<usize>(),
        2000
    );
    c.bench_function("parse and save 200 slides", |b| {
        b.iter(|| black_box(Deck::parse(black_box(&text)).deck.text().len()))
    });
    c.bench_function("one op and save", |b| {
        b.iter_batched(
            || deck.clone(),
            |mut d| {
                d.apply(Op::SetGeometry {
                    id: "s150-3".into(),
                    x: Some(300.0),
                    y: None,
                    w: None,
                    h: None,
                    rot: None,
                })
                .unwrap();
                black_box(d.text().len())
            },
            criterion::BatchSize::LargeInput,
        )
    });
}

criterion_group!(group, benches);
criterion_main!(group);
