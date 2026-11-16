---
s-md-slides: 1
title: Pitch
canvas: 16:9
transition: fade
theme:
  colors:
    accent1: "#2563eb"
    accent2: "#f59e0b"
  fonts:
    heading: Noto Sans
---

<!-- master id=brand -->
<!-- image id=logo x=1720 y=40 w=160 h=60 -->
![s-md logo](assets/logo.svg)

<!-- text id=footer x=120 y=1010 w=700 h=40 size=20 color=accent1 -->
{title}: {slide} / {total}

<!-- layout id=title-body master=brand -->
<!-- placeholder id=title x=120 y=80 w=1680 h=200 size=72 weight=bold -->
<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->

---

<!-- slide layout=title-body bg=assets/hero.png -->
<!-- text id=title x=120 y=380 w=1680 h=240 size=104 weight=bold color=#ffffff align=center valign=middle enter=fade dur=600 -->
# Slides you can diff

<!-- notes -->
Open with the problem: decks live in apps, not files.

---

<!-- slide layout=title-body transition=push dur=500 -->
<!-- text id=t2 x=120 y=80 w=1680 h=200 size=72 weight=bold -->
# Why it works

<!-- text id=b1 x=120 y=320 w=800 h=120 size=40 enter=fly-left delay=300 dur=500 order=1 -->
- One markdown file

<!-- text id=b2 x=120 y=460 w=800 h=120 size=40 enter=fly-left delay=300 dur=500 order=1 -->
- Free canvas layout

<!-- shape id=s1 kind=rect x=1000 y=320 w=800 h=500 fill=accent2 radius=24 enter=scale order=2 -->
Every edit is one line in git

<!-- notes -->
Two clicks on this slide: the bullets together, then the box.

---

<!-- slide layout=title-body -->
<!-- text id=t3 x=120 y=80 w=1680 h=200 size=72 weight=bold -->
# Growth

<!-- image id=chart x=120 y=320 w=1000 h=650 fit=contain enter=fade order=1 -->
![Revenue by quarter, rising each quarter](assets/chart.png)

<!-- table id=tbl x=1200 y=320 w=600 h=400 size=28 -->
| Quarter | Revenue |
|:--------|--------:|
| Q1 | 1.2 M |
| Q2 | 1.9 M |
| Q3 | 2.4 M |
| Q4 | 3.1 M |

---

<!-- slide transition=push -->
<!-- code id=c1 x=120 y=120 w=1680 h=700 lines=2 size=32 -->
```rust
let mut deck = Deck::parse(&text).deck;
let undo = deck.apply(op)?;
std::fs::write(path, deck.text())?;
```

<!-- notes -->
Close with the API: parse, apply, write.
