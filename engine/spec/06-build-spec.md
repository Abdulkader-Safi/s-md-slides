# Build spec: s-md-slides engine

> **This is the hand-off document for Claude Code.** It is self-contained: build from this file alone. It pulls together the product, users, requirements and tech decisions from stages 1 to 5. Where it says "see Stage N", that is optional background.

Status: draft. Sources: 01 to 05 as of 2026-11-15.

## 1. What we're building
`s-md-slides` (Safi Markdown slides) is a Rust library that stores a full free-canvas slide deck (every box placed where it was dragged, like PowerPoint or Google Slides) in one readable markdown file. Element positions live in one-line HTML comments, and the content under each one is normal markdown. The library parses decks, edits them through named operations that each return their own inverse, saves them back changing only the bytes that were edited, and exports them to an HTML presentation, HTML or SVG per slide, PDF, PNG, JPEG and GIF. It also ships a CLI with `validate`, `export`, `serve` (live preview) and `apply`. It is project 1 of 4: the GPUI desktop app builds on these crates, a separate npm package wraps the core (WASM) or ports it (TypeScript) and proves itself against this repo's conformance vectors, and an Obsidian plugin builds on that npm package. Mission: the deck belongs to the `.md` file, not to any app. MIT, open source, for anyone.

## 2. Stack (decided, see Stage 4)
| Layer | Choice |
|-------|--------|
| Language | Rust, MSRV latest stable minus two, edition 2024 |
| Repo | `github.com/Abdulkader-Safi/s-md-slides`, Cargo workspace, three published crates |
| Deck scanner and writer | Own line-based scanner plus source map (byte spans), splice-only writer |
| Markdown inside elements | `pulldown-cmark` 0.13.x (tables, strikethrough, tasklists, math) |
| HTML sanitizing | Own allowlist filter on pulldown-cmark HTML events |
| Front matter | `serde-saphyr` 1.3.x, read only |
| Serialization | `serde`, `serde_json`. TypeScript types generated with `ts-rs` |
| WASM target (size gate only) | `wasm32-unknown-unknown`, wasm-bindgen-cli plus binaryen `wasm-opt -Oz`. No wasm-pack |
| Presentation runtime | Vanilla TypeScript, bundled with esbuild, committed as JS, embedded with `include_str!` |
| Native text layout | `parley` 0.9.x |
| Rasterizing | `resvg` and `tiny-skia`. JPEG `jpeg-encoder`. GIF `gif` (NeuQuant) |
| PDF | `krilla` plus `krilla-svg`, selectable text. Not svg2pdf (archived 2026-07-10) |
| Mermaid and math (native) | `mermaid-rs-renderer` 0.3.x, `ratex-svg` |
| Code highlighting (native, Could) | `giallo` (license checked in M0), fallback `syntect` with `default-fancy` |
| Fonts (native) | Noto Sans and Noto Sans Arabic (OFL 1.1), bundled. System fonts via fontique |
| CLI | `clap`, `notify` 8.2 plus `notify-debouncer-full` 0.7, `tiny_http`, server-sent events |
| Tests | `insta`, `proptest`, `cargo-fuzz`, `criterion`, conformance JSON vectors, PNG reference images |
| CI and release | GitHub Actions on macOS, Windows, Linux. `cargo-deny`, `cargo-about` |
| Database, network, auth, hosting | None. $0 running cost |

Pin every dependency to an exact version in M0, checked on crates.io that day.

## 3. Architecture and principles

```
                 host app (GPUI app, npm wrapper, CLI, anyone)
                   |  text in / text out, asset bytes via resolver
                   v
+--------------------------- s-md-slides (core) ----------------------------+
|  scanner --> Source map (byte spans) --> Deck model                       |
|                 ^                            |                            |
|                 |       Op --> validate --> apply --> inverse Op          |
|                 +------ writer (splices changed spans only) <-+           |
|  Theme resolver   Timeline   Diagnostics   HTML / browser-SVG export      |
|  Presentation runtime (embedded JS/CSS)                                   |
+---------------------------------------------------------------------------+
        |  model + resolver                         |  core API
        v                                           v
+---- s-md-slides-render (native) -----+   +---- s-md-slides-cli ------------+
| parley layout -> flattened SVG       |   | validate | export | apply       |
| resvg -> PNG/JPEG   gif -> GIF       |   | serve: notify + tiny_http + SSE |
| krilla -> PDF   mermaid / math       |   +---------------------------------+
+--------------------------------------+
```

The keystone is the source map. The deck keeps the original text plus a byte span for every front matter key, directive line, attribute token and body. Operations edit the model and mark spans dirty. The writer copies the original and splices only dirty spans, then shifts later offsets.

