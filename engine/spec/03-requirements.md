# Functional requirements

Status: signed off 2026-11-15

> **Changes:** 2026-11-15 (after sign-off): split into four repos (Rust engine, GPUI app, npm package, Obsidian plugin). The npm package is its own project: a WASM wrapper of the core if the package stays under 4 MB, otherwise a TypeScript port, proven by the engine's conformance vectors. Code syntax highlighting dropped to Could.

Requirements say what the engine must do, not how. Tech choices are in Stage 4. Each traces to the product (Stage 1) and the jobs and journey (Stage 2). The exact grammar is written in Stage 5. The sketch below only fixes the shape so the requirements are concrete.

## Format sketch

```markdown
---
s-md-slides: 1
title: Q3 review
canvas: 16:9
dir: auto
theme: ./themes/dsrpt.json
---

<!-- master id=brand -->
<!-- image id=logo x=1720 y=40 w=160 h=60 -->
![dsrpt](assets/logo.svg)

<!-- layout id=title-body master=brand -->
<!-- placeholder id=title x=120 y=80 w=1680 h=200 -->
<!-- placeholder id=body x=120 y=320 w=1680 h=640 -->

---

<!-- slide layout=title-body transition=fade -->

<!-- text id=t1 x=120 y=80 w=1680 h=200 enter=fade -->
# Quarterly review

<!-- text id=t2 x=120 y=320 w=760 h=560 dir=rtl enter=fly-right delay=300 -->
- مبيعات أعلى بنسبة ١٢٪
- Second point

<!-- shape id=s1 kind=arrow x=900 y=600 w=200 h=0 stroke=#e11 -->

<!-- notes -->
Mention the Kuwait numbers first.
```

Rules the sketch shows: front matter sets the deck. Masters and layouts come before the first `---`. Slides are separated by `---`. Each element is a directive comment followed by its markdown body, which runs until the next directive, `---` or end of file.

## Core features (MVP)

Priorities: **Must** = in the first release. **Should** = v1, but the first release can ship without it. **Could** = if time allows.

### Format: parse and write (FMT)
- **FR-FMT-01 [Must]** The engine must parse a deck from a UTF-8 string into a deck model: front matter, masters, layouts, slides, elements, notes.
  *Acceptance:* the sample deck parses into the expected model (slide count, element count, every attribute value), checked by a golden test.
- **FR-FMT-02 [Must]** The engine must serialize a deck model back to a string.
  *Acceptance:* parse then serialize of every deck in the sample suite returns a string identical byte for byte to the input.
- **FR-FMT-03 [Must]** Edits must change only the lines of the elements they touch. Everything else (spacing, comments, unknown attributes, markdown inside untouched elements, line endings) must stay as written.
  *Acceptance:* moving one element in a 50-slide deck produces a diff of exactly one changed line.
- **FR-FMT-04 [Must]** Unknown front matter keys, unknown directive attributes and unknown directive kinds must be kept on save, in place.
  *Acceptance:* a deck with `foo=bar` on an element and an unknown `<!-- widget ... -->` directive round-trips unchanged, and survives a move of that element.
- **FR-FMT-05 [Must]** The deck must declare its format version (`s-md-slides: 1`). A missing version means 1. A version newer than the engine supports opens read-only with a diagnostic.
  *Acceptance:* a `s-md-slides: 2` deck loads, `is_read_only()` is true, and edit operations return an error.
- **FR-FMT-06 [Must]** Malformed input must never fail the whole parse. The engine returns a best-effort deck plus diagnostics, each with severity, message, line and column.
  *Acceptance:* a deck with a bad attribute (`x=abc`) on slide 3 loads all slides, and returns one error diagnostic pointing at that line and column.
- **FR-FMT-07 [Must]** Every element has an `id` that is unique within the deck. The engine writes a short id when it creates an element, and assigns ids on load to hand-written elements that lack one, written on the next save of that element.
  *Acceptance:* a hand-written element without `id` gets one in memory. Saving after editing it writes the id. Saving without editing writes nothing new.
- **FR-FMT-08 [Should]** A plain markdown file with no directives must open as a deck: each `---`-separated section becomes a slide with one text element filling the safe area.
  *Acceptance:* a three-section note opens as three slides, each with its text visible.

