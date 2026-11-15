# Tech decisions

Status: signed off 2026-11-15

For each decision: what was chosen, what else was considered, and why, traced to a requirement (Stage 3) or the mission and users (Stages 1 and 2). Versions checked 2026-11-15. Exact pins are set in M0 from crates.io at build time.

> **Constraints going in:** Rust only, one codebase (Stage 1). npm build is WebAssembly and the `.wasm` must be under 1 MB, hard ceiling 1.5 MB (NFR-02). Must run in Obsidian mobile webviews, so no threads, no file system and no network in the WASM build (FR-PKG-02, NFR-04). MIT, no GPL or AGPL (NFR-09). One developer with Claude Code. $0 running cost. The desktop app (separate spec) reuses the mdk spec's GPUI choices, so nothing here may depend on GPUI.
> **Changes:** 2026-11-15 (after sign-off): split into four repos (Rust engine, GPUI app, npm package, Obsidian plugin). The npm package is its own project: a WASM wrapper of the core if the package stays under 4 MB, otherwise a TypeScript port, proven by the engine's conformance vectors. Code syntax highlighting dropped to Could.

## Overall architecture
- **Choice:** four repos (project order): the engine (this spec), the desktop app, the npm package and the Obsidian plugin. The engine repo is a Cargo workspace with three published crates. The small core goes everywhere. Anything heavy is native only.
  - `s-md-slides` (core, WASM-safe): deck model, parser, lossless writer, operations, animation timeline, diagnostics, HTML export, the HTML presentation, and browser SVG export.
  - `s-md-slides-render` (native only): text layout, flattened SVG, PNG, JPEG, GIF and PDF export, Mermaid, math, code highlighting, bundled fonts.
  - `s-md-slides-cli` (native binary `s-md-slides`): `validate`, `export`, `serve`, `apply`.
  - Not in this repo: the wasm-bindgen bindings and the npm package. They live in the npm repo, which depends on the `s-md-slides` crate from crates.io and proves itself against the `conformance/` vectors (FR-PKG-03).