**Non-negotiable principles:**
1. **Never print the file from the model.** Output is the original text with changed spans spliced in. Unchanged save is byte-identical. One move changes one line.
2. **Never lose data.** Unknown front matter keys, attributes and directive kinds round-trip in place. Ordinary comments stay text.
3. **The core stays WASM-safe.** No `std::fs`, no network, no threads, no fonts and no rasterizer in `s-md-slides`. Anything heavy goes in `s-md-slides-render`. The WASM size gate runs on every push.
4. **The engine never touches files.** Hosts pass text in and out. Assets come only through `AssetResolver`.
5. **Never panic on input.** Malformed decks give a best-effort model plus diagnostics with line and column. Fuzzed.
6. **Every edit is an `Op`.** Validate first, change nothing on error, return the inverse. Hosts, MCP, the CLI and agents all use the same ops in JSON.
7. **One timeline.** HTML, GIF and hosts all read `state_at(t)` from the core.
8. **Positions are never mirrored.** RTL changes text direction and presenter controls only.

## 4. Repository structure
```
s-md-slides/
  Cargo.toml                 workspace, shared release profile (opt-level z for wasm builds)
  crates/
    core/                    crate `s-md-slides`
      src/scanner.rs         lines, fences, front matter, separators, directives
      src/source_map.rs      spans, dirty tracking, offset shifting
      src/attrs.rs           attribute tokenizer, quoting, number formatting
      src/model/             Deck, FrontMatter, Theme, Master, Layout, Slide, Element, kinds
      src/frontmatter.rs     serde-saphyr reading, key-line edits
      src/ops.rs             Op enum, validate, apply, inverse, batch
      src/writer.rs          splicing
      src/theme.rs           role resolution, master and layout inheritance
      src/timeline.rs        steps, state_at
      src/diag.rs            Diagnostic, codes
      src/query.rs           elements_at, search, assets
      src/export/            html.rs, svg.rs (foreignObject), sanitize.rs, presentation.rs
      src/runtime/runtime.js generated from /runtime, committed, CI checks it is current
    render/                  crate `s-md-slides-render` (native only)
      src/layout.rs flatten.rs raster.rs gif.rs pdf.rs mermaid.rs math.rs fonts.rs progress.rs
      assets/fonts/          Noto Sans, Noto Sans Arabic, licenses
    cli/                     crate `s-md-slides-cli`, binary `s-md-slides`
      src/main.rs validate.rs export.rs serve.rs apply.rs resolver.rs
  runtime/                   TypeScript presentation runtime, esbuild config
  conformance/               JSON vectors (one file per case)
  tests/decks/               golden decks (25 or more)
  examples/                  hello.md, pitch-16x9.md, carousel-4x5.md, all-elements.md
  themes/default.json
  docs/format.md             full grammar, one example per kind
  skills/s-md-slides/SKILL.md
  AGENTS.md
  fuzz/                      fuzz_parse, fuzz_ops
  benches/                   criterion
  scripts/wasm-size.sh
  .github/workflows/         ci.yml, fuzz.yml (nightly), release.yml
  PROGRESS.md
  LICENSE                    MIT
```

## 5. Data model
(Full tables in Stage 5.)
- **Deck:** `source`, `version` (default 1, above 1 is read-only), `front`, `masters`, `layouts`, `slides`, `line_ending`, id counters.
- **FrontMatter:** `title`, `author`, `canvas` (`16:9` = 1920x1080 default, `4:5` = 1080x1350, `WxH`), `dir` (auto, ltr, rtl), `lang`, `theme` (inline or JSON path), `transition`, unknown keys.
- **Theme:** colors by role (background, text, accent1 to accent6, code, code-bg), fonts (heading, body, code), default background.
- **Master:** `id`, elements drawn behind slides. Text may use `{slide}`, `{total}`, `{title}`.
- **Layout:** `id`, `master`, placeholders (id, geometry, kind).
- **Slide:** `layout`, `master` (inherit, none, id), `hide`, `bg`, `transition`, `elements` (file order = stacking), `notes`, unknown attributes.
- **Element:** `id`, `kind`, `x y w h rot opacity locked name dir`, style (font, size, color, weight, align, valign, line-height, padding, fill, stroke, stroke-width, radius), `enter exit order delay dur`, raw `body`, unknown attributes. Kinds: text, image (`fit`, `crop`), shape (`kind` rect, ellipse, line, arrow), code (`lines`), table (`header`), diagram (mermaid), math, video (`src poster autoplay loop muted`), embed (https `src`, `poster`), group (`items`), unknown.
- **Timeline:** steps, duration, `state_at(t)` (opacity, dx, dy, scale per element).
- **AssetRef:** relative `path` as written, `kind` (image, video, poster, theme, background), `used_by`. Bytes come only through `AssetResolver`.
- **Diagnostic:** severity, code, message, line, column, element, slide.

