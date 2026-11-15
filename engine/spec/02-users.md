# Users and usage

Status: signed off 2026-11-15

The engine is a package. Its users are developers who install it and build a host on it (an app, a plugin, a CLI, a site generator). Deck authors use it indirectly through those hosts, and directly when they edit a deck file by hand, which Stage 1 says will happen often.

## Personas

### Primary: "Safi," the host builder
- **Background:** Senior engineer who builds his own tools. Has shipped Rust/GPUI specs (mdk), TypeScript Obsidian plugins (S-Calc, S-Charts) and npm packages (SafiCSS, safi-studio-scanner). Plans two hosts on this engine: a GPUI desktop app and an Obsidian plugin.
- **What drives them:** wants to write the format logic once and spend his time on UI. Wants files he owns in git, and portfolio work he can show.
- **Current workaround:** every markdown tool he builds re-implements its file format per app. mdk had to port MD Kanban's parser and writer from TypeScript to Rust and prove byte compatibility with golden files. That's the cost this engine removes for slides.
- **Tech comfort:** high in Rust and TypeScript. Comfortable with `cargo`, `npm`, wasm-pack and CI. Expects typed APIs and good errors, and won't read a 40-page spec to load a file.
- **Relationship to plain text:** the `.md` file is the source of truth, and apps are views onto it.
- **Devices and environment:** macOS (Apple Silicon) for development. Hosts must run on macOS, Windows, Linux, Obsidian desktop and Obsidian mobile (iOS, Android).

### Secondary: "Lina," the outside developer
- **Background:** a developer Safi has never met who finds the crate or npm package. Examples: someone adding slide decks to a docs site, a note app, a static site generator, or an AI tool that writes presentations.
- **What drives them:** needs a free-canvas slide format without designing one. MIT matters to her.
- **Current workaround:** Marp for rendering markdown slides (no free positioning), or inventing a JSON format nobody else reads.
- **Tech comfort:** works in either Rust or JS, rarely both. Reads the README and the API docs, copies the first example.
- **Why she matters:** outside users test that the format is documented well enough to stand without Safi's apps. **Why she isn't the beachhead:** scope follows Safi's two hosts. Lina gets a stable, documented API, not features built for her.

### Indirect: "Omar," the deck author
- **Background:** writes decks in the desktop app or Obsidian, and sometimes fixes text in Vim, Obsidian source mode or a git web UI. Might be presenting to a GCC client in Arabic, or making a 4:5 Instagram carousel.
- **What he needs from the engine:** to find and edit the words without wading through coordinates, and for a save from either app to leave his hand edits alone.
- **Tech comfort:** varies. Some authors never open the raw file. The format must not punish the ones who do.

### Not the target
- Designers who want Keynote or Canva-level effects (3D, morph, motion paths). Out in Stage 1.
- Teams that need `.pptx` fidelity or real-time co-editing.
- Marp users expecting their decks to open. Own syntax, by decision.
We won't add PowerPoint compatibility layers or a sync server to win any of these.

## Roles and permissions
None. It's a single library with no accounts. File access is the host's job: the engine reads and writes strings and bytes, and never opens files or the network itself (so it runs in a sandboxed webview).

