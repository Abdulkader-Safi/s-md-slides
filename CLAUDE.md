# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

`s-md-slides` stores a free-canvas slide deck in one markdown file. Element positions live in one-line HTML comments (`<!-- text id=t1 x=120 y=80 w=600 h=200 -->`) and the content under each is normal markdown. The Rust engine lives in `engine/`. The build spec is `engine/spec/06-build-spec.md` and it is self-contained: build from it, milestone by milestone (M0 to M7), in order. Files 01 to 05 in `engine/spec/` are background.

## Workflow rules (from the user, always follow)

- Track every task on the md-kanban board in `s-md-slides-tasks/` with the `md-kanban` skill (`/mdk`). Before starting a task set its `status` to `in-progress`. When it is finished set `done`, or `review` if the user should check it. Refresh `modified` on every edit.
- Work on one branch per milestone (`m0-de-risk`, `m1-format-core` and so on). Commit after each finished task, push to GitHub, and keep one open PR per milestone that lists its tasks as a checklist. End the milestone with a commit naming it.
- Write every piece of prose (README, docs, code comments, task cards, commit messages, PR bodies, the skill file) with the `ai-writing-rules:anti-ai-writing` skill: plain US English, sentence-case headings, no em dashes, straight quotes. Use `ai-writing-rules:copywrite` for README and other reader-facing copy.
- Keep `engine/PROGRESS.md` current: milestone status, decisions, deviations from the spec and why.
- Do not add features outside the spec. If something seems missing, note it in `PROGRESS.md` and ask. If a spec decision turns out wrong in practice, stop and tell the user. Never swap a stack choice quietly.
- At the end of each milestone, run the tests and tick the acceptance boxes it covers. Do not move on with a failing box.

## Engine architecture (target, see build spec sections 3 to 6)

Three crates in a Cargo workspace rooted at `engine/`:

- `crates/core` (`s-md-slides`): scanner, source map, model, ops, writer, theme, timeline, diagnostics, HTML export, browser-SVG export and the embedded presentation runtime. Must stay WASM-safe: no `std::fs`, network, threads, fonts or rasterizer. Run `scripts/wasm-size.sh` after adding any dependency here (fails above 1.5 MB, target under 1 MB).
- `crates/render` (`s-md-slides-render`, native only): parley layout to flattened SVG, resvg PNG/JPEG, gif, krilla PDF, Mermaid diagrams.
- `crates/cli` (`s-md-slides-cli`, binary `s-md-slides`): `validate`, `export`, `serve`, `apply`.

The keystone is the source map. A deck keeps its original text plus a byte span for every front matter key, directive line, attribute token and body. Ops edit the model and mark spans dirty. The writer copies the original text and splices only dirty spans. Rules that follow from this:

1. Never print the file from the model. An unchanged save is byte-identical, and one move changes one line.
2. Unknown front matter keys and unknown attributes round-trip in place, as do unknown directive kinds.
3. The engine never touches files. Hosts pass text in and out, and assets come through `AssetResolver`.
4. Never panic on input. Malformed decks give a best-effort model plus diagnostics with line and column.
5. Every edit is an `Op`: validate first, change nothing on error, return the inverse.
6. One timeline: the HTML export, the GIF export and every host read `state_at(t)` from the core.
7. RTL changes text direction and presenter controls only. Positions are never mirrored.

Pin every dependency to an exact version (`=x.y.z`).

## Commands

Run from `engine/`:

```sh
cargo build --workspace
cargo test --workspace
cargo test -p s-md-slides <test_name>      # one test
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
scripts/wasm-size.sh                       # core WASM size gate
```

Spikes from M0 live in `engine/spikes/` until they are folded into the crates or deleted.