File grammar summary (format version 1):
1. UTF-8, BOM kept, line endings kept.
2. Optional YAML front matter between `---` lines at the top.
3. A line that is exactly `---` outside front matter and code fences separates sections. Horizontal rules in text use `***` or `___`.
4. A section whose first directive is `master` or `layout` holds definitions. Every other section is a slide, optionally opened by `<!-- slide ... -->`.
5. A directive is a whole line `<!-- kind attr ... -->`, counted only if `kind` is known (slide, master, layout, placeholder, notes, text, image, shape, code, table, diagram, math, video, embed, group) or it has `id=`.
6. Attributes are `key=value`, `key="quoted \" value"` or bare `key`. Order kept.
7. A body runs to the next directive, separator or end of file.
8. Markdown before a slide's first directive becomes an implicit full-slide text element.
9. `<!-- notes -->` holds speaker notes to the end of the slide.
10. Ids `[a-z0-9-]{1,32}`, generated as kind letter plus counter (`t1`, `i4`).

## 6. API surface
Core (`s-md-slides`):
- `Deck::parse(&str) -> Parsed { deck, diagnostics }`, `deck.text()`, `deck.is_read_only()`, `deck.diagnostics()`
- `deck.apply(Op) -> Result<Op>` (inverse), `deck.apply_batch(Vec<Op>) -> Result<Op>`
- Ops: `AddSlide`, `RemoveSlide`, `DuplicateSlide`, `MoveSlide`, `AddElement` (structured or `raw`), `RemoveElement`, `DuplicateElement`, `SetGeometry`, `SetAttrs`, `SetBody`, `SetZ`, `SetNotes`, `SetSlideAttrs`, `SetFrontMatter`, `AddDefinition`, `RemoveDefinition`, `Group`, `Ungroup`, `Batch`. JSON tagged with `"op"`.
- Queries: `slides`, `slide`, `element`, `canvas`, `resolved_style`, `elements_at`, `search`, `assets`, `timeline`
- Export: `export::slide_html`, `export::slide_svg` (Frame `Start | End | At(ms)`), `export::presentation_html` (with `embed`), `trait AssetResolver`
- Errors: `EngineError { code, message, element, slide }`, serializable

Render (`s-md-slides-render`): `flatten_svg`, `png(scale)`, `jpeg(scale, quality)`, `gif(fps, looped, progress)`, `pdf(progress)`, `trait Progress -> Control (Continue | Cancel)`, `Fonts::bundled / with_system / add_dir`.

CLI (`s-md-slides`), exit codes 0 ok, 1 deck errors, 2 usage, 3 I/O, 4 cancelled:
- `validate <deck> [--json]`
- `export <deck> --to html|pdf|png|jpeg|gif [--slide N] [--scale 1|2] [--quality 90] [--fps 15] [--embed] [-o PATH]`
- `serve <deck> [--host 127.0.0.1] [--port 4321] [--open]`
- `apply <deck> <ops.json> [--dry-run]` (prints the inverse batch)

## 7. Outputs (the engine has no screens)
- **HTML presentation:** stage scaled to the window, previous and next buttons, counter, progress bar, fullscreen (F), presenter view (P) in a second window synced with `BroadcastChannel` showing current and next slide, notes and timer. Keys: Right, Down, Space, PageDown next. Left, Up, PageUp previous. Home, End. Swipe. RTL decks swap Left and Right. States: loading, presenting, end screen, missing-asset placeholder. `prefers-reduced-motion` makes motion instant. CSP meta tag with frame-src limited to youtube-nocookie and player.vimeo.
- **serve:** `GET /` presentation with reload script, `GET /events` SSE `reload` (keeps the current slide), `GET /<path>` deck-folder files only, other methods 405. Parse errors show a diagnostics overlay.
- **CLI text:** `file:line:col: severity code message` on stderr, `--json` for tools, colors unless `NO_COLOR`.

## 8. Build plan (milestones)

### M0: de-risk (do this first)
Throwaway spikes in a `spikes/` folder, deleted or folded in after.
- **Tasks:**
  1. **WASM size:** core crate with the scanner stub, serde-saphyr, pulldown-cmark, serde_json and the runtime string, built for `wasm32-unknown-unknown` with the release profile and `wasm-opt -Oz`. Measure.
  2. **Lossless writer:** scanner, source map and writer for front matter, separators and text elements only, plus `SetGeometry`, `SetBody` and `RemoveElement` with inverses. proptest on 1,000 generated decks with random op sequences.
  3. **Layout match and RTL:** one slide with English, Arabic and mixed text. Export it as HTML (browser) and through parley, flattened SVG, resvg PNG and krilla PDF. Compare line breaks. Check Arabic is shaped and right-to-left in the PNG, and selectable in the PDF.
  4. **License check:** confirm giallo's license, and the license of every bundled grammar and font.
  5. Pin exact versions of every dependency in `Cargo.toml`.
- **Exit criteria:** WASM under 1 MB (if 1 to 1.5 MB, continue and log it in PROGRESS.md; if over 1.5 MB, replace serde-saphyr with a flat front matter scanner and re-measure; if still over, stop and report). Zero byte differences across all proptest cases. Line breaks match on the test slide or differ only at the last word with the same fonts, and Arabic renders correctly. If Arabic fails in krilla or resvg, stop and report before M1.