## Jobs to be done
- When **Safi starts a new host**, he wants to **add one dependency and load, edit and save a deck in a few lines**, so he can **spend his time on the UI instead of parsing**.
- When **a user drags a box in a host**, the host wants to **call one function (move this element to x, y) and save**, so that **only that element's attribute line changes in the file and the git diff is one line**.
- When **a deck was hand-edited with a mistake**, the host wants **a clear error with line and column plus a best-effort deck**, so the app **still opens the file and shows where the problem is**.
- When **someone adds attributes the engine doesn't know** (a newer engine version, another app, an AI agent), they want **them kept on save**, so **no host ever damages another host's data**.
- When **a deck needs to leave the app**, the host wants **a ready HTML presentation, plus HTML or SVG per slide**, so **every host exports the same way without its own renderer**. The native build also wants **PDF, PNG, JPEG and GIF** directly.
- When **Omar fixes a typo by hand**, he wants **the text to look like normal markdown**, so he **can find it and change it without breaking layout**.
- When **Omar writes an Arabic slide**, he wants **that text box to flow right-to-left** while the rest of the deck stays left-to-right.
- When **an AI agent (Claude Code, or Claude through the desktop app's MCP server) is asked to make or change a deck**, it wants **a skill file that explains the format and the edit operations**, so it **writes valid decks the first time and edits through the same operations the apps use**.
- When **Omar wants to present or check a deck outside the apps**, he wants **to run `s-md-slides serve deck.md` and open a browser**, so he **sees the presentation update live as he edits the file in any editor**. The server previews only. It never writes the file.
- When **Lina evaluates the package**, she wants **a format spec, a sample deck and a 10-line example**, so she **can decide in under 30 minutes**.

## Primary user journey
1. **Discovery:** Safi's own need first. For outsiders: crates.io, npm, the GitHub README, and Safi's posts ("PowerPoint, but the file is markdown").
2. **Setup:** `cargo add s-md-slides` or `npm i s-md-slides`. In JS, one async `init()` call loads the WebAssembly. Under 5 minutes.
3. **First use:** parse the bundled sample deck, list its slides and elements, print them.
4. **Aha moment:** call `move_element`, serialize, and `git diff` shows exactly one changed line. Then open the same file in the other host and it looks the same.
5. **Optional power-up:** run `s-md-slides serve` for a live preview, point an agent at the bundled skill, or export a standalone HTML presentation, or add custom attributes for host-specific data and watch them survive a save.
6. **Finish:** the host ships with the engine as its file layer.
7. **Ongoing use:** upgrade engine versions without the format breaking. Decks record the format version they were written with.

## Core loop
**The repeated action:** read file text -> `parse` into a deck model -> apply edits through the API -> `serialize` -> host writes the file. Alongside it: `export` when the deck leaves the app.

**What pulls users back:**
- Each new host Safi builds reuses it at zero format cost.
- Edits from any host keep diffs small and keep other hosts' data.
- Export is already done and matches across hosts.

## Language, accessibility and locale
- **API and docs:** English. Rust docs on docs.rs, TypeScript types and a README for npm.
- **Deck content:** any Unicode text, in left-to-right and right-to-left languages. Direction works at two levels: a deck default in front matter (`dir: auto | ltr | rtl`, auto by default) and an override per text element. `auto` resolves from the first strong character. Right-to-left covers Arabic and Hebrew, with mixed Arabic and English in one box allowed. A deck with `dir: rtl` also mirrors the HTML presentation's controls (next is on the left, and the arrow keys follow reading direction). Element positions are never mirrored automatically, because the author placed them on purpose.
- **Fonts:** a deck names font families per element. The WebAssembly build bundles no fonts. The native export bundles a default Latin and Arabic pair so exports from machines without those fonts still work. Which pair is decided in stage 04 (Noto Sans and Noto Sans Arabic are candidates, OFL).
- **Accessibility of exports:** HTML export keeps heading structure and reading order (element order in the file), uses image alt text from the markdown, supports keyboard control (arrows, space, Home, End) and turns animations off under `prefers-reduced-motion`. The engine warns when an image has no alt text. Target WCAG 2.2 AA for the exported HTML presentation.
- **Units:** canvas units only (16:9 is 1920 x 1080). No dates, currencies or locale formats in the engine.

---

### Open questions
None.

### Assumptions confirmed
1. **Primary user : RESOLVED.** The engine is a package first. Its primary users are developers building hosts, Safi first. Apps come after.
2. **Hand edits : RESOLVED.** Deck authors edit files by hand often (Stage 1), so they're an indirect persona with real requirements.
3. **RTL and LTR : RESOLVED.** Both directions supported: a deck-level default plus a per-element override, auto by default. RTL decks mirror the HTML presenter controls, not element positions.
4. **AI agents : RESOLVED.** The engine ships `AGENTS.md` plus a Claude skill (`skills/s-md-slides/SKILL.md`) that teaches an agent the file format and the edit operations. The desktop app adds an MCP server (app spec), and its tools are thin wrappers over the engine's operations, so the skill covers both editing the file directly and editing through MCP.
5. **Edit operations as data : RESOLVED.** Every edit is a named, serializable operation (for example `move_element`, `set_text`, `add_slide`). Hosts use them for undo and redo, the app's MCP server exposes them as tools, and the CLI can apply them.
6. **CLI : RESOLVED.** A native `s-md-slides` command ships with the crate: `validate`, `export` (PDF, HTML, PNG, JPEG, GIF) and `serve`. `serve` runs a local preview server for one deck file, watches it and live-reloads the browser as the file changes from any editor. Preview only, never writes the file. It listens on 127.0.0.1 only, unless a flag says otherwise.
7. **Supported environments : RESOLVED.** Rust MSRV latest stable minus two. JS: Obsidian desktop (Electron), Obsidian mobile webviews (iOS 16+, Android 10+), current Chrome, Safari and Firefox, Node 20+.
