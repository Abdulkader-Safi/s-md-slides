# s-md-slides

Slide decks you can read in any text editor. Every box sits exactly where you put it, like in PowerPoint or Google Slides, and the whole deck is one markdown file.

```markdown
---
s-md-slides: 1
title: Q3 review
canvas: 16:9
---

<!-- text id=t1 x=120 y=80 w=1680 h=200 size=72 weight=bold -->
# Revenue grew 18%

<!-- image id=i1 x=120 y=320 w=900 h=600 fit=contain -->
![Revenue by month, rising each month](assets/revenue.png)

<!-- notes -->
Mention the new pricing first.
```

Positions live in one-line HTML comments, and the content under each comment is normal markdown. The file still reads well on GitHub, in Obsidian or in `cat`, and moving a box changes one line, so git diffs stay small.

## Install the command

macOS and Linux:

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/Abdulkader-Safi/s-md-slides/releases/latest/download/s-md-slides-cli-installer.sh | sh
```

Windows (PowerShell):

```powershell
powershell -ExecutionPolicy Bypass -c "irm https://github.com/Abdulkader-Safi/s-md-slides/releases/latest/download/s-md-slides-cli-installer.ps1 | iex"
```

With Rust installed, `cargo install s-md-slides-cli` works too. Prebuilt archives for each platform are on the [releases page](https://github.com/Abdulkader-Safi/s-md-slides/releases).

```sh
s-md-slides validate deck.md             # problems, with line and column
s-md-slides export deck.md --to pdf      # also html, png, jpeg, gif
s-md-slides serve deck.md                # live preview that reloads when you save
s-md-slides apply deck.md ops.json       # scripted edits; prints the undo
```

The HTML export is a full presentation in one file: keyboard and touch controls, a presenter view with notes and a timer, and click-by-click builds. The PDF keeps text selectable. Arabic and other right-to-left decks work, and positions are never mirrored.

## Use the library

```sh
cargo add s-md-slides
```

```rust
use s_md_slides::{Deck, Op};

let text = "<!-- text id=t1 x=100 y=100 w=800 h=200 -->\n# Hello\n";
let parsed = Deck::parse(text);
for d in &parsed.diagnostics {
    println!("{}:{} {} {}", d.line, d.column, d.code, d.message);
}
let mut deck = parsed.deck;
let undo = deck.apply(Op::SetGeometry { id: "t1".into(), x: Some(240.0), y: None, w: None, h: None, rot: None }).unwrap();
assert_eq!(deck.text(), "<!-- text id=t1 x=240 y=100 w=800 h=200 -->\n# Hello\n");
deck.apply(undo).unwrap();
assert_eq!(deck.text(), text);
```

A move rewrites one line and leaves every other byte alone. Each op returns its own undo, and a broken deck still loads, with its problems listed. The core crate has no file or network access and builds for WebAssembly. Native PNG, JPEG, GIF and PDF export is in `s-md-slides-render`.

## Docs

The [wiki](https://github.com/Abdulkader-Safi/s-md-slides/wiki) is the guide: [installing](https://github.com/Abdulkader-Safi/s-md-slides/wiki/Installation), [a first deck](https://github.com/Abdulkader-Safi/s-md-slides/wiki/Your-first-deck), each command ([validate](https://github.com/Abdulkader-Safi/s-md-slides/wiki/CLI-validate), [export](https://github.com/Abdulkader-Safi/s-md-slides/wiki/CLI-export), [serve](https://github.com/Abdulkader-Safi/s-md-slides/wiki/CLI-serve), [apply](https://github.com/Abdulkader-Safi/s-md-slides/wiki/CLI-apply)), the [ops reference](https://github.com/Abdulkader-Safi/s-md-slides/wiki/Ops-reference), and [using the library](https://github.com/Abdulkader-Safi/s-md-slides/wiki/Library-getting-started) from Rust.

- [The format](engine/docs/format.md): every element, attribute and setting, with examples that run as tests.
- [The agent skill](engine/skills/s-md-slides/SKILL.md): teaches Claude and other coding agents to write and edit decks.
- [Conformance vectors](engine/conformance/): 54 JSON cases another implementation can test itself against.
- [AGENTS.md](engine/AGENTS.md): for agents working on the engine itself.

## Status

Version 0.1, the first release. The build follows [engine/spec/06-build-spec.md](engine/spec/06-build-spec.md), and decisions are logged in [engine/PROGRESS.md](engine/PROGRESS.md). A desktop app, an npm package and an Obsidian plugin will build on this engine, each in its own repository.

## License

MIT. Third-party notices are generated with `cargo about`, and the bundled Noto fonts are under the SIL Open Font License.