### M1: format core
- **Builds:** FR-FMT-01, FR-FMT-02, FR-FMT-03, FR-FMT-04, FR-FMT-05, FR-FMT-06, FR-FMT-07, FR-FMT-08, FR-CANVAS-01, FR-CANVAS-02, FR-NOTES-01, FR-DIR-01, FR-PKG-02
- **Tasks:** full scanner (fences, BOM, CRLF, front matter, sections, directives, attributes, bodies, implicit text, notes), source map, model types, id generation, diagnostics with codes, limits (20 MB, 5,000 slides, 2,000 elements per slide, 4 KB attribute values), read-only for newer versions. First 15 golden decks. `fuzz_parse` target. CI workflow with fmt, clippy, tests on three OSes and the WASM size gate.
- **Exit criteria:** every golden deck round-trips byte for byte. Fuzzing runs 1 million cases with no panic. WASM size gate passes.

### M2: operations
- **Builds:** FR-OPS-01, FR-OPS-02, FR-OPS-03, FR-OPS-04, FR-OPS-05, FR-EL-10
- **Tasks:** every `Op` variant with validation and inverse, `raw` insertion, batch, groups, queries (`elements_at` rotation-aware, `search`, `assets`). JSON round-trip for every op. `fuzz_ops` target. proptest over all op types.
- **Exit criteria:** apply plus inverse is byte-identical for every op type over 10,000 generated cases. Each op touches only its targets' spans.

### M3: elements and style
- **Builds:** FR-EL-01, FR-EL-02, FR-EL-03, FR-EL-04, FR-EL-05, FR-EL-06, FR-EL-07, FR-EL-08, FR-EL-09, FR-STYLE-01, FR-STYLE-02, FR-STYLE-03, FR-STYLE-04, FR-STYLE-05
- **Tasks:** kind-specific attributes and body validation for all kinds, alt-text warning, theme loading (inline and JSON via resolver), role resolution, masters with `{slide}`, `{total}`, `{title}`, layouts and placeholders, `AddSlide` from a layout, `master=none` and `hide`, backgrounds. Golden decks for each kind. `themes/default.json`.
- **Exit criteria:** `resolved_style` correct for every golden deck. 25 golden decks in total, all round-tripping.

### M4: animation and HTML export
- **Builds:** FR-ANIM-01, FR-ANIM-02, FR-ANIM-03, FR-ANIM-04, FR-DIR-02, FR-EXP-01, FR-EXP-02, FR-EXP-07, FR-ASSET-01, FR-ASSET-02, FR-ASSET-03
- **Tasks:** timeline and `state_at`. Slide HTML and browser SVG. Sanitizer with warnings. Presentation runtime in TypeScript (controls, keys, swipe, presenter view, build steps, transitions, RTL swap, reduced motion, end screen, CSP), built with esbuild and committed, with a CI check that it is current. Embed mode with data URIs. Video and embed output.
- **Exit criteria:** `examples/pitch-16x9.md` and `examples/carousel-4x5.md` present fully in Chrome, Safari and Firefox from a single offline HTML file. Keyboard-only use works. axe-core reports no WCAG 2.2 AA violations on the presentation controls.

### M5: native render
- **Builds:** FR-EXP-03, FR-EXP-04, FR-EXP-05, FR-EXP-06, FR-EL-11
- **Tasks:** parley layout into flattened SVG with positioned text runs, bundled Noto fonts plus system fonts, resvg PNG, JPEG, krilla PDF with selectable text, GIF from timeline frames, progress and cancel, Mermaid and math to SVG (code box plus warning when a diagram fails), video and embed poster frames. FR-EL-11 highlighting only if time allows. PNG reference images.
- **Exit criteria:** every example deck exports to every format. Reference PNGs within 0.5% pixel difference. PDF text is selectable. A three-step slide makes a GIF that plays in order. Cancel stops within 200 ms.

### M6: CLI
- **Builds:** FR-CLI-01, FR-CLI-02, FR-CLI-03, FR-CLI-04
- **Tasks:** clap commands and exit codes, the CLI asset resolver with path checks, export progress bar, `serve` with notify, tiny_http and SSE, diagnostics overlay, keep-current-slide reload, never writing, 127.0.0.1 default with a warning for other hosts, `apply` with `--dry-run` printing the inverse.
- **Exit criteria:** editing a deck in Vim updates an open browser on the same slide within 500 ms, and the deck's modified time never changes because of `serve`. A request for `/../secret` returns 404.