### Canvas (CANVAS)
- **FR-CANVAS-01 [Must]** The deck sets its canvas in front matter: `16:9` (1920 x 1080, default), `4:5` (1080 x 1350), or a custom `WxH` in units.
  *Acceptance:* each of the three forms parses to the right width and height. A missing value gives 1920 x 1080.
- **FR-CANVAS-02 [Must]** Element positions and sizes are in canvas units. They are kept as written, even when they fall outside the canvas (which gives a warning).
  *Acceptance:* an element at x=-200 round-trips unchanged and produces one warning.

### Elements (EL)
Common attributes on every element: `id`, `x`, `y`, `w`, `h`, `rot` (degrees), `opacity`, `locked`, `name`. Stacking order is file order: later elements draw on top.
- **FR-EL-01 [Must]** Text element: markdown body (headings, emphasis, lists, links, inline code), with style attributes for font, size, color, weight, alignment, vertical alignment, line height and padding.
  *Acceptance:* a text element with `size=48 align=center color=#fff` parses those values, and the HTML export renders them.
- **FR-EL-02 [Must]** Image element: one markdown image (path and alt text), with `fit=contain|cover|fill` and an optional crop.
  *Acceptance:* the image path, alt text and fit parse. A missing alt text gives a warning.
- **FR-EL-03 [Must]** Shape element: `kind=rect|ellipse|line|arrow`, with fill, stroke, stroke width and corner radius, and optional text inside rect and ellipse.
  *Acceptance:* each kind parses and exports to SVG with the right geometry.
- **FR-EL-04 [Must]** Code element: a fenced code block with a language tag, shown in the theme's code font and code colors, with an optional line-highlight list. The language tag is kept so hosts can color it.
  *Acceptance:* a `rust` block exports in the code font on the code background, with highlighted lines marked, and its language tag round-trips.
- **FR-EL-05 [Must]** Table element: a markdown table, with header and cell styles from the theme.
  *Acceptance:* a 3 x 4 GFM table exports as a table with the right cells and alignment.
- **FR-EL-06 [Should]** Diagram element: a Mermaid source block. The engine stores the source and the host or native export renders it.
  *Acceptance:* the source round-trips. Native export produces an image of the diagram. The npm API returns the source for the host to render.
- **FR-EL-07 [Should]** Math element: a LaTeX block, rendered the same way as diagrams. Inline `$...$` math inside text elements is also kept.
  *Acceptance:* same as FR-EL-06.
- **FR-EL-08 [Could]** Video element: a relative path to a video file, with poster frame, autoplay, loop and muted flags. Plays in the HTML presentation. PDF, PNG, JPEG and GIF use the poster frame.
  *Acceptance:* a video plays in HTML export. The PNG export shows the poster.
- **FR-EL-09 [Could]** Embed element: a web URL (YouTube, Vimeo, or a page), shown as an iframe in the HTML presentation and as its poster image elsewhere. The engine never fetches it.
  *Acceptance:* a YouTube embed plays in HTML export. No network request is made by `parse`, `serialize` or native export.
- **FR-EL-11 [Could]** Syntax highlighting of code elements in native exports (token colors from the theme). Not part of the first release.
  *Acceptance:* a `rust` block exports with token colors from the theme.
- **FR-EL-10 [Should]** Group: a `group` directive with `items=` (ids) that hosts move and resize as one.
  *Acceptance:* `move_element` on a group moves every member, and writes only the members' lines.

### Theme, masters and layouts (STYLE)
- **FR-STYLE-01 [Must]** A theme sets colors (named roles: background, text, accent 1 to 6, code), fonts (heading, body, code) and the default slide background. It sits inline in front matter or in a JSON file referenced by path.
  *Acceptance:* a text element with `color=accent1` resolves to the theme's accent 1. Changing the theme file changes the export.
- **FR-STYLE-02 [Must]** Element style values can be literal (`#e11`, `Inter`) or theme roles (`accent1`, `heading`).
  *Acceptance:* both forms parse and round-trip exactly as written.
- **FR-STYLE-03 [Should]** Masters: named sets of elements (logo, footer, slide number) drawn behind every slide that uses them. Text in masters can contain `{slide}`, `{total}` and `{title}`.
  *Acceptance:* a master footer with `{slide} / {total}` exports as "3 / 12" on slide 3 of 12.
