# s-md-slides engine

> **Changes:** 2026-11-15 (after sign-off): split into four repos (Rust engine, GPUI app, npm package, Obsidian plugin). The npm package is its own project: a WASM wrapper of the core if the package stays under 4 MB, otherwise a TypeScript port, proven by the engine's conformance vectors. Code syntax highlighting dropped to Could.
> 2026-11-15: name set to s-md-slides (Safi Markdown slides), MIT confirmed, export scope widened to PDF, HTML presenter, PNG, JPEG and GIF, canvas presets added, audience set to anyone.

Status: signed off 2026-11-15

Project 1 of 4, each in its own repo. 1: this engine, Rust crates. 2: the desktop app (Rust + GPUI + gpui-kit, spec in `../../app/spec/`), built on the engine crates. 3: the npm package, built on this engine as WebAssembly or as a TypeScript port. 4: the Obsidian plugin, built on the npm package.

## Mission
Make one plain-text slide format that any app can open, edit visually and save back without damaging it. The deck belongs to the `.md` file, not to the app. The engine is that format written down as code, so a deck made in the desktop app opens unchanged in Obsidian and in any future app Safi builds.

This shapes every later decision:
- The engine has no UI. It parses, validates, edits and writes. Each app draws slides itself.
- It ships as Rust crates. The npm package (project 3) wraps the core as WebAssembly if the package stays under 4 MB, otherwise it ports it to TypeScript. Either way it must pass this engine's conformance vectors, so every host reads and writes files identically.
- Round-trip safety comes before features. If the engine opens a file and saves it with no edits, the bytes must match.

Business model: free and open source under MIT. Anyone can install the crate, the npm package, the desktop app or the Obsidian plugin. It also feeds Safi's content: Rust, GPUI and WASM for the technical audience, and a free slides app for everyone else.

## One-liner
A Rust library, also on npm via WebAssembly, that stores a full free-canvas slide deck (every box placed where you dragged it) in one readable markdown file, so visual slide editors can be built on plain text.

## The problem
People who keep notes and docs in markdown (developers, Obsidian users, technical writers) still make slides in PowerPoint, Keynote or Google Slides. They lose version control, diffing and search, and the deck lives in a binary file or someone else's cloud.

Existing markdown slide tools fix the file problem but drop the visual editing:
- **Marp** is the most used markdown slide tool. You write markdown and it renders slides. There's no visual editor, so layout comes from themes and CSS. (slidemodel.com, deckary.com)
- **Slidev** is Vue-based and code-first, aimed at developer talks. A sample project's `node_modules` is about 400 MB. (gist.github.com/johnloy)
- **presenterm** is a Rust tool that renders slides in the terminal. It's code-first with no free positioning, BSD-2. (terminaltrove.com)
- **Deckset** is a native Mac and iOS app, $35 one-time. Layout is automatic from themes, so you can't drag boxes. (toolradar.com, checked Oct 2026)
- **iA Presenter** is text-first, Mac only and commercial. (gist.github.com/johnloy)
- **Obsidian Advanced Slides** has 842k downloads, but its last update was 2023-02-15. The active fork is Slides Extended. Both are reveal.js and code-first. (community.obsidian.md, obsidianstats)

The flaw they share is that layout is either automatic (themes) or code (CSS/HTML). None of them lets you drag a box and still get clean markdown on disk. None of them publishes its format as a standalone library that other apps can build on.

The gap: an open, documented markdown format for free-canvas slides, with a reference parser and writer usable from Rust and JavaScript.

## The value
After: Safi drags a chart image to the right half of slide 3 in the desktop app and saves. `git diff` shows one changed line, the image block's position. He opens the same file in Obsidian on his laptop and slide 3 looks the same. A teammate fixes a typo in the speaker notes in Vim without touching the layout.

The key difference from Marp, the closest open tool: Marp turns markdown into slides one way only. This engine works both ways. An app changes the slide model and the engine writes it back to markdown, keeping the parts it didn't touch.

Versus Marp, an app built on this engine gets:
- Export from one place: a standalone HTML presentation with next and previous controls, a PDF with one page per slide, and any single slide as PNG or JPEG, or as GIF when it has animations.
- Canvas presets: 16:9 (default), 4:5 portrait (the Instagram carousel size) and any custom width and height.
- Free positioning: x, y, width, height and rotation per element, in fixed canvas units, so it looks the same in every host.
- Lossless round-trip: unknown attributes, comments and formatting survive a save. Apps can extend the format without breaking each other.
- Structured edits through an API (move element, add slide, reorder), so hosts never hand-edit text.
- One implementation in two languages: a Cargo crate and an npm package from the same Rust source.