### M7: conformance, docs, agents and release
- **Builds:** FR-PKG-01, FR-PKG-03, FR-AGENT-01, FR-AGENT-02
- **Tasks:** conformance vectors covering every FR-FMT, FR-OPS and FR-ANIM-04 acceptance case, `docs/format.md` with every example tested, `AGENTS.md`, `skills/s-md-slides/SKILL.md` (format, canvases, ops, CLI, how the desktop app's MCP tools map to ops), README with the 10-line example, criterion benchmarks, nightly 10-million-case fuzz workflow, `cargo-deny`, `cargo-about` notices, release workflow publishing three crates plus CLI binaries and `conformance-<version>.zip`.
- **Exit criteria:** every acceptance box below is ticked. A fresh Claude Code session with only the skill writes a five-slide deck that `validate` passes. `v0.1.0` tag publishes cleanly to crates.io.

## 9. Acceptance criteria

### POC (proves the core idea)
- [x] **FR-FMT-01** The engine must parse a deck from a UTF-8 string into a deck model: front matter, masters, layouts, slides, elements, notes. Check: the sample deck parses into the expected model (slide count, element count, every attribute value), checked by a golden test.
- [x] **FR-FMT-02** The engine must serialize a deck model back to a string. Check: parse then serialize of every deck in the sample suite returns a string identical byte for byte to the input.
- [x] **FR-FMT-03** Edits must change only the lines of the elements they touch. Everything else (spacing, comments, unknown attributes, markdown inside untouched elements, line endings) must stay as written. Check: moving one element in a 50-slide deck produces a diff of exactly one changed line.
- [x] **FR-FMT-04** Unknown front matter keys, unknown directive attributes and unknown directive kinds must be kept on save, in place. Check: a deck with `foo=bar` on an element and an unknown `<!-- widget ... -->` directive round-trips unchanged, and survives a move of that element.
- [x] **FR-FMT-05** The deck must declare its format version (`s-md-slides: 1`). A missing version means 1. A version newer than the engine supports opens read-only with a diagnostic. Check: a `s-md-slides: 2` deck loads, `is_read_only()` is true, and edit operations return an error.
- [x] **FR-FMT-06** Malformed input must never fail the whole parse. The engine returns a best-effort deck plus diagnostics, each with severity, message, line and column. Check: a deck with a bad attribute (`x=abc`) on slide 3 loads all slides, and returns one error diagnostic pointing at that line and column.
- [x] **FR-FMT-07** Every element has an `id` that is unique within the deck. The engine writes a short id when it creates an element, and assigns ids on load to hand-written elements that lack one, written on the next save of that element. Check: a hand-written element without `id` gets one in memory. Saving after editing it writes the id. Saving without editing writes nothing new.
- [x] **FR-CANVAS-01** The deck sets its canvas in front matter: `16:9` (1920 x 1080, default), `4:5` (1080 x 1350), or a custom `WxH` in units. Check: each of the three forms parses to the right width and height. A missing value gives 1920 x 1080.
- [x] **FR-CANVAS-02** Element positions and sizes are in canvas units. They are kept as written, even when they fall outside the canvas (which gives a warning). Check: an element at x=-200 round-trips unchanged and produces one warning.
- [x] **FR-EL-01** Text element: markdown body (headings, emphasis, lists, links, inline code), with style attributes for font, size, color, weight, alignment, vertical alignment, line height and padding. Check: a text element with `size=48 align=center color=#fff` parses those values, and the HTML export renders them.
- [x] **FR-DIR-01** Deck default direction in front matter (`dir: auto|ltr|rtl`, default auto) and a per-element override. `auto` resolves from the first strong character. Check: an Arabic text box with no `dir` resolves to rtl. An English one resolves to ltr. `dir=ltr` forces ltr.
- [x] **FR-NOTES-01** Each slide can carry speaker notes in markdown after a `<!-- notes -->` directive. Check: notes parse, round-trip, appear in the HTML presenter view and never appear on the slide.
- [x] **FR-OPS-01** Every change goes through a named, serializable operation: add, remove, duplicate and reorder slides; add, remove, duplicate, move, resize, rotate and restyle elements; set text; set attributes; change z-order; set notes; set slide background, layout and transition; set front matter values. Check: each operation round-trips through JSON and gives the same result in Rust and in the npm build.
- [x] **FR-OPS-02** Applying an operation returns its inverse operation, so hosts get undo and redo for free. Check: for every operation type, apply then apply-inverse returns a byte-identical file.
- [x] **FR-OPS-03** Operations validate before they change anything. An invalid one (unknown id, negative width) returns an error and leaves the deck unchanged. Check: `move_element("nope", ...)` returns an error and the serialized deck is unchanged.
- [x] **FR-OPS-04** A batch of operations applies as one step, with one combined inverse. Check: a batch of 20 moves undoes with one inverse.
- [x] **FR-EXP-01** HTML and SVG per slide, at a chosen frame of its animation (start, end, or a time). Check: the SVG of slide 1 at its end state shows all elements in the right places.
- [x] **FR-PKG-02** The core crate builds for `wasm32-unknown-unknown` with no file system, network or threads, so the npm repo can wrap it. Engine CI builds it on every push and reports its size. Check: the CI job builds the core to WASM and fails above 1.5 MB.

### MVP (shippable v1)
- [x] **FR-EL-02** Image element: one markdown image (path and alt text), with `fit=contain|cover|fill` and an optional crop. Check: the image path, alt text and fit parse. A missing alt text gives a warning.
- [x] **FR-EL-03** Shape element: `kind=rect|ellipse|line|arrow`, with fill, stroke, stroke width and corner radius, and optional text inside rect and ellipse. Check: each kind parses and exports to SVG with the right geometry.
- [x] **FR-EL-04** Code element: a fenced code block with a language tag, shown in the theme's code font and code colors, with an optional line-highlight list. The language tag is kept so hosts can color it. Check: a `rust` block exports in the code font on the code background, with highlighted lines marked, and its language tag round-trips.
- [x] **FR-EL-05** Table element: a markdown table, with header and cell styles from the theme. Check: a 3 x 4 GFM table exports as a table with the right cells and alignment.
- [x] **FR-STYLE-01** A theme sets colors (named roles: background, text, accent 1 to 6, code), fonts (heading, body, code) and the default slide background. It sits inline in front matter or in a JSON file referenced by path. Check: a text element with `color=accent1` resolves to the theme's accent 1. Changing the theme file changes the export.
- [x] **FR-STYLE-02** Element style values can be literal (`#e11`, `Inter`) or theme roles (`accent1`, `heading`). Check: both forms parse and round-trip exactly as written.
- [x] **FR-STYLE-05** Slide background: color, gradient or image, per slide, overriding the theme. Check: `bg=#0b1020` and `bg=assets/hero.jpg` both export correctly.
- [x] **FR-ANIM-01** Per-element entrance and exit effects: `appear`, `fade`, `fly-left|right|up|down`, `scale`. Each has an order, a delay and a duration (milliseconds). Check: `enter=fly-left delay=300 dur=500` parses, round-trips and plays in the HTML presentation.
- [x] **FR-ANIM-02** Build steps: elements with the same order appear together on one click. Elements without an order are visible from the start. Check: a slide with orders 1, 1, 2 takes two clicks to finish in the HTML presentation.
- [x] **FR-ANIM-03** Slide transitions: `none`, `fade`, `push`, with a duration. Check: `transition=push` plays between slides in the HTML presentation.
- [x] **FR-ANIM-04** The engine produces an animation timeline for a slide (each element's state at any time in milliseconds), so every host and the GIF export play the same thing. Check: `timeline(slide).state_at(t)` returns matching opacity and offset values in the Rust and npm builds for the same deck.
- [x] **FR-DIR-02** In an rtl deck, the HTML presentation mirrors its controls and arrow keys. Element positions are never mirrored. Check: in an rtl deck, Left Arrow goes to the next slide, and every element keeps its x.
- [x] **FR-ASSET-01** Images, videos and theme files are referenced by paths relative to the deck file. The engine never reads files. The host passes asset bytes to export through a resolver callback. Check: export asks the resolver for `assets/chart.png` exactly once, and works in Obsidian with a vault-backed resolver.
- [x] **FR-ASSET-02** Export can embed all assets into the output (single-file HTML with data URIs). The source deck always keeps relative paths. Check: `export_html(embed=true)` produces one file that works offline with the deck's folder deleted.
- [x] **FR-ASSET-03** The engine lists every asset a deck references, with any missing ones flagged by the host's resolver. Check: `assets()` returns every path once. A missing file gives a warning diagnostic on export.
- [x] **FR-EXP-02** A standalone HTML presentation: next and previous controls, keyboard (arrows, space, Home, End, F for fullscreen), touch swipe, slide counter, presenter view with notes, next slide and timer, animations and transitions, `prefers-reduced-motion` respected. Check: the sample deck exports to one HTML file that presents fully in Chrome, Safari and Firefox, offline when assets are embedded.
- [x] **FR-EXP-03** PDF: one page per slide at its end state, text kept as selectable text. Native build only. (In Obsidian, the plugin prints the HTML export to PDF.) Check: a 12-slide deck gives a 12-page PDF whose text can be selected and searched.
- [x] **FR-EXP-04** PNG and JPEG of a single slide, at a chosen scale (1x, 2x) and JPEG quality. Native build only. (In Obsidian, from the SVG through the browser canvas.) Check: slide 3 at 2x gives a 3840 x 2160 PNG.
- [x] **FR-EXP-06** Exports report progress for long jobs (PDF, GIF) and can be cancelled. Check: a GIF export emits progress events from 0 to 100 and stops within 200 ms of cancel.
- [x] **FR-EXP-07** Raw HTML inside markdown is sanitized in every export: no `<script>`, no `on*` handlers, no `javascript:` URLs. iframes are allowed only through the embed element. Check: a deck containing `<script>` and `<img onerror>` exports with both removed, plus a warning.
- [x] **FR-CLI-01** `s-md-slides validate <deck>` prints diagnostics and exits non-zero on errors. Check: a broken deck exits 1 with `file:line:col` messages.
- [x] **FR-CLI-02** `s-md-slides export <deck> --to html|pdf|png|jpeg|gif [--slide N] [--scale 2] [--embed]`. Check: each format writes the expected file. `--slide` limits image formats to one slide.
- [x] **FR-CLI-03** `s-md-slides serve <deck>` serves the HTML presentation on 127.0.0.1, watches the deck and its assets, and reloads open browsers on change, keeping the current slide. Preview only, it never writes the deck. `--host` and `--port` override. Check: editing the deck in Vim updates the open browser on the same slide within 500 ms. The deck's modified time never changes because of the server.
- [ ] **FR-AGENT-01** The repo ships `AGENTS.md` and a Claude skill (`skills/s-md-slides/SKILL.md`) covering the format, the canvas presets, the operation list, the CLI and how the desktop app's MCP tools map to operations. Check: a fresh Claude Code session given only the skill writes a five-slide deck that `validate` passes with no errors.
- [ ] **FR-AGENT-02** A written format spec (`docs/format.md`) with the full grammar and one example per element type, plus a sample deck suite used by the tests. Check: every example in the spec is in the test suite and parses with no diagnostics.
- [ ] **FR-PKG-01** The engine repo publishes three crates to crates.io: `s-md-slides` (core), `s-md-slides-render` (native export) and `s-md-slides-cli` (the `s-md-slides` binary), all on one version number. Check: `cargo add s-md-slides` runs the 10-line README example, and `cargo install s-md-slides-cli` gives a working `s-md-slides` command.
- [ ] **FR-PKG-03** Conformance vectors: the engine repo ships a `conformance/` folder of JSON cases (input deck, operations, expected output text, expected model, expected diagnostics, timeline samples), versioned with the format and released with each engine version. Any implementation (the npm WASM wrapper or a TypeScript port) proves it matches by passing all of them. Check: the vectors cover every FR-FMT, FR-OPS and FR-ANIM-04 acceptance case, and the native core passes 100% of them in CI.

### v1 Should and Could (not blocking the first release)
- [x] **FR-FMT-08** A plain markdown file with no directives must open as a deck: each `---`-separated section becomes a slide with one text element filling the safe area. Check: a three-section note opens as three slides, each with its text visible.
- [x] **FR-EL-06** Diagram element: a Mermaid source block. The engine stores the source and the host or native export renders it. Check: the source round-trips. Native export produces an image of the diagram. The npm API returns the source for the host to render.
- [x] **FR-EL-07** Math element: a LaTeX block, rendered the same way as diagrams. Inline `$...$` math inside text elements is also kept. Check: same as FR-EL-06.
- [x] **FR-EL-08** Video element: a relative path to a video file, with poster frame, autoplay, loop and muted flags. Plays in the HTML presentation. PDF, PNG, JPEG and GIF use the poster frame. Check: a video plays in HTML export. The PNG export shows the poster.
- [x] **FR-EL-09** Embed element: a web URL (YouTube, Vimeo, or a page), shown as an iframe in the HTML presentation and as its poster image elsewhere. The engine never fetches it. Check: a YouTube embed plays in HTML export. No network request is made by `parse`, `serialize` or native export.
- [ ] **FR-EL-11** Syntax highlighting of code elements in native exports (token colors from the theme). Not part of the first release. Check: a `rust` block exports with token colors from the theme.
- [x] **FR-EL-10** Group: a `group` directive with `items=` (ids) that hosts move and resize as one. Check: `move_element` on a group moves every member, and writes only the members' lines.
- [x] **FR-STYLE-03** Masters: named sets of elements (logo, footer, slide number) drawn behind every slide that uses them. Text in masters can contain `{slide}`, `{total}` and `{title}`. Check: a master footer with `{slide} / {total}` exports as "3 / 12" on slide 3 of 12.
- [x] **FR-STYLE-04** Layouts: named placeholder positions on a master. A new slide created from a layout gets one element per placeholder. A slide can turn off its master (`master=none`) or hide single master elements. Check: `add_slide(layout="title-body")` creates two text elements at the placeholder positions. `master=none` drops the logo from that slide's export.
- [x] **FR-OPS-05** Query helpers: find an element by id, list elements at a point (top first), and search text across slides and notes. Check: `elements_at(x, y)` on overlapping shapes returns them top first.
- [x] **FR-EXP-05** GIF of a single animated slide: its full timeline at a chosen frame rate (default 15 fps), looping optional. Slides without animation export as a one-frame GIF with a warning. Native build only. Check: a slide with three build steps gives a GIF that plays all three in order.
- [x] **FR-CLI-04** `s-md-slides apply <deck> <ops.json>` applies a batch of operations and writes the deck (for scripts and agents). Check: a JSON batch of moves changes only those lines.

### Non-functional
- [ ] **NFR-01** Performance: parse plus serialize of a 200-slide, 2,000-element deck under 50 ms native on an M1. One op plus serialize under 5 ms. One 1920 x 1080 PNG under 300 ms. `serve` reload under 500 ms. Measured by criterion and the M6 test.
- [ ] **NFR-02** Size: the core's WASM build under 1 MB uncompressed, never above 1.5 MB, enforced by `scripts/wasm-size.sh` in CI.
- [ ] **NFR-03** Round-trip: every golden deck and conformance case saves byte-identical, and every op touches only its targets' spans.
- [ ] **NFR-04** Privacy and security: no network in core or render. `serve` binds 127.0.0.1 by default, serves only the deck folder, never writes. Exports are sanitized and carry the CSP.
- [ ] **NFR-05** Robustness: `fuzz_parse` and `fuzz_ops` each pass 10 million runs with no panic before release.
- [ ] **NFR-06** Platforms: CI green on macOS, Windows and Linux at the MSRV, plus the `wasm32-unknown-unknown` core build.
- [ ] **NFR-07** Accessibility and locale: axe-core finds no WCAG 2.2 AA violations in the presentation. Keyboard-only use, alt text, reading order and reduced motion work. Arabic and mixed text render correctly in every format.
- [ ] **NFR-08** Stability: semver on all three crates. A deck with unknown keys from a newer minor version loads and saves without loss. A `s-md-slides: 2` deck opens read-only.
- [ ] **NFR-09** License: MIT. `cargo-deny` passes with no GPL or AGPL. Third-party notices generated by `cargo-about`.
- [ ] **NFR-10** Docs: every public item documented on docs.rs, TypeScript types generated, `docs/format.md` and the skill match the code (their examples are tests).

## 10. Standard states
- Undo and redo: FR-OPS-02, FR-OPS-04 (hosts keep the stack).
- Empty states: an empty string is a deck with no slides and no errors (FR-FMT-06).
- Loading and progress: FR-EXP-06. Presentation loading state (section 7).
- Errors: FR-FMT-06, FR-OPS-03.
- Confirm before destructive actions: out of scope (host UI). All destructive ops are undoable.
- Autosave and reopen: out of scope (hosts own files). FR-FMT-02 guarantees the round trip.
- Search: FR-OPS-05.
- Keyboard shortcuts: FR-EXP-02 (presentation). Editor shortcuts are the hosts' job.
- Persistent settings: front matter (FR-CANVAS-01, FR-DIR-01, FR-STYLE-01).
- First run: example decks and README (FR-AGENT-02, FR-PKG-01).
- Offline: fully offline. Only embeds reach the network, in the viewer's browser.
- Permissions per role: out of scope, no roles.
- Notifications: out of scope.
- Import and export: FR-EXP-01 to FR-EXP-07. Import from PowerPoint, Keynote or Marp: out of scope.
- Delete and restore: inverse ops (FR-OPS-02).
- Audit log: out of scope, git is the history.
- Multi-language and RTL: FR-DIR-01, FR-DIR-02.
- Accessibility: FR-EXP-02, FR-EL-02, NFR-07.
- Mobile layout: the presentation scales to any screen and supports swipe (FR-EXP-02).
- Rate limits: out of scope.

## 11. Out of scope for MVP
Don't build these even if they look easy.
- PowerPoint, Keynote, Google Slides or Marp import and export.
- Motion paths, emphasis effects, morph.
- Audio, narration, recorded timings.
- Real-time collaboration or sync.
- Charts from data.
- A plugin API for custom element types (unknown directives already round-trip).
- Live video or embed frames in GIF, PDF or images (poster only).
- Any editor UI.
- The npm package and the Obsidian plugin (their own repos). This repo only provides the WASM-safe core, the size gate and the conformance vectors.

## 12. Top risks and mitigations
1. **WASM size over budget.** M0 task 1. Fallback: flat front matter scanner instead of serde-saphyr. If still over, report: the npm repo falls back to a TypeScript port.
2. **Lossless writing breaks under edits.** M0 task 2, then proptest and fuzzing in every milestone.
3. **Native layout differs from the browser.** M0 task 3. Same bundled fonts and line-height rules in both. Differences beyond the last word of a line get logged in PROGRESS.md with a screenshot.
4. **RTL lost in PNG or PDF.** M0 task 3. Stop and report if it fails.
5. **Young dependencies** (krilla, parley, mermaid-rs-renderer, ratex). Exact pins, upgrades in their own commits, one module per dependency so each can be swapped.

## 13. Instructions for Claude Code
- Build milestone by milestone, in order. Finish M0 before any feature work.
- Keep a `PROGRESS.md` at the repo root: milestone status, decisions made, deviations from this spec and why.
- At the end of each milestone, run the tests and tick the acceptance boxes it covers. Don't move on with a failing box.
- Don't add features outside this spec. If something seems missing, note it in `PROGRESS.md` and ask.
- If a spec decision turns out to be wrong in practice, stop and explain. Don't silently swap a stack choice.
- Commit after each milestone with a message naming the milestone.
- Keep secrets out of the repo. There are none to keep: the engine needs no keys.
- Never add a dependency to `crates/core` without re-running `scripts/wasm-size.sh`.
- Write all docs, comments and the skill in plain US English, sentence-case headings, no em dashes.

---

*Companion files in `docs/spec/`: 01-product, 02-users, 03-requirements, 04-tech-decisions, 05-tech-requirements. This build spec is the entry point.*