- **FR-STYLE-04 [Should]** Layouts: named placeholder positions on a master. A new slide created from a layout gets one element per placeholder. A slide can turn off its master (`master=none`) or hide single master elements.
  *Acceptance:* `add_slide(layout="title-body")` creates two text elements at the placeholder positions. `master=none` drops the logo from that slide's export.
- **FR-STYLE-05 [Must]** Slide background: color, gradient or image, per slide, overriding the theme.
  *Acceptance:* `bg=#0b1020` and `bg=assets/hero.jpg` both export correctly.

### Animation (ANIM)
- **FR-ANIM-01 [Must]** Per-element entrance and exit effects: `appear`, `fade`, `fly-left|right|up|down`, `scale`. Each has an order, a delay and a duration (milliseconds).
  *Acceptance:* `enter=fly-left delay=300 dur=500` parses, round-trips and plays in the HTML presentation.
- **FR-ANIM-02 [Must]** Build steps: elements with the same order appear together on one click. Elements without an order are visible from the start.
  *Acceptance:* a slide with orders 1, 1, 2 takes two clicks to finish in the HTML presentation.
- **FR-ANIM-03 [Must]** Slide transitions: `none`, `fade`, `push`, with a duration.
  *Acceptance:* `transition=push` plays between slides in the HTML presentation.
