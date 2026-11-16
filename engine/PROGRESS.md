# Progress

Build spec: [spec/06-build-spec.md](spec/06-build-spec.md). Task board: `../s-md-slides-tasks/`.

## Milestones

| Milestone                     | Status              |
| ----------------------------- | ------------------- |
| M0 de-risk                    | done (PR #1)        |
| M1 format core                | done (PR #2)        |
| M2 operations                 | done (PR #3)        |
| M3 elements and style         | done (PR #4)        |
| M4 animation and HTML export  | done (PR #5)        |
| M5 native render              | done (PR #6)        |
| M6 CLI                        | done (PR #7)        |
| M7 conformance, docs, release | not started         |

## Decisions

- 2026-11-15: the Cargo workspace lives in `engine/`, not at the git root. The git repo also holds the task board, and the spec puts the GPUI app, npm package and Obsidian plugin in their own repos later.
- 2026-11-15: MSRV is 1.95 (latest stable 1.97.1 minus two).
- 2026-11-15: one branch and one PR per milestone. Commits after each task.
- 2026-11-16: keep serde-saphyr (about 400 KB of WASM) and nested inline themes. Spec 04 set a 150 KB limit for it; the build spec only replaces it above 1.5 MB total, and the core is 677 KB. The flat front matter reader stays the fallback if a later milestone trips the size gate. Decided by the user.
- 2026-11-15: approved after review. Native PDF export draws text by passing parley glyph runs straight to krilla (`draw_glyphs` with parley's cluster ranges, ligature continuations merged, glyph y offsets negated). krilla-svg draws everything that is not text. PNG export keeps the flattened SVG through resvg.

## M6 results

Code: `crates/cli` (crate `s-md-slides-cli`, binary `s-md-slides`): `validate`, `export`, `serve`, `apply`, and the deck-folder resolver.

Acceptance boxes ticked: FR-CLI-01 to FR-CLI-04. Exit criteria: a Vim-style save (write a new file, rename it over the deck) reached a connected browser in 133 ms, and the page reloaded on the same slide with the new text (checked in headless Chrome; the test suite checks the event arrives within 500 ms). The deck's modified time never changes because of `serve`. `/../secret` and percent-encoded `..` return 404; other methods return 405.

Decisions:

- `serve` keeps live-reload connections open and writes `event: reload` to each when the deck, one of its assets or its theme file changes (100 ms debounce, recursive watch on the deck's folder, other files ignored). The reload script is part of the runtime build with its own SHA-256 in the CSP, added through the new export option `live_reload`.
- The watcher reacts only to creates, writes, renames and removes. Linux also reports reads, and every rebuild reads the deck, so the first version reloaded the browser forever on Linux (caught by CI). The serve test now fails if one save causes more than one reload.
- Errors in the deck show as an overlay above the slides; the presentation still renders what parses.
- File serving: GET only, files under the deck's folder after resolving symlinks, no `..` segments, no directory listing. A host other than 127.0.0.1, localhost or ::1 prints a network warning.
- `export` writes next to the deck by default (`deck.pdf`, `deck-3.png`); `-o` names a file, or a folder when several images are written. `--to png|jpeg|gif` without `--slide` exports every slide. It exits 1 if the deck has errors, after writing.
- `apply` accepts one op or a list, prints the inverse batch on stdout, and writes through a temp file plus rename, so a crash never leaves half a deck.
- Ctrl-C during `export` stops at the next page or frame, writes nothing half-done, and exits 4. A second Ctrl-C quits at once. Uses the `ctrlc` crate (3.5.2, MIT or Apache-2.0), added with the user's approval on 2026-11-16; only `export` installs the handler, so Ctrl-C still stops `serve` normally.

## M5 results

Code: `crates/render` (crate `s-md-slides-render`): `fonts.rs`, `layout.rs` (markdown to parley glyph runs), `flatten.rs` (scene and SVG), `raster.rs`, `pdf.rs`, `gif.rs`, `graphics.rs` (Mermaid and math). Example: `cargo run -p s-md-slides-render --release --example render -- deck.md out-dir`. Reference PNGs: `engine/tests/reference/`. The M0 spikes are gone.

Acceptance boxes ticked: FR-EXP-03 to FR-EXP-06, FR-EL-06 to FR-EL-09. FR-EL-11 (syntax highlighting, a Could) is not built and its card is in the backlog. Exit criteria: every example deck exports to every format (PNG, JPEG, SVG, PDF, GIF); the 11 reference PNGs match within 0.5% on macOS, Windows and Linux in CI; PDF text is selectable (12 pages, ToUnicode maps; `pdftotext` reads Latin and Arabic); a three-step slide's GIF shows each step in order; cancel returns `E-CANCELLED` within 200 ms.

Decisions:

- Raster exports (PNG, JPEG, GIF) draw text as glyph outlines (SVG paths from skrifa) at exactly parley's positions, instead of `<text>` that usvg would shape again. That removes the RLO and whitespace workarounds from the M0 spike and makes the PNG match parley exactly. The PDF keeps the approved path: parley glyphs straight into krilla as real text, krilla-svg for everything else, element by element so stacking order holds.
- `flatten_svg` therefore outputs outlines rather than positioned `<text>` runs. Anything that needs selectable text uses the PDF or the browser SVG from the core.
- GIFs pause 600 ms (`GIF_HOLD_MS`) after each step and sample by step with the new core method `Timeline::state_at_step`. Without that, steps made of `appear` (zero duration) all show in the first frame, since `state_at(t)` can't tell zero-length steps apart.
- Mermaid (mermaid-rs-renderer) and math (ratex, KaTeX fonts embedded) run behind `catch_unwind`; a failure shows the source in a code box with `W-DIAGRAM`.
- Native export never requests URLs, and the SVG loader only accepts `data:` images (usvg's default reads local files for other hrefs, and Mermaid and math SVG come from deck text). Colors are escaped in the flattened SVG as a second guard. Both from a security review.
- Native export never requests URLs: only relative paths go to the resolver, so embeds and remote images become gray boxes or posters.
- Bundled fonts: Noto Sans, Noto Sans Arabic and Noto Sans Mono, each in regular and bold (OFL-1.1). Italic is synthesized by parley and drawn as a skew. `Fonts::with_system()` adds installed fonts; tests use `Fonts::bundled()` so output is identical everywhere.
- Linux builds of the render crate need the fontconfig library (`libfontconfig1-dev`) for parley's system-font support. CI installs it.
- Measured on an M-series Mac (release): a 1920x1080 slide renders in about 50 to 240 ms once fonts are warm; the first render of a process takes about 600 ms. The NFR-01 benchmark in M7 measures this properly.
- Not yet drawn natively: underline and strikethrough decorations, images inside text bodies, raw HTML (dropped).

## M4 results

Acceptance boxes ticked: FR-ANIM-01 to FR-ANIM-04, FR-DIR-02, FR-EXP-01, FR-EXP-02, FR-EXP-07, FR-ASSET-01 to FR-ASSET-03, plus the export halves of FR-NOTES-01, FR-EL-01, FR-EL-03 to FR-EL-05, FR-STYLE-01 and FR-STYLE-03 to FR-STYLE-05. Still open until M5: FR-EL-06 to FR-EL-09, whose checks need native rendering or PNG posters. Exit criteria: both example decks present fully from one offline HTML file in every browser the spec names; keyboard-only use works; axe-core finds no WCAG 2.2 AA violations on the controls.

Code: `crates/core/src/timeline.rs`, `crates/core/src/export/` (sanitizer, slide HTML, browser SVG, presentation), the runtime in `runtime/` (TypeScript, built into `crates/core/src/runtime/`), example decks in `examples/`, and the export example `cargo run -p s-md-slides --example export -- deck.md out.html --embed`.

Decisions:

- Each timeline entry carries its from and to state. Rust's `state_at` and the runtime both blend them with ease-out cubic, frame by frame. Spec 04 says the runtime emits CSS animations; one shared formula in data form is a stronger match for FR-ANIM-04 (the HTML export and the GIF play the same numbers), and the runtime stays at 7 KB.
- Steps: elements without `order` animate when the slide appears (step 0). Each distinct `order` is one click. An element with both `enter` and `exit` exits on the click after its entrance; one with only `exit` exits at its `order`. The spec doesn't say how `exit` is timed.
- Fly effects start or end just off the canvas edge (fly-left from x = -w, and so on). `scale` grows from half size while fading in.
- A slide's transition length is its `dur=` attribute (default 400 ms). The deck default transition is front matter `transition:`, else fade.
- Headings inside a text box use the box's `size`, set in the heading font, instead of the browser's 2em for h1. A box with `size=72` shows its `# Title` at 72 px, as the spec's examples expect.
- Mermaid and math show their source in a labeled box in HTML and browser SVG. Drawing them needs the native renderer (M5) or the host (Obsidian renders them itself, per spec).
- Asset URLs and embed `src` are scheme-checked at export: `javascript:` and other unsafe schemes become the gray placeholder, and embeds must be https. YouTube links are rewritten to youtube-nocookie.
- Security review fixes (2026-11-16): the sanitizer escapes `<` in every text it echoes and treats an unquoted `<` inside a would-be tag as text; every style attribute is attribute-escaped; `resolved_style` replaces values that failed validation with defaults; font names are limited to letters, digits, spaces and `-_.`. `fuzz_parse` now renders every fuzzed deck to HTML and SVG and fails on any `<script` in slide markup: 1,000,000 runs, none found.
- The CSP pins the runtime by its SHA-256 (`crates/core/src/runtime/runtime.sha256`, written by the build). CI rebuilds the runtime and fails if the committed copy differs.
- Presentation controls stay fully opaque. They faded to 35% when idle at first, which failed axe's color contrast check on dark slides.
- WASM: the size probe now calls every export, so the linker keeps them. The core measures 953 KB with the runtime, under the 1 MB target.
- The browser check (`node runtime/test/check.mjs`) exports both example decks with embedded assets into a temp folder with no assets beside them, then checks the clicks, effects, transitions, rtl keys, reduced motion, presenter view, CSP errors and axe-core in headless Chrome and Firefox (`BROWSER=firefox FIREFOX_PATH=...`). Safari runs the same checks through `safaridriver` with `node runtime/test/safari.mjs` (needs Develop > Allow Remote Automation, so it isn't in CI); Safari 26.6 passed all 21 on 2026-11-16. It skips reduced motion and console errors, which WebDriver can't reach in Safari.

## M3 results

Acceptance boxes ticked: FR-EL-02 and FR-STYLE-02. The other M3 requirements (FR-EL-01, 03 to 09, FR-STYLE-01, 03 to 05) have their model half done and tested in `crates/core/tests/style.rs`, but each check also names an export ("the HTML export renders them", "exports to SVG", "3 / 12 on slide 3"), so they get ticked in M4 and M5. Exit criteria: 25 golden decks round-trip, `resolved_style` is snapshotted for every element and reviewed, and `fuzz_parse` passed another 1,000,000 runs with the new body parsing.

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
- Numbers in the model are `f64`, not the spec's `f32`. With `f32`, JSON for hosts showed `line-height: 1.2999999523162842` and `x: 786.239990234375` for values written as `1.3` and `786.24`.

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

Acceptance boxes ticked in the build spec: FR-FMT-01 to FR-FMT-08, FR-CANVAS-01, FR-CANVAS-02, FR-DIR-01, FR-PKG-02. FR-NOTES-01 stays open until M4: notes parse and round-trip now, but its check also needs the HTML presenter view. Exit criteria: all 15 golden decks round-trip, `fuzz_parse` passed 1,000,000 cases, the WASM gate passes at 572 KB, and CI is green on all three operating systems.

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
