# s-md-slides

Slide decks you can read in any text editor. Every box sits exactly where you dragged it, like in PowerPoint or Google Slides, and the whole deck is one markdown file.

```markdown
---
s-md-slides: 1
title: Q3 review
canvas: 16:9
---

<!-- text id=t1 x=120 y=80 w=1680 h=200 size=72 weight=700 -->
# Revenue grew 18%

<!-- image id=i1 x=120 y=320 w=900 h=600 fit=contain -->
![Revenue by month](assets/revenue.png)

<!-- notes -->
Mention the new pricing first.
```

Positions live in one-line HTML comments. The content under each comment is normal markdown, so the file still makes sense on GitHub, in Obsidian or in `cat`. Moving a box in an editor changes one line of the file, which keeps git diffs small.

## Status

Pre-alpha. Nothing here works yet. The build follows [engine/spec/06-build-spec.md](engine/spec/06-build-spec.md), milestone by milestone, and progress is logged in [engine/PROGRESS.md](engine/PROGRESS.md).

## What the engine will do

The engine is a Rust library in [`engine/`](engine/). It will:

- parse a deck into a model and report problems with line and column, without ever failing the whole file
- edit the deck through named operations (move, resize, set text, add slide and so on), each of which returns its own undo
- save by splicing only the edited bytes back into the original text, so an unchanged deck saves byte for byte the same
- export to an HTML presentation with presenter view, HTML or SVG per slide, PDF with selectable text, PNG, JPEG and GIF
- ship a CLI: `s-md-slides validate`, `export`, `serve` (live preview in the browser) and `apply` (run a JSON batch of operations)

Arabic and other right-to-left text is supported. Positions are never mirrored.

## Repository layout

```
engine/             the Rust engine (Cargo workspace, three crates)
  spec/             product, users, requirements, tech decisions, build spec
s-md-slides-tasks/  md-kanban task board, one markdown file per task
```

The engine is project 1 of 4. A GPUI desktop app, an npm package and an Obsidian plugin will build on it, each in its own repository.

## License

MIT.