What it deliberately is not:
- Not the live editor's renderer. Hosts draw slides on screen themselves: GPUI in the desktop app, DOM in Obsidian. The engine does own export (below), because every host needs the same output.
- Not a PowerPoint converter in v1. `.pptx` import and export go to v2 or never.
- Not a collaboration or sync layer. The file system and git handle that.
- Not compatible with Marp or Slidev syntax. Own syntax, by decision.

## Product category
Developer library: a document format plus its SDK. On syntax it competes with Marp's markdown dialect. On visual editing it fills the space that iA Presenter and Deckset cover only on Mac and only commercially. White space: open format, free canvas, used from both Rust and JS.

## Business model and license
Free and open source, MIT, the same as S-Calc, SafiCSS and MD Kanban. Published to crates.io and npm as `s-md-slides` (both names free on 2026-11-15). No paid tier.

## Platforms
- Rust crate: any target `std` supports. The desktop app needs macOS, Windows and Linux.
- npm package: WebAssembly plus TypeScript types. It must run in Obsidian desktop (Electron) and Obsidian mobile (iOS and Android webviews), so no Node-only APIs.
- Size budget: the whole Obsidian plugin must stay under 4 MB, so the engine's `.wasm` gets a much smaller budget (target set in stage 03).

Export and the size budget pull against each other. Making PNG, JPEG, GIF and PDF in Rust needs a text layout engine, a rasterizer and bundled fonts, which can cost several MB of WebAssembly on their own. The likely split, settled in stage 04: the core parses, edits, writes and produces HTML and SVG per slide. Raster and PDF export sit behind a separate native-only Cargo feature. In Obsidian the plugin turns the engine's SVG into PNG, JPEG and GIF with the browser's own canvas, and makes PDF with Electron's print-to-PDF on desktop.

## What success looks like
One year in: the desktop app and the Obsidian plugin both ship on the engine, and a deck moves between them with no visible change. The format spec is public on GitHub with a test-suite of sample decks.

Measurable signals:
- Two first-party hosts (desktop app, Obsidian plugin) on the same engine version.
- 100% byte-identical round-trip on the sample-deck suite.
- npm `.wasm` under the stage 03 size budget.
- At least one third-party project using the crate or npm package (stretch).

## UI and product references
The engine has no UI. Format references, kept in `references/` as they're collected:
- Marp's directives and `---` separators: what to avoid copying, and how readable a deck can stay.
- Obsidian Canvas `.canvas` JSON: an example of a free-canvas format that is not human-friendly. The format to avoid producing.
- presenterm: a Rust markdown slide parser to read for structure (BSD-2, readable as reference).

---

### Open questions
None.

### Assumptions confirmed
1. **Layout model : RESOLVED.** Full free canvas. Every element has a position and size. Mitigation: each element stays a normal markdown block with one short attribute line, so the text reads as markdown.
2. **Syntax : RESOLVED.** Own syntax, not Marp, Slidev or Advanced Slides compatible.
3. **npm build : RESOLVED.** One Rust codebase, WebAssembly for npm, if it fits the Obsidian plugin's 4 MB ceiling. If the size spike in M0 fails, fall back to a TypeScript port tested against the same sample-deck suite.
4. **License : RESOLVED.** MIT for the engine and the app.
5. **Name : RESOLVED.** `s-md-slides`, short for Safi Markdown slides. Free on crates.io and npm as of 2026-11-15.
6. **Export : RESOLVED.** The engine exports a standalone HTML presentation with controls to move between slides, a PDF, and each slide alone as PNG or JPEG, or as GIF when the slide has animations. Where each format is produced (WASM or host) is settled in stage 04.
7. **Canvas : RESOLVED.** 16:9 by default (1920 x 1080 units), 4:5 (1080 x 1350), or a custom width and height set in the deck's front matter.
8. **Audience : RESOLVED.** Open source for anyone. Safi's desktop app and Obsidian plugin are the first two hosts.
9. **Animations : RESOLVED.** Per-element entrance and exit effects (appear, fade, fly in from a side, scale), each with order, delay and duration, plus slide transitions (none, fade, push). Stored in the element's attribute line. No motion paths, emphasis effects or morph in v1.
10. **Engine first : RESOLVED.** The packages ship first and are useful alone. The desktop app and Obsidian plugin are built on them afterwards.
11. **Hand editing : RESOLVED.** People will edit deck files by hand often, so readability of the syntax is a requirement.
12. **RTL and LTR : RESOLVED.** v1 supports left-to-right and right-to-left languages (Arabic, Hebrew), with a deck-level default direction and a per-element override (auto, ltr, rtl; auto by default).
