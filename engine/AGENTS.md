# AGENTS.md

Guide for coding agents working on the s-md-slides engine. To write or edit decks rather than the engine, read `skills/s-md-slides/SKILL.md`; the format itself is in `docs/format.md`.

## What this is

A Rust workspace with three published crates:

- `crates/core` (`s-md-slides`): parses decks, edits them through ops, saves by splicing bytes, and exports HTML, browser SVG and the presentation. It must build for `wasm32-unknown-unknown`: no file system, network, threads, fonts or rasterizer.
- `crates/render` (`s-md-slides-render`): native PNG, JPEG, GIF and PDF, with parley text layout and bundled Noto fonts.
- `crates/cli` (`s-md-slides-cli`, binary `s-md-slides`): `validate`, `export`, `serve` and `apply`.

`crates/wasm-probe` exists only to measure the core's WASM size. `runtime/` is the TypeScript presentation runtime; its build is committed into `crates/core/src/runtime/`.

## Rules the code depends on

1. Never write a deck by printing the model. Every edit is one byte range spliced into the original text, so an unchanged deck saves byte for byte and one move changes one line.
2. Every edit is an `Op`. It validates first, changes nothing on error and returns its inverse (a `Replace` of the exact bytes).
3. Parsing never panics and never fails. Problems are diagnostics with a code, a line and a column.
4. Anything unknown (front matter keys, attributes, directive kinds with an id) stays where it is.
5. The engine never reads files. Hosts pass text in and out and hand asset bytes over through `AssetResolver`.
6. Positions are never mirrored for right-to-left decks.
7. Anything printed into HTML or SVG goes through the escapers in `crates/core/src/export/sanitize.rs`; raw HTML in decks goes through the allowlist sanitizer.

## Commands

Run from this folder:

```sh
cargo test --workspace --release                          # everything
cargo +1.95.0 clippy --workspace --all-targets -- -D warnings   # lint at the MSRV, as CI does
cargo fmt --all
scripts/wasm-size.sh                                      # core WASM size, fails above 1.5 MB
cd runtime && npm ci && npm run build                     # rebuild the presentation runtime
node runtime/test/check.mjs                               # browser checks in Chrome (BROWSER=firefox too)
cargo +nightly fuzz run fuzz_parse                        # or fuzz_ops, from fuzz/
```

Regenerate expected outputs only after a deliberate change, then review the diff:

```sh
INSTA_UPDATE=always cargo test -p s-md-slides --test golden    # model snapshots
UPDATE_CONFORMANCE=1 cargo test -p s-md-slides --test conformance
UPDATE_REFERENCE=1 cargo test -p s-md-slides-render --release  # reference PNGs
cargo test -p s-md-slides --features ts --lib                   # TypeScript types in bindings/
```

## Where to look

- Parsing and the model: `crates/core/src/deck.rs`, `attrs.rs`, `frontmatter.rs`, `kinds.rs`.
- Ops and the writer: `crates/core/src/ops.rs`.
- Themes, styles, masters: `crates/core/src/theme.rs`. Timeline: `timeline.rs`.
- Tests: golden decks in `tests/decks/`, conformance vectors in `conformance/`, acceptance tests in each crate's `tests/`.
- Decisions and the reasons behind them: `PROGRESS.md`. The spec: `spec/06-build-spec.md`.

## Writing

Docs and comments, like every message the engine prints, are plain US English with sentence-case headings, no em dashes and straight quotes. Diagnostic messages are one sentence that says what to do.