- **FR-ANIM-04 [Must]** The engine produces an animation timeline for a slide (each element's state at any time in milliseconds), so every host and the GIF export play the same thing.
  *Acceptance:* `timeline(slide).state_at(t)` returns matching opacity and offset values in the Rust and npm builds for the same deck.

### Direction (DIR)
- **FR-DIR-01 [Must]** Deck default direction in front matter (`dir: auto|ltr|rtl`, default auto) and a per-element override. `auto` resolves from the first strong character.
  *Acceptance:* an Arabic text box with no `dir` resolves to rtl. An English one resolves to ltr. `dir=ltr` forces ltr.
- **FR-DIR-02 [Must]** In an rtl deck, the HTML presentation mirrors its controls and arrow keys. Element positions are never mirrored.
  *Acceptance:* in an rtl deck, Left Arrow goes to the next slide, and every element keeps its x.

### Notes (NOTES)
- **FR-NOTES-01 [Must]** Each slide can carry speaker notes in markdown after a `<!-- notes -->` directive.
  *Acceptance:* notes parse, round-trip, appear in the HTML presenter view and never appear on the slide.

### Assets (ASSET)
- **FR-ASSET-01 [Must]** Images, videos and theme files are referenced by paths relative to the deck file. The engine never reads files. The host passes asset bytes to export through a resolver callback.
  *Acceptance:* export asks the resolver for `assets/chart.png` exactly once, and works in Obsidian with a vault-backed resolver.
- **FR-ASSET-02 [Must]** Export can embed all assets into the output (single-file HTML with data URIs). The source deck always keeps relative paths.
  *Acceptance:* `export_html(embed=true)` produces one file that works offline with the deck's folder deleted.
- **FR-ASSET-03 [Must]** The engine lists every asset a deck references, with any missing ones flagged by the host's resolver.
  *Acceptance:* `assets()` returns every path once. A missing file gives a warning diagnostic on export.

### Edit operations (OPS)
- **FR-OPS-01 [Must]** Every change goes through a named, serializable operation: add, remove, duplicate and reorder slides; add, remove, duplicate, move, resize, rotate and restyle elements; set text; set attributes; change z-order; set notes; set slide background, layout and transition; set front matter values.
  *Acceptance:* each operation round-trips through JSON and gives the same result in Rust and in the npm build.
- **FR-OPS-02 [Must]** Applying an operation returns its inverse operation, so hosts get undo and redo for free.
  *Acceptance:* for every operation type, apply then apply-inverse returns a byte-identical file.
- **FR-OPS-03 [Must]** Operations validate before they change anything. An invalid one (unknown id, negative width) returns an error and leaves the deck unchanged.
  *Acceptance:* `move_element("nope", ...)` returns an error and the serialized deck is unchanged.
- **FR-OPS-04 [Must]** A batch of operations applies as one step, with one combined inverse.
  *Acceptance:* a batch of 20 moves undoes with one inverse.
- **FR-OPS-05 [Should]** Query helpers: find an element by id, list elements at a point (top first), and search text across slides and notes.
  *Acceptance:* `elements_at(x, y)` on overlapping shapes returns them top first.

### Export (EXP)
- **FR-EXP-01 [Must]** HTML and SVG per slide, at a chosen frame of its animation (start, end, or a time).
  *Acceptance:* the SVG of slide 1 at its end state shows all elements in the right places.
- **FR-EXP-02 [Must]** A standalone HTML presentation: next and previous controls, keyboard (arrows, space, Home, End, F for fullscreen), touch swipe, slide counter, presenter view with notes, next slide and timer, animations and transitions, `prefers-reduced-motion` respected.
  *Acceptance:* the sample deck exports to one HTML file that presents fully in Chrome, Safari and Firefox, offline when assets are embedded.
- **FR-EXP-03 [Must]** PDF: one page per slide at its end state, text kept as selectable text. Native build only. (In Obsidian, the plugin prints the HTML export to PDF.)
  *Acceptance:* a 12-slide deck gives a 12-page PDF whose text can be selected and searched.
- **FR-EXP-04 [Must]** PNG and JPEG of a single slide, at a chosen scale (1x, 2x) and JPEG quality. Native build only. (In Obsidian, from the SVG through the browser canvas.)
  *Acceptance:* slide 3 at 2x gives a 3840 x 2160 PNG.
- **FR-EXP-05 [Should]** GIF of a single animated slide: its full timeline at a chosen frame rate (default 15 fps), looping optional. Slides without animation export as a one-frame GIF with a warning. Native build only.
  *Acceptance:* a slide with three build steps gives a GIF that plays all three in order.
- **FR-EXP-06 [Must]** Exports report progress for long jobs (PDF, GIF) and can be cancelled.
  *Acceptance:* a GIF export emits progress events from 0 to 100 and stops within 200 ms of cancel.
- **FR-EXP-07 [Must]** Raw HTML inside markdown is sanitized in every export: no `<script>`, no `on*` handlers, no `javascript:` URLs. iframes are allowed only through the embed element.
  *Acceptance:* a deck containing `<script>` and `<img onerror>` exports with both removed, plus a warning.

### CLI (CLI)
- **FR-CLI-01 [Must]** `s-md-slides validate <deck>` prints diagnostics and exits non-zero on errors.
  *Acceptance:* a broken deck exits 1 with `file:line:col` messages.
- **FR-CLI-02 [Must]** `s-md-slides export <deck> --to html|pdf|png|jpeg|gif [--slide N] [--scale 2] [--embed]`.
  *Acceptance:* each format writes the expected file. `--slide` limits image formats to one slide.
- **FR-CLI-03 [Must]** `s-md-slides serve <deck>` serves the HTML presentation on 127.0.0.1, watches the deck and its assets, and reloads open browsers on change, keeping the current slide. Preview only, it never writes the deck. `--host` and `--port` override.
  *Acceptance:* editing the deck in Vim updates the open browser on the same slide within 500 ms. The deck's modified time never changes because of the server.
- **FR-CLI-04 [Should]** `s-md-slides apply <deck> <ops.json>` applies a batch of operations and writes the deck (for scripts and agents).
  *Acceptance:* a JSON batch of moves changes only those lines.

### Agents and docs (AGENT)
- **FR-AGENT-01 [Must]** The repo ships `AGENTS.md` and a Claude skill (`skills/s-md-slides/SKILL.md`) covering the format, the canvas presets, the operation list, the CLI and how the desktop app's MCP tools map to operations.
  *Acceptance:* a fresh Claude Code session given only the skill writes a five-slide deck that `validate` passes with no errors.
- **FR-AGENT-02 [Must]** A written format spec (`docs/format.md`) with the full grammar and one example per element type, plus a sample deck suite used by the tests.
  *Acceptance:* every example in the spec is in the test suite and parses with no diagnostics.

### Packaging (PKG)
- **FR-PKG-01 [Must]** The engine repo publishes three crates to crates.io: `s-md-slides` (core), `s-md-slides-render` (native export) and `s-md-slides-cli` (the `s-md-slides` binary), all on one version number.
  *Acceptance:* `cargo add s-md-slides` runs the 10-line README example, and `cargo install s-md-slides-cli` gives a working `s-md-slides` command.
- **FR-PKG-02 [Must]** The core crate builds for `wasm32-unknown-unknown` with no file system, network or threads, so the npm repo can wrap it. Engine CI builds it on every push and reports its size.
  *Acceptance:* the CI job builds the core to WASM and fails above 1.5 MB.
- **FR-PKG-03 [Must]** Conformance vectors: the engine repo ships a `conformance/` folder of JSON cases (input deck, operations, expected output text, expected model, expected diagnostics, timeline samples), versioned with the format and released with each engine version. Any implementation (the npm WASM wrapper or a TypeScript port) proves it matches by passing all of them.
  *Acceptance:* the vectors cover every FR-FMT, FR-OPS and FR-ANIM-04 acceptance case, and the native core passes 100% of them in CI.

## Standard states
- Undo and redo: FR-OPS-02, FR-OPS-04 (hosts own the stack, the engine supplies inverses).
- Empty states: an empty string parses as a deck with no slides and the default canvas, with no errors (part of FR-FMT-06).
- Loading and progress: FR-EXP-06.
- Error states and messages: FR-FMT-06, FR-OPS-03.
- Confirm before destructive actions: out of scope for the engine, a host UI concern. Every destructive operation is undoable (FR-OPS-02).
- Autosave or save-and-reopen: out of scope. Hosts own files. Round-trip covered by FR-FMT-02.
- Search and filter: FR-OPS-05.
- Keyboard shortcuts: FR-EXP-02 (presentation keys). Editor shortcuts belong to hosts.
- Settings that persist: deck-level settings live in front matter (FR-CANVAS-01, FR-DIR-01, FR-STYLE-01). App settings belong to hosts.
- First run and onboarding: FR-AGENT-02 (sample deck), FR-PKG-01 (README example).
- Offline: the engine is fully offline. Only embeds in the HTML export reach the network, in the viewer's browser (FR-EL-09).
- Permissions per role: out of scope, no roles.
- Notifications: out of scope, a library has none.
- Import and export: FR-EXP-01 to 07. Import from PowerPoint, Keynote or Marp: out of scope.
- Delete and restore: inverse operations (FR-OPS-02).
- Audit log: out of scope. git is the history.
- Multi-language and RTL: FR-DIR-01, FR-DIR-02.
- Accessibility: FR-EXP-02, FR-EL-02 alt warnings, NFR-07.
- Mobile layout: the HTML presentation scales the canvas to any screen and supports swipe (FR-EXP-02).
- Rate limits and quotas: out of scope, no services.

## Account and auth
None. A library and a local CLI, single user, no network services.

## Data the product handles
- **Deck** : the parsed file: front matter, masters, layouts, slides, plus the original text for lossless writing. In memory only, owned by the host.
- **Slide** : background, layout, master settings, transition, elements, notes.
- **Element** : kind, id, geometry, style, animation, direction, markdown body, unknown attributes.
- **Master and layout** : named element sets and placeholders, stored in the deck before the first slide.
- **Theme** : colors, fonts, background. Inline or a JSON file next to the deck.
- **Asset reference** : a relative path inside the deck. The bytes stay with the host and only pass through export.
- **Operation** : a JSON-serializable edit and its inverse. Never stored by the engine.
- **Diagnostic** : severity, message, line, column, code.
Nothing is stored by the engine itself. The CLI writes only export files, plus the deck for `apply`.

## Integrations
- **Host file systems** : none directly. The host passes text in and out and resolves asset bytes (FR-ASSET-01).
- **Obsidian's renderers** : the plugin renders Mermaid and math with Obsidian's built-in Mermaid and MathJax. The engine hands over the source (FR-EL-06, FR-EL-07).
- **Desktop app MCP server** : wraps the operations (FR-OPS-01) as tools. Specified in the app spec.
- **npm package (project 3)** : wraps the core crate as WebAssembly, or ports it to TypeScript, and passes the conformance vectors (FR-PKG-03).
- **Browser (serve)** : local HTTP on 127.0.0.1 with live reload (FR-CLI-03).

## Non-functional requirements
- **NFR-01 Performance:** parse plus serialize of a 200-slide, 2,000-element deck in under 50 ms native on an M1, and under 150 ms in the npm build inside Obsidian desktop. One operation plus serialize in under 5 ms. PNG of one 1920 x 1080 slide in under 300 ms native. `serve` reload under 500 ms after save.
- **NFR-02 Size:** the core's WASM build under 1 MB uncompressed (hard ceiling 1.5 MB), measured in engine CI (FR-PKG-02). The npm package must stay under 4 MB in total (npm spec), and the Obsidian plugin under 4 MB.
- **NFR-03 Round-trip:** unchanged save is byte-identical. An operation touches only the lines of the elements it changes (FR-FMT-02, FR-FMT-03), checked on every deck in the sample suite.
- **NFR-04 Privacy and security:** no network in the engine core. `serve` binds 127.0.0.1 by default, serves only the deck and files under its folder (no path traversal), and never writes. Exports sanitize raw HTML (FR-EXP-07).
- **NFR-05 Robustness:** no panics on any input, proven by fuzzing the parser and operation applier for at least 10 million cases in CI before each release.
- **NFR-06 Platforms:** Rust MSRV latest stable minus two on macOS, Windows and Linux, plus the `wasm32-unknown-unknown` target for the core. JavaScript environments (Obsidian desktop and mobile, browsers, Node 20+) are specified in the npm package spec.
- **NFR-07 Accessibility and locale:** HTML presentation meets WCAG 2.2 AA (keyboard, contrast of controls, alt text, reading order, reduced motion). Full Unicode, LTR and RTL.
- **NFR-08 Stability:** semantic versioning for the API. The format version changes only for breaking format changes. Decks written by a newer minor engine version load in older ones without losing data (FR-FMT-04).
- **NFR-09 License:** MIT. No GPL or AGPL dependencies, checked in CI.
- **NFR-10 Docs:** docs.rs for every public item, TypeScript types for every export, the format spec and the agent skill kept in sync with the code (FR-AGENT-01, FR-AGENT-02).

## Out of scope for MVP
- **PowerPoint, Keynote, Google Slides or Marp import and export** : v2 at the earliest. `.pptx` export is the most likely to return.
- **Motion paths, emphasis effects (pulse, spin) and morph** : v2 if users ask.
- **Audio, narration, recorded timings** : never in the engine. A host could add them.
- **Real-time collaboration or sync** : never. Files and git.
- **Charts from data** : v2, possibly by reusing S-Charts' approach.
- **Custom element types through a plugin API** : v2. Unknown directives already round-trip (FR-FMT-04).
- **Video and embeds in GIF, PDF and images** : poster frame only, never live frames.
- **An editor UI** : the apps' job.
- **npm packaging and the Obsidian plugin** : their own repos and specs (projects 3 and 4). This engine supplies the core crate and the conformance vectors they build on.

---

### Open questions
None.

### Assumptions confirmed
1. **Syntax : RESOLVED.** Element directives are HTML comments above a markdown body. Marp uses comments for directives too, every markdown parser passes them through, and GitHub, VS Code and Obsidian reading view hide them, so the deck reads as a clean document anywhere. Chosen by Claude because Safi asked for best practice.
2. **Elements : RESOLVED.** Text, image, shapes, code, tables, Mermaid diagrams, math, video and embeds are all in v1.
3. **Masters : RESOLVED.** Full masters with inherited elements, plus layouts with placeholders.
4. **Assets : RESOLVED.** Relative paths in the deck, with an embed-all option at export.
5. **Element ids : RESOLVED.** Every element carries a short `id=` in its directive, always (FR-FMT-07).
6. **Priority split : RESOLVED.** Must: text, image, shapes, code, tables, theme, HTML, PDF, PNG, JPEG. Should: masters, layouts, groups, Mermaid, math, GIF. Could: video, embeds.
7. **Raw HTML : RESOLVED.** Allowed in text elements, sanitized on export (FR-EXP-07).
8. **Plain notes as decks : RESOLVED.** Should (FR-FMT-08).