- **Considered:**
  - One crate with Cargo features: possible, but a feature left on by mistake pulls fonts and the rasterizer into the WASM build and breaks NFR-02 silently. Separate crates make that impossible.
  - Rendering everything in the core, WASM included: text layout, a rasterizer and fonts would cost several MB of WASM on their own (the SVG text feature alone adds about 400 KiB, per svg2pdf's feature docs on lib.rs).
- **Why:** this split keeps NFR-02 and still gives native builds every export format (FR-EXP-03 to 05). Hosts that use WASM (Obsidian) get HTML and browser SVG from the core, and make images and PDF with the browser (Stage 1).

## Deck parser and lossless writer (the keystone)
- **Choice:** our own line-based scanner in the core. It finds front matter, `---` separators and `<!-- directive -->` lines, and records each element's byte span in the original text. Element bodies stay as raw markdown strings and are not re-parsed for writing. The writer copies the original text and replaces only the spans of changed directive lines or bodies.
- **Considered:**
  - Parse the whole file with a markdown AST (comrak, pulldown-cmark) and print it back: printers normalize spacing and list markers, which breaks FR-FMT-02.
  - A full concrete syntax tree like rowan: more than we need, since only directive lines change structurally.
- **Why:** byte-identical round-trip (FR-FMT-02, NFR-03), one-line diffs (FR-FMT-03), unknown attributes kept in place (FR-FMT-04), and line and column for every diagnostic (FR-FMT-06). A small hand-written scanner is also the cheapest code to fuzz (NFR-05).

## Markdown inside elements
- **Choice:** `pulldown-cmark` 0.13.x (MIT, latest 0.13.4 in May 2026), with tables, strikethrough, task lists and math options on. Used only to turn element bodies into HTML for export, never for writing.
- **Considered:** comrak (heavier, more features than needed). markdown-rs (used by gpui-kit's TextView, but the engine must not depend on the app's choices).
- **Why:** small, fast, `no_std`-capable, and its event stream lets us filter raw HTML ourselves (FR-EXP-07).

## HTML sanitizing
- **Choice:** our own allowlist filter on pulldown-cmark's raw HTML events: keep a short list of inline and block tags with safe attributes, drop `<script>`, `on*` attributes and `javascript:` URLs, and emit a warning for each removal.
- **Considered:** ammonia: thorough, but it pulls in html5ever, which is too large for the WASM budget.
- **Why:** FR-EXP-07 within NFR-02. Fuzzed with the parser.

## Front matter
- **Choice:** `serde-saphyr` 1.3.x (MIT or Apache-2.0, YAML 1.2, panic-free), the same crate mdk chose. Reading only. The writer changes front matter by replacing single key lines, just like directives.
- **Considered:** our own flat `key: value` reader: smaller, but an inline theme needs nested YAML (FR-STYLE-01).
- **Landmines:** its WASM size is unmeasured. M0 measures it. If it costs more than 150 KB, inline themes get limited to flat dotted keys (`theme.accent1: "#e11"`) read by our own scanner.

## Edit operations and the API boundary
- **Choice:** operations are one Rust enum with serde derives. `apply` validates, changes the model and the writer spans, and returns the inverse (FR-OPS-01 to 04). Across the WASM boundary, operations, the model and diagnostics travel as JSON strings through `serde_json`. TypeScript types are generated from the Rust types with `ts-rs` (MIT) at build time and shipped in the npm package.
- **Considered:** `serde-wasm-bindgen` to pass JS objects directly: slightly faster, but JSON is needed anyway for MCP, `apply ops.json` and the agent skill, and it's easier to debug. `tsify`: similar to ts-rs. ts-rs is more widely used.
- **Why:** one wire format for hosts, the CLI, the app's MCP server and agents (Stage 2). JSON speed is well inside NFR-01 at deck sizes.

## WebAssembly build
This section is the plan the npm repo follows. The engine repo only builds the core to WASM in CI for the size gate (FR-PKG-02).
- **Choice:** `cargo build --target wasm32-unknown-unknown --release`, then `wasm-bindgen-cli` (pinned to the same version as the `wasm-bindgen` crate, 0.2.12x as of June 2026) with `--target web`, then binaryen's `wasm-opt -Oz`. Release profile: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`. A small hand-written `package.json` and an `init()` wrapper.
- **Considered:** `wasm-pack`. The rustwasm working group and its tools were sunset and archived in July 2025, and wasm-pack is now maintained by one person (drager) under the wasm-bindgen org. It still works, but it hides the steps that control size.
- **Why:** explicit steps make the size budget visible and enforceable in CI (NFR-02). `--target web` loads in browsers, Electron and mobile webviews with no bundler (FR-PKG-02).
- **Landmines:** a version mismatch between the `wasm-bindgen` crate and the CLI fails the build. Pin both to the same exact version in the same commit.

## Animation timeline
- **Choice:** pure Rust in the core: each slide's build steps, delays and durations become a timeline that answers `state_at(t)` (opacity, offset, scale per element). The HTML presentation runtime uses the same numbers, emitted as CSS animations. GIF export samples `state_at` per frame.
- **Why:** one source of truth for every host and the GIF (FR-ANIM-04).

## HTML presentation runtime
- **Choice:** a small vanilla TypeScript runtime (target under 15 KB minified) for controls, keyboard, swipe, presenter view with notes and timer, build steps, transitions, RTL mirroring and reduced motion. Compiled with esbuild at repo build time, then embedded in the core with `include_str!`. It goes into every HTML export and into `serve`.
- **Considered:** reveal.js (MIT, used by Advanced Slides): its own slide model fights a free canvas, and it's much larger.
- **Why:** FR-EXP-02, FR-DIR-02, NFR-07, and the same runtime for export and `serve` (FR-CLI-03).

## Browser SVG export (core, WASM)
- **Choice:** SVG per slide with shapes and images as SVG, and text inside `<foreignObject>` as HTML, so the browser does text layout and bidi (FR-EXP-01, FR-DIR-01).
- **Why:** no text layout in WASM, which keeps NFR-02.
- **Landmines:** drawing a `foreignObject` SVG to a canvas for PNG has historically tainted the canvas in Safari, which would block PNG export in Obsidian on iOS. That belongs to the Obsidian plugin spec. If it fails there, mobile exports HTML only.

## Native text layout and flattened SVG (`s-md-slides-render`)
- **Choice:** `parley` 0.9.x (Apache-2.0 or MIT, Linebender) for shaping, line breaking and bidi reordering. Its output becomes positioned `<text>` runs in a flattened SVG that needs no further layout.
- **Considered:** cosmic-text (also good, but Parley is used by Slint and Masonry and has active bidi fixes). Letting usvg wrap text: SVG has no automatic wrapping.
- **Why:** PNG, JPEG, GIF and PDF all start from one flattened SVG, so they match each other (FR-EXP-03 to 05). Parley resolves bidi, which covers RTL and mixed text (FR-DIR-01).
- **Landmines:** browser layout (HTML export) and Parley layout (PNG and PDF) can wrap a line differently. Using the same bundled fonts and the same line-height rules keeps it close. M0 compares both on the sample deck.

## Rasterizing (PNG, JPEG, GIF)
- **Choice:** `resvg` and `tiny-skia` (Apache-2.0 or MIT) render the flattened SVG to pixels. PNG through tiny-skia's encoder, JPEG through `jpeg-encoder` (small, MIT or Apache-2.0), GIF through the `gif` crate with its built-in NeuQuant quantizer (FR-EXP-04, FR-EXP-05).
- **Considered:** the `image` crate: fine, but larger than needed for three encoders.
- **Why:** pure Rust, no system libraries, the same on macOS, Windows and Linux.

## PDF
- **Choice:** `krilla` with `krilla-svg` (MIT or Apache-2.0, by the Typst developers). One page per slide from the flattened SVG, text embedded as real selectable text (FR-EXP-03).
- **Considered:** `svg2pdf`: archived on 2026-07-10, and its README now points to krilla with krilla-svg. printpdf: lower level, no SVG input.
- **Landmines:** krilla is pre-1.0. Pin exactly and upgrade on purpose.

## Mermaid and math (native)
- **Choice:** `mermaid-rs-renderer` 0.3.x (MIT) for diagrams and `ratex-svg` (MIT) for LaTeX, both producing SVG that is placed into the flattened slide. The same crates the mdk spec evaluated. In Obsidian, the plugin uses Obsidian's built-in Mermaid and math renderers, so the WASM build carries neither (FR-EL-06, FR-EL-07).
- **Landmines:** both are young. If mermaid-rs-renderer can't draw a diagram type, the export shows its source in a code box with a warning.

## Code highlighting (native)
Could (FR-EL-11), not in the first release. Code shows in the theme's code font and colors until then.
- **Choice:** `giallo` (TextMate grammars, output identical to VS Code). Its author says it bundles four times more languages in under 2 MB than tree-sitter grammars that easily added 100 MB. A starter set of about 30 languages. In the WASM build, code goes out with a `language-x` class and the host highlights it (Obsidian has its own highlighter) (FR-EL-04).
- **Considered:** `syntect` with pure-Rust fancy-regex: proven and pure Rust, but it uses older Sublime grammars, and fancy-regex roughly halves highlighting speed. Tree-sitter: accurate, but the grammars are large.
- **Landmines:** confirm giallo's license is MIT-compatible in M0. If it isn't, use syntect with `default-fancy`.

## Fonts (native)
- **Choice:** bundle Noto Sans and Noto Sans Arabic (SIL OFL 1.1) in `s-md-slides-render`, used when a deck names a font that isn't installed. System fonts are found through Parley's font enumeration (fontique).
- **Why:** an export from a machine without the deck's fonts still renders Latin and Arabic (Stage 2).

## CLI and `serve`
- **Choice:** `clap` for arguments. `notify` 8.2 plus `notify-debouncer-full` 0.7 (MIT or Apache-2.0, as in mdk) to watch the deck and its assets. `tiny_http` (MIT or Apache-2.0) for a single-threaded local HTTP server, with live reload over server-sent events. Bound to 127.0.0.1, serving only the deck folder with paths canonicalized and checked, and never writing (FR-CLI-01 to 04, NFR-04).
- **Considered:** axum and tokio: a full async stack for one local preview page. rouille: similar to tiny_http, and less active.
- **Why:** the smallest server that meets FR-CLI-03. Server-sent events need no WebSocket library.

## Testing
- **Choice:**
  - Golden round-trip tests on every deck in `tests/decks/`, asserting byte-identical output (FR-FMT-02).
  - `insta` snapshot tests for the parsed model, HTML and flattened SVG.
  - `proptest` for generated decks and random operation sequences, checking that apply plus inverse is identity (FR-OPS-02).
  - `cargo-fuzz` (libFuzzer) targets for the parser and the operation applier. 10 million cases each run nightly and before release (NFR-05).
  - The `conformance/` JSON vectors run against the native core in CI, and are published with each release for the npm repo to run against its build (FR-PKG-03).
  - Image tests: rendered PNGs compared to references with a small tolerance.

## CI/CD and publishing
- **Choice:** GitHub Actions. On every push: fmt, clippy, tests on macOS, Windows and Linux. A WASM build with a size gate that fails above 1 MB. `cargo-deny` for licenses (NFR-09). On a `v*` tag: publish the three crates to crates.io, attach CLI binaries for the three OSes and a `conformance-<version>.zip` to the GitHub release.
- **Why:** one version number across the three crates (FR-PKG-01). The npm package follows the engine's major and minor version, so `s-md-slides@1.4.x` on npm matches crate 1.4.

## Not used
- **Database, auth, hosting, payments, email, analytics:** none.
- **Network:** none in the core or the render crate. Only `serve` opens a local socket.

## Reused from existing work
- **mdk spec (2026-10-11):** the YAML crate, the watcher pair, the Mermaid and math crates, the `cargo-deny` setup and the CI matrix.
- **S-Calc and S-Charts:** Obsidian plugin build and release experience, used later in the plugin spec.
- **SafiCSS and safi-studio-scanner:** npm publishing with TypeScript types and an esbuild bundle.

## Cost
$0 a month. GitHub Actions is free for public repos. crates.io and npm are free.

## Licensing
- **Project:** MIT.
- **Dependencies:** pulldown-cmark (MIT), serde-saphyr (MIT or Apache-2.0), serde and serde_json (MIT or Apache-2.0), ts-rs (MIT), wasm-bindgen (MIT or Apache-2.0), parley (Apache-2.0 or MIT), resvg and tiny-skia (Apache-2.0 or MIT), krilla and krilla-svg (MIT or Apache-2.0), gif and jpeg-encoder (MIT or Apache-2.0), mermaid-rs-renderer (MIT), ratex-svg (MIT), notify and notify-debouncer-full (MIT or Apache-2.0), tiny_http (MIT or Apache-2.0), clap (MIT or Apache-2.0), Noto fonts (OFL 1.1).
- **Check at build time:** giallo's license, and the license of every TextMate grammar bundled. `cargo-deny` enforces it, and `cargo-about` generates the third-party notices for the CLI binaries.

## Top technical risks
1. **WASM size.** The parser, writer, operations, pulldown-cmark, serde-saphyr, serde_json and the HTML runtime must fit in 1 MB. Spike: M0 builds the core to WASM with a stub of every module and measures it. Fallback: drop serde-saphyr for a flat front matter scanner. If it's still too large, fall back to a TypeScript port (Stage 1).
2. **Lossless writing under edits.** Byte identity plus one-line diffs across every operation is the core promise. Spike: M0 writes the scanner and writer for text elements only, and runs proptest with random operation sequences on 1,000 generated decks.
3. **Native text layout that matches the browser.** PNG and PDF (Parley) versus HTML (browser) wrapping. Spike: M0 exports one slide with English, Arabic and mixed text both ways and compares line breaks.
4. **RTL through the flattened SVG into PNG and PDF.** Spike: in the same M0 slide, check that Arabic renders shaped and right-to-left in resvg and stays selectable in krilla's PDF.
5. **Young dependencies** (krilla, parley, mermaid-rs-renderer, ratex). Mitigation: exact pins, upgrades in their own commits, and code isolated behind one module per dependency.

---

### Open questions
None.

### Assumptions confirmed
1. **Repositories : RESOLVED.** Four repos: the Rust engine (crates used by the app), the GPUI desktop app, the npm package (WASM wrapper if the package stays under 4 MB, otherwise a TypeScript port) and the Obsidian plugin (uses the npm package). The engine's conformance vectors keep the npm package honest.
2. **Default fonts : RESOLVED.** Noto Sans and Noto Sans Arabic bundled in `s-md-slides-render`.
3. **Code highlighting : RESOLVED.** Not in WASM. Hosts may color code. Native highlighting is Could (FR-EL-11), because code is a nice-to-have for Safi.

---

Sources:
- wasm-pack and rustwasm sunset: https://blog.rust-lang.org/inside-rust/2025/07/21/sunsetting-the-rustwasm-github-org/ , https://nickb-dev.pages.dev/blog/life-after-wasm-pack-an-opinionated-deconstruction
- wasm-bindgen-cli 0.2.125: https://docs.rs/crate/wasm-bindgen-cli/latest
- pulldown-cmark 0.13.4: https://docs.rs/crate/pulldown-cmark/*
- svg2pdf archived, use krilla: https://github.com/typst/svg2pdf
- svg2pdf text feature size: https://lib.rs/crates/svg2pdf/features
- krilla-svg selectable text: https://docs.rs/krilla-svg
- Parley 0.9.0, bidi: https://docs.rs/crate/parley/0.9.0/source/src/lib.rs , https://linebender.org/blog/tmil-22/
- giallo: https://docs.rs/crate/giallo/0.3.0
- syntect fancy-regex: https://github.com/trishume/syntect
