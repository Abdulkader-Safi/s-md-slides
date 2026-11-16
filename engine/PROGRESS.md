# Progress

Build spec: [spec/06-build-spec.md](spec/06-build-spec.md). Task board: `../s-md-slides-tasks/`.

## Milestones

| Milestone                     | Status              |
| ----------------------------- | ------------------- |
| M0 de-risk                    | done (PR #1)        |
| M1 format core                | done (PR #2)        |
| M2 operations                 | done (PR #3)        |
| M3 elements and style         | in progress         |
| M4 animation and HTML export  | not started         |
| M5 native render              | not started         |
| M6 CLI                        | not started         |
| M7 conformance, docs, release | not started         |

## Decisions

- 2026-11-15: the Cargo workspace lives in `engine/`, not at the git root. The git repo also holds the task board, and the spec puts the GPUI app, npm package and Obsidian plugin in their own repos later.
- 2026-11-15: MSRV is 1.95 (latest stable 1.97.1 minus two).
- 2026-11-15: one branch and one PR per milestone. Commits after each task.
- 2026-11-16: keep serde-saphyr (about 400 KB of WASM) and nested inline themes. Spec 04 set a 150 KB limit for it; the build spec only replaces it above 1.5 MB total, and the core is 677 KB. The flat front matter reader stays the fallback if a later milestone trips the size gate. Decided by the user.
- 2026-11-15: approved after review. Native PDF export draws text by passing parley glyph runs straight to krilla (`draw_glyphs` with parley's cluster ranges, ligature continuations merged, glyph y offsets negated). krilla-svg draws everything that is not text. PNG export keeps the flattened SVG through resvg.

## M3 results

Code: `crates/core/src/kinds.rs` (attributes and bodies per kind), `crates/core/src/theme.rs` (themes, `resolved_style`, backgrounds, masters), `themes/default.json`.

Decisions:

- Element bodies are read with pulldown-cmark, which the spec already picks for export. The core's WASM build went from 677 KB to 852 KB. M4 adds the export code and the presentation runtime, so the 1 MB target is getting close; if it trips, the first fallback is the flat front matter reader (about 400 KB).
- Every element gets `style` (style attributes as written) and `content` (typed kind data: image path, alt text and fit; code language and source plus highlighted lines; table header flag; Mermaid or math source; video and embed fields; group items).
- Colors are `#rgb`, `#rrggbb`, `#rrggbbaa` or a theme role, as the spec lists. Named CSS colors such as `red` give `E-ATTR-VALUE`. `fill` and `bg` also take `linear(angle,color,color[,color])`.
- A theme file named in front matter is read only when the host calls `load_theme(&resolver)`, since the engine never touches files. Until then the default theme applies. A loaded theme survives later edits as long as the front matter still names the same file.
- `AssetResolver` and `Asset` live in the core now (M3 needs them for themes) rather than waiting for M4's export module.
- Which master a slide gets: its own `master=`, else its layout's master, else the first master in the deck. `master=none` gives none.
- `resolved_style` defaults: body font and `text` color, 32 px (24 px for code and tables), weight 400, line height 1.3, start alignment, top. Code uses the code font and colors on `code-bg` with 24 px padding. Rect and ellipse fill with `accent1`; lines and arrows stroke with `text` at 2 px.
- `AddSlide` from a layout copies the placeholder's style attributes onto the new element, which is how layout styling reaches slides.

## M2 results

Acceptance boxes ticked: FR-OPS-01 to FR-OPS-05 and FR-EL-10. FR-OPS-01's "same result in the npm build" half is proven by the conformance vectors in M7; the npm package lives in its own repo. Exit criteria: the proptest runs 10,000 decks with random sequences of every op type (about 35,000 successful ops), each op stays inside its target's bytes, and undo restores the exact file. `fuzz_ops` passed 1,000,000 runs in 21 minutes with no panic and every inverse exact.

Code: `crates/core/src/ops.rs` (every op from the spec), `crates/core/src/query.rs`, TypeScript types in `bindings/`.

Decisions:

- Every op still plans one contiguous byte range. `Group`, and `SetGeometry` on a group, expand into a batch of simpler ops (write missing member ids, add the group line; move each member), so they get one combined `Batch` inverse.
- `MoveSlide` and `SetZ` rewrite the span between the old and new position with the same blocks in a new order. Untouched bytes stay identical, so the diff shows only the moved block.
- `SetAttrs` refuses to drop `x`, `y`, `w` or `h` from a known kind (`E-ATTR-MISSING`), refuses to change or remove `id` (`E-ATTR-ID`), and checks values of keys the engine reads (`x`, `w`, `opacity`, `dir` and so on). Ops can't write a deck that fails `validate`.
- `SetFrontMatter` replaces the key's line plus any indented or list lines under it. String values stay plain when YAML reads them back unchanged, otherwise they are written as JSON strings (valid YAML), for example `canvas: "4:5"`.
- `AddSlide` with a layout writes one element per placeholder at the placeholder's geometry (FR-STYLE-04's op half).
- `AddDefinition` appends to the last definitions section, or starts one before the first slide. `RemoveDefinition` refuses while a slide or layout uses the id (`E-IN-USE`).
- `SetNotes` refuses on slides with two notes blocks (`E-UNSUPPORTED`), rather than guessing how to merge them.
- New public queries `element_range(id)` and `slide_range(i)` give the byte range of an element or slide in `text()`, so hosts can jump from a box to its source. The proptest uses them to check that ops stay inside their target.
- TypeScript types come from ts-rs behind a `ts` feature, so ts-rs never reaches the WASM build. They are committed in `engine/bindings/` and CI fails if they go stale.

## M1 results

Acceptance boxes ticked in the build spec: FR-FMT-01 to FR-FMT-08, FR-CANVAS-01, FR-CANVAS-02, FR-DIR-01, FR-PKG-02. FR-NOTES-01 stays open until M4: notes parse and round-trip now, but its check also needs the HTML presenter view. Exit criteria: all 15 golden decks round-trip, `fuzz_parse` passed 1,000,000 cases, the WASM gate passes at 572 KB, and CI is green on macOS, Windows and Linux.

Code: `crates/core` (crate `s-md-slides`), probe for the size gate: `crates/wasm-probe`. The M0 writer spike is folded in and `spikes/core` and `spikes/wasm` are gone.

Decisions:

- Undo is one low-level op, `Replace { at, old, new }`. It checks that the bytes at `at` still equal `old` before swapping, so undo is byte-exact by construction and fails with `E-STALE` instead of corrupting the file when applied out of order. This replaces the spike's `SetDirective` and `InsertRaw`. Offsets are UTF-8 byte offsets; a TypeScript port must count bytes the same way. M2 can still return semantic inverses on top if hosts want them.
- Every op is planned as one contiguous byte range plus its replacement, so an op only ever touches its own element's bytes.
- Elements without `id=` get an in-memory id (kind letter plus counter, skipping used ids). Across edits the id follows the element: ids are keyed by where the element starts, and those offsets shift past each splice. Editing such an element writes its id. A duplicate id gets a fresh in-memory id plus `E-DUP-ID`.
- Implicit text (markdown before a slide's first directive) fills a safe area of 5% margins: x=96 y=54 w=1728 h=972 on 16:9. The spec names a safe area without numbers. The first edit writes a full `<!-- text ... -->` line above it.
- Trailing blank lines after a body belong to the gap between elements, not the body, so `SetBody` keeps the spacing and `RemoveElement` takes the gap with it.
- An empty file, or one with only front matter, has no slides. A trailing `---` still makes an empty last slide.
- Over a limit (20 MB, 5,000 slides, 2,000 elements on a slide), the deck opens read-only with `E-LIMIT`. Ops refuse to run, and the text is untouched.
- Columns in diagnostics count characters, not bytes. YAML errors are mapped through byte offsets because YAML counts a lone CR as a line break and the rest of the engine does not (found by the fuzzer).
- A newer format version gives `E-VERSION` with error severity, so `validate` exits 1 for decks this engine can't edit.
- Directives in the wrong place (a `placeholder` outside a layout, a second `<!-- slide -->`) give `W-MISPLACED` and are read as text, so nothing goes missing from the model.

For M5: parley's default `system` feature pulls in fontique's fontconfig backend, which needs the fontconfig library at build time on Linux. CI excludes `spike-layout` from clippy and tests until then. The render crate needs either fontconfig installed in CI or fontique's dlopen option.

Open question from M0, settled: removing the first element of a file whose next line is `---` is still refused (`E-STRUCTURE`), because the file would start with `---` and turn into front matter. A deck with front matter is unaffected.

## M0 results

### Layout match and RTL spike (done)

Code: `spikes/layout`. Run `cargo run -p spike-layout --release -- target/spike-layout`, then `python3 spikes/layout/compare.py target/spike-layout`. One 1920 x 1080 slide with an English box, an Arabic box (`dir=rtl`) and a mixed English and Arabic line, all at 40 px with line height 1.3, using bundled Noto Sans and Noto Sans Arabic only (system fonts off on the native side).

Native path: parley 0.13 lays out each box, then every glyph run becomes one SVG `<text>` at its parley position. One usvg tree feeds resvg (PNG) and krilla-svg (PDF).

Results:

- Line breaks match Chrome exactly in all three boxes (4, 3 and 2 lines). Not just the last word: every line is identical.
- Arabic in the PNG is shaped (joined letters) and right to left, punctuation on the correct side, right-aligned in its box. The PNG and Chrome's screenshot look the same apart from macOS font smoothing.
- The PNG renders in 70 to 85 ms (NFR-01 allows 300 ms).
- Arabic in the PDF renders correctly and is selectable. Copy and search of Arabic is viewer-dependent; see below.

Three fixes were needed to get usvg to draw parley runs faithfully, and M5 needs all of them:

1. Place every run by its left edge. Each run has one bidi level, so that is enough.
2. usvg 0.47 runs bidi with a fixed LTR base and ignores `direction` and `unicode-bidi`. RTL runs are wrapped in RLO ... PDF (U+202E, U+202C) so neutrals such as the Arabic comma stay where parley put them.
3. Bidi rule L1 moves edge whitespace to the base level even inside an override, so edge spaces are trimmed and x moves by the width parley gave them.

Also: krilla-svg 0.8.1 depends on usvg 0.47, so resvg and usvg are pinned to 0.47.0 (latest is 0.50) to keep one SVG tree for PNG and PDF.

PDF text layer (copy and search). Noto Sans Arabic builds letters from shared dotless bases plus dot marks, so one glyph stands for several letters. krilla handles that with ActualText spans, which only some viewers read. Tested two ways of putting text in the PDF:

| Viewer engine | SVG path (krilla-svg) | Direct path (parley glyphs into krilla) |
| --- | --- | --- |
| English, all viewers | Copy and search work | Copy and search work |
| PDFKit (macOS Preview) | Wrong letters, RLO/PDF characters leak into the text | Wrong letters, no control characters |
| poppler (pdftotext, many Linux viewers) | Letters split apart, control characters leak | Every letter correct, logical order, extra spaces between some letter groups |
| pdfium (Chrome) | Lines broken up, wrong letters | Every letter correct, some words reversed |

Recommendation for M5: draw text with the direct path (parley glyph runs into krilla with parley's cluster ranges, ligature continuations merged) and keep krilla-svg for everything that is not text. It renders identically, puts parley's exact glyphs in the PDF instead of a second shaping pass, and has the best text layer in every viewer tested. This keeps krilla and krilla-svg, but changes how text reaches krilla, so it needs a yes before M5. Perfect Arabic search in every viewer is likely out of reach with this font; the rendering itself is correct everywhere.

### Pinned versions (done)

Every dependency the spec names is pinned with `=x.y.z` in `[workspace.dependencies]` of `Cargo.toml`, checked on crates.io on 2026-11-15, and they all resolve together. The highest MSRV among them is krilla's 1.92, below ours (1.95).

Where the pin differs from the version family in the spec:

| Crate | Spec | Pinned | Why |
| --- | --- | --- | --- |
| serde-saphyr | 1.3.x | 2.1.1 | Latest stable. The WASM spike measured this version |
| parley, fontique | 0.9.x | 0.13.0 | Latest stable. The layout spike used this version |
| resvg, usvg | latest | 0.47.0 | krilla-svg 0.8.1 is built on usvg 0.47; one SVG tree for PNG and PDF |
| notify, notify-debouncer-full | 8.2, 0.7 | 8.2.2, 0.7.0 | As the spec says. 9.0 and 0.8 are release candidates |
| giallo | Could | not pinned | EUPL-1.2, syntect 5.3.0 is pinned instead |

parley 0.13 and krilla 0.8 depend on different versions of the font stack (skrifa 0.49 and 0.42, read-fonts, font-types). That only adds native binary size, and the core crate is not affected.

### License check (done)

| Item | License | Verdict |
| --- | --- | --- |
| giallo 0.6.0 | EUPL-1.2 | Not used. EUPL is copyleft and would bind the MIT binaries. FR-EL-11 uses the spec's fallback, syntect |
| syntect 5.3.0 | MIT | OK |
| syntect bundled grammars (sublimehq/Packages) | Permissive custom notice (copy, use, modify, sell, distribute), with per-file exceptions marked by a `-license` file | OK. Recheck the exceptions if FR-EL-11 is built in M5 |
| Noto Sans, Noto Sans Arabic | OFL-1.1 | OK, ship OFL.txt next to the fonts |
| KaTeX fonts (ratex-katex-fonts, via ratex-svg) | OFL-1.1 | OK, same as above |
| mermaid-rs-renderer 0.3.1, ratex-svg 0.1.15 | MIT | OK |
| tiny-skia 0.12.0 | BSD-3-Clause | OK |
| jpeg-encoder 0.7.1 | (MIT OR Apache-2.0) AND IJG | OK, IJG needs an attribution line in the notices |
| notify 8.2.2 | CC0-1.0 | OK |

No GPL or AGPL anywhere. cargo-deny in M7 needs EUPL-1.2 on its deny list so giallo can't slip back in.

### WASM size spike (done)

Code: `spikes/wasm`, gate: `scripts/wasm-size.sh <package>`. The spike wraps the writer spike, serde-saphyr (front matter into a struct with flattened unknown keys), pulldown-cmark HTML with tables, strikethrough, tasklists and math, serde_json, and a 40 KB placeholder for the presentation runtime (the real one is not written yet). Built with the workspace release profile (`opt-level = "z"`, LTO, one codegen unit, `panic = "abort"`), wasm-bindgen 0.2.130, then `wasm-opt -Oz` from binaryen.

| Build                                    | Size   | gzip   |
| ---------------------------------------- | ------ | ------ |
| Full spike                               | 745 KB | 295 KB |
| Without serde-saphyr                     | 343 KB | 135 KB |
| Without the pulldown-cmark HTML renderer | 735 KB | 290 KB |

Result: under 1 MB, so M0 passes. serde-saphyr is about 400 KB of it, most likely from its `deserialize` feature pulling in annotate-snippets and encoding_rs. The full core adds ops, themes, the timeline, export code plus the sanitizer on top, with about 280 KB of headroom. If the gate trips in a later milestone, the spec's fallback (a flat front matter scanner instead of serde-saphyr) wins back roughly 400 KB.

`strip = true` removed the `target_features` section wasm-bindgen reads, so the release profile uses `strip = "debuginfo"`.

### Lossless writer spike (done)

Code: `spikes/core`. Scanner, splice-only writer, `SetGeometry`, `SetBody`, `RemoveElement`, plus `Batch` and two internal ops used as inverses (`SetDirective` restores a directive line as written, `InsertRaw` puts a removed element back).

Result: proptest ran 1,000 generated decks with 1 to 7 random ops each, about 3,400 successful ops per run. Every case undid back to the original bytes. Zero differences. Every move changed at most one line. Failed ops left the deck unchanged. The generator covers BOM, LF, CRLF and mixed endings, front matter with unknown keys, unknown attributes, quoted values with escapes, unknown directive kinds with ids, ordinary comments, fences holding `---` and directive-looking lines, Arabic text and files with no final newline.

How it writes: each op computes one byte range and its replacement, splices it into the source, re-scans, and checks the structure did not shift (same element count per slide, body exactly as asked). A failed check refuses the op. Re-scanning is O(file size) per op. This is the lazy version of the spec's "dirty spans plus offset shifting"; M1 benchmarks decide if it stays (NFR-01: one op plus save under 5 ms).

Bug found and fixed: a directive on the last line with no line ending got its new body glued onto the same line.

Open questions for M1:

- Removing the first element of the file when the next line is `---` is refused, because the file would then start with `---` and read as front matter. A deck can't have an empty first slide followed by a separator unless it has front matter. Options: always write front matter when this happens, or accept the refusal.
- Undo of an attribute edit restores the whole directive line as written (`SetDirective`). The spec's op list has no such op. M2 either keeps it as an internal op in the JSON schema or makes `SetAttrs` carry raw values.
- In-memory ids for elements without `id=` are assigned by file order on every scan, so removing an element with an explicit id can shift the ids of id-less elements. M1 needs a stable mapping (FR-FMT-07).
