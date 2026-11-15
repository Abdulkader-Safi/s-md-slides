# Technical requirements

Status: signed off 2026-11-15

The engineering blueprint. Tech context: engine repo, Cargo workspace with three crates. `s-md-slides` is the core (WASM-safe: own lossless scanner, pulldown-cmark, serde-saphyr, serde_json, ts-rs). `s-md-slides-render` is native export (parley, resvg, tiny-skia, krilla, gif, jpeg-encoder, mermaid-rs-renderer, ratex-svg, Noto fonts). `s-md-slides-cli` is the binary (clap, notify, tiny_http). The npm package and the apps are separate repos.

## Architecture

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

The keystone is the **source map**. The deck keeps the original text plus a byte span for every structural piece (front matter keys, each directive line, each attribute token, each body). The model is a view over those spans. Every operation edits the model and records which spans changed. The writer copies the original text and splices in new text only for changed spans, then shifts the offsets after them. That one structure gives byte-identical round-trips, one-line diffs, kept unknown attributes, and line and column for diagnostics (FR-FMT-02 to 06).

## File grammar (format version 1)

Written out in full in `docs/format.md` (FR-AGENT-02). The rules:

1. **Encoding:** UTF-8. A leading BOM is kept. Line endings (LF or CRLF) are detected per file and kept. New lines use the file's majority ending.
2. **Front matter:** optional. Starts with `---` on line 1 and ends at the next `---` line. YAML, read with serde-saphyr.
3. **Separator:** a line that is exactly `---` (trailing spaces allowed), outside front matter and outside fenced code blocks (``` or ~~~). It splits the rest of the file into sections. A horizontal rule inside slide text uses `***` or `___`.
4. **Sections:** a section whose first directive is `master` or `layout` is a definitions section. Every other section is a slide. A slide section may start with `<!-- slide ... -->`.
5. **Directive line:** a whole line `<!-- kind attr attr ... -->`. `kind` matches `[a-z][a-z0-9-]*`. A line counts as a directive only if `kind` is a known kind or it carries an `id=` attribute. Any other comment (`<!-- fix this later -->`) is ordinary text inside the current body.
6. **Known kinds:** `slide`, `master`, `layout`, `placeholder`, `notes`, `text`, `image`, `shape`, `code`, `table`, `diagram`, `math`, `video`, `embed`, `group`.
7. **Attributes:** `key=value` or a bare `key` (true). `key` matches `[a-z][a-z0-9.-]*`. `value` is bare (no spaces or quotes) or double-quoted with `\"` and `\\` escapes. Order is kept. Duplicate keys: the last one wins, with a warning.
8. **Body:** every line after a directive up to the next directive, separator or end of file. Blank lines at the end of a body are kept as trivia between elements.
9. **Implicit text:** markdown in a slide before its first element directive becomes a text element filling the safe area, with an in-memory id (FR-FMT-08).
10. **Notes:** `<!-- notes -->` with a body up to the end of the slide. An element directive after notes gives a warning and is read as an element.
11. **Numbers:** integers when whole, otherwise up to two decimals, no trailing zeros. Positions and sizes in canvas units, `rot` in degrees, `delay` and `dur` in milliseconds.
12. **Colors:** `#rgb`, `#rrggbb`, `#rrggbbaa`, or a theme role (`background`, `text`, `accent1` to `accent6`, `code`). Gradients: `linear(angle,color,color[,color])`.
13. **Ids:** `[a-z0-9-]{1,32}`, unique in the deck. Generated as kind letter plus a counter (`t1`, `i4`, `s12`), skipping ids already in use.

## Data models

Conventions: no database. Everything lives in memory and is owned by the host. Ids are strings (rule 13). Units are canvas units, degrees and milliseconds. `Span` is a byte range in the source text. Every model struct carries the spans it came from, and an `extra` list of unknown attributes in their original order.

### Deck
| Field | Type | Notes |
|-------|------|-------|
| source | String | original text, updated by the writer |
| version | u32 | from `s-md-slides:`, default 1. Above 1 means read-only (FR-FMT-05) |
| front | FrontMatter | |
| masters | Vec<Master> | from definition sections |
| layouts | Vec<Layout> | from definition sections |
| slides | Vec<Slide> | file order |
| line_ending | Lf or Crlf | detected |
| next_ids | map kind -> u32 | id counters |

### FrontMatter
| Field | Type | Notes |
|-------|------|-------|
| title, author | Option<String> | |
| canvas | Canvas | `16:9` (1920x1080, default), `4:5` (1080x1350), or `WxH` |
| dir | Dir | auto (default), ltr, rtl |
| lang | Option<String> | BCP 47, for the HTML `lang` attribute |
| theme | ThemeRef | inline mapping or relative path to JSON |
| transition | Option<Transition> | deck default |
| extra | ordered map | unknown keys, kept |

### Theme
| Field | Type | Notes |
|-------|------|-------|
| colors | map role -> Color | background, text, accent1 to accent6, code, code-bg |
| fonts | heading, body, code | family names |
| background | Background | default slide background |

### Master
| Field | Type | Notes |
|-------|------|-------|
| id | String | |
| elements | Vec<Element> | drawn behind slides that use it |

### Layout
| Field | Type | Notes |
|-------|------|-------|
| id | String | |
| master | Option<String> | master id |
| placeholders | Vec<Placeholder> | id, geometry, kind (default text) |

### Slide
| Field | Type | Notes |
|-------|------|-------|
| index | usize | derived, not stored in the file |
| layout | Option<String> | layout id |
| master | MasterSetting | inherit, `none`, or a master id |
| hide | Vec<String> | master element ids hidden on this slide |
| bg | Option<Background> | color, gradient or image path |
| transition | Option<Transition> | none, fade or push, plus `dur` |
| elements | Vec<Element> | file order = stacking order |
| notes | Option<String> | markdown |
| extra | attrs | unknown slide attributes |

### Element (common)
| Field | Type | Notes |
|-------|------|-------|
| id | String | unique |
| kind | Kind | text, image, shape, code, table, diagram, math, video, embed, group, or Unknown(String) |
| x, y, w, h | f32 | required except `group` (derived) and line or arrow shapes (`h` may be 0) |
| rot | f32 | default 0 |
| opacity | f32 | 0 to 1, default 1 |
| locked | bool | hosts must not move it, the engine still allows it |
| name | Option<String> | label for layers panels |
| dir | Option<Dir> | text-bearing kinds only |
| style | Style | font, size, color, weight, align, valign, line-height, padding, fill, stroke, stroke-width, radius |
| enter, exit | Option<Effect> | appear, fade, fly-left, fly-right, fly-up, fly-down, scale |
| order | Option<u32> | build step |
| delay, dur | u32 | ms, defaults 0 and 400 |
| body | String | raw markdown, never normalized |
| extra | attrs | unknown attributes |

Kind-specific attributes:
| Kind | Attributes | Body |
|------|------------|------|
| text | style only | markdown |
| image | `fit` contain, cover or fill, `crop=x,y,w,h` | one `![alt](path)` |
| shape | `kind` rect, ellipse, line or arrow, `arrow-start`, `arrow-end` | optional text |
| code | `lines=1,3-5` (highlighted lines) | one fenced block with a language |
| table | `header` (bool, default true) | one GFM table |
| diagram | none | one ` ```mermaid ` block |
| math | none | `$$...$$` or a ` ```math ` block |
| video | `src`, `poster`, `autoplay`, `loop`, `muted` | empty |
| embed | `src` (https only), `poster` | empty |
| group | `items=id,id,...` | empty |

### Timeline
| Field | Type | Notes |
|-------|------|-------|
| steps | Vec<Step> | step 0 = visible at start. Each step lists element ids with enter or exit, delay and dur |
| duration | u32 | ms, all steps played back to back |
| `state_at(t)` | per element: opacity, dx, dy, scale | same function for HTML, GIF and hosts |

### AssetRef
| Field | Type | Notes |
|-------|------|-------|
| path | String | relative to the deck file, exactly as written |
| kind | image, video, poster, theme, background | where it is used |
| used_by | Vec<String> | element ids, slide indexes or `front` |

### Diagnostic
| Field | Type | Notes |
|-------|------|-------|
| severity | error, warning, info | |
| code | String | for example `E-ATTR-VALUE`, `E-DUP-ID`, `E-VERSION`, `E-FRONTMATTER`, `E-UNCLOSED-FENCE`, `E-IMAGE-BODY`, `W-OFF-CANVAS`, `W-NO-ALT`, `W-HTML-REMOVED`, `W-NOTES-ORDER`, `W-DUP-ATTR`, `W-MISSING-ASSET` |
| message | String | English, one sentence |
| line, column | u32 | 1-based |
| element, slide | Option | where known |

Relationships: a slide refers to one layout. A layout refers to one master. A group refers to element ids on the same slide. Removing a layout or master that slides use is refused with an error (FR-OPS-03).

## API surface

Style: a plain Rust library API. The same calls are exposed to JavaScript by the npm repo, with JSON at the boundary. Errors are `Result<T, EngineError>`, where `EngineError { code, message, element, slide }` is serializable. The library never panics on input (NFR-05).

### Parse and write (core)
- **`Deck::parse(text: &str) -> Parsed { deck, diagnostics }`** : never fails. Best-effort deck plus diagnostics (FR-FMT-01, FR-FMT-06).
- **`deck.text() -> &str`** : the current file text, after splices (FR-FMT-02, FR-FMT-03).
- **`deck.is_read_only() -> bool`** : true for newer format versions (FR-FMT-05).
- **`deck.diagnostics() -> Vec<Diagnostic>`** : re-checks the current model.

### Operations (core)
- **`deck.apply(op: Op) -> Result<Op, EngineError>`** : validates, applies, returns the inverse (FR-OPS-01 to 03). Read-only decks return `E-READ-ONLY`.
- **`deck.apply_batch(ops: Vec<Op>) -> Result<Op, EngineError>`** : all or nothing. The inverse is one `Batch` (FR-OPS-04).

`Op` variants, all JSON-serializable with a `"op"` tag:
- `AddSlide { index, layout?, raw? }` and `RemoveSlide { index }`, `DuplicateSlide { index }`, `MoveSlide { from, to }`
- `AddElement { slide, index?, kind, attrs, body } | AddElement { slide, index?, raw }`. The `raw` form inserts exact source text, so the inverse of a remove puts back the original bytes.
- `RemoveElement { id }`, `DuplicateElement { id, dx?, dy? }`
- `SetGeometry { id, x?, y?, w?, h?, rot? }` covers move, resize and rotate. On a group it moves members.
- `SetAttrs { id, set: { key: value }, remove: [key] }` for style, animation, dir and any attribute.
- `SetBody { id, text }`
- `SetZ { id, to: index | "front" | "back" | "forward" | "backward" }`
- `SetNotes { slide, text? }`
- `SetSlideAttrs { slide, set, remove }` for bg, layout, master, hide, transition.
- `SetFrontMatter { key, value? }` (null removes the key)
- `AddDefinition { raw }` and `RemoveDefinition { id }` for masters and layouts.
- `Group { ids, id? }` and `Ungroup { id }`
- `Batch { ops }`

### Queries (core)
- `deck.slides()`, `deck.slide(i)`, `deck.element(id)`, `deck.canvas()`
- `deck.resolved_style(id) -> ResolvedStyle` : theme roles, master and layout inheritance applied (FR-STYLE-01 to 04).
- `deck.elements_at(slide, x, y) -> Vec<id>` : top first, rotation aware (FR-OPS-05).
- `deck.search(query) -> Vec<Hit>` : text and notes (FR-OPS-05).
- `deck.assets() -> Vec<AssetRef>` : every referenced path once (FR-ASSET-03).
- `deck.timeline(slide) -> Timeline` (FR-ANIM-04).

### Export (core)
- `export::slide_html(&deck, slide, &Options) -> String` (FR-EXP-01)
- `export::slide_svg(&deck, slide, Frame, &Options) -> String` : browser SVG, text in foreignObject (FR-EXP-01)
- `export::presentation_html(&deck, &Options, &dyn AssetResolver) -> Result<String>` : standalone, with `embed` for data URIs (FR-EXP-02, FR-ASSET-02)
- `trait AssetResolver { fn resolve(&self, path: &str) -> Option<Asset { bytes, mime }> }` : the only way the engine sees files (FR-ASSET-01)
- `Frame` = `Start | End | At(ms)`. `Options` = `{ embed, scale, include_notes, highlight_lines }`

### Export (render, native)
- `render::flatten_svg(&deck, slide, Frame, &dyn AssetResolver, &Fonts) -> Result<String>`
- `render::png(.., scale) -> Result<Vec<u8>>` and `render::jpeg(.., scale, quality)` (FR-EXP-04)
- `render::gif(.., fps, looped, &mut dyn Progress) -> Result<Vec<u8>>` (FR-EXP-05)
- `render::pdf(&deck, &dyn AssetResolver, &Fonts, &mut dyn Progress) -> Result<Vec<u8>>` (FR-EXP-03)
- `trait Progress { fn report(&mut self, done: u32, total: u32) -> Control }`, where `Control` = `Continue | Cancel`. Cancel returns `E-CANCELLED` within one frame or one page (FR-EXP-06).
- `Fonts::bundled()` plus `Fonts::with_system()` and `Fonts::add_dir(path)`

### CLI (`s-md-slides`)
Exit codes: 0 ok, 1 deck has errors, 2 usage error, 3 I/O error, 4 cancelled.
- `validate <deck> [--json]` : diagnostics as `file:line:col: severity code message`, or JSON (FR-CLI-01)
- `export <deck> --to html|pdf|png|jpeg|gif [--slide N] [--scale 1|2] [--quality 90] [--fps 15] [--embed] [-o PATH]` (FR-CLI-02)
- `serve <deck> [--host 127.0.0.1] [--port 4321] [--open]` (FR-CLI-03)
- `apply <deck> <ops.json> [--dry-run]` : prints the inverse batch to stdout so scripts can undo (FR-CLI-04)

## Permissions
None. No accounts or roles. File access belongs to hosts. The CLI acts as the user who runs it.

## Outputs (in place of screens)

### HTML presentation (FR-EXP-02)
- **Purpose:** present a deck in any browser, offline.
- **Parts:** a stage that scales the canvas to fit the window (letterboxed), previous and next buttons, a slide counter, a progress bar, fullscreen. The presenter view opens at `#presenter` in a second window, synced through `BroadcastChannel`: current and next slide, notes, an elapsed timer.
- **States:** loading (a blank stage until the first slide paints), presenting, end (a "End of deck" screen after the last slide), error (a missing asset shows a gray box with its path).
- **Keys:** Right, Down, Space or PageDown for next. Left, Up or PageUp for previous. Home and End. F for fullscreen. P for the presenter view. In an rtl deck, Left and Right swap (FR-DIR-02). Touch swipe follows the same direction.
- **Motion:** under `prefers-reduced-motion`, effects and transitions become instant (NFR-07).

### `serve` pages (FR-CLI-03)
- `GET /` : the presentation, with a reload script added.
- `GET /events` : server-sent events. Sends `reload` with the new slide count. The page keeps its current slide, clamped to the new count.
- `GET /<path>` : files under the deck's folder only. 404 outside it.
- Any other method: 405.

### CLI output
Plain text on stderr for humans, `--json` for tools. Colors unless `NO_COLOR` is set or stderr isn't a terminal.

## State management
- **Source of truth:** `Deck.source` plus the source map. The model is never written out by printing it, only by splicing.
- **Undo and redo:** hosts keep their own stacks of inverse operations. The engine keeps no history.
- **Change notification:** none inside the engine. `apply` returns, and the host re-reads what it needs. Hosts that watch files (the desktop app) call `Deck::parse` again on an outside change.
- **Persistence:** none. Hosts write `deck.text()` to disk.
- **Concurrency:** a `Deck` is `Send` but not shared. One owner mutates it. No internal threads in the core.

## Background and long-running tasks
- **PDF and GIF export:** run on the caller's thread with a `Progress` callback, cancellable per page or frame. The CLI runs them on a worker thread and prints a progress bar.
- **`serve` watcher:** notify plus notify-debouncer-full at 100 ms on the deck file and every asset in `deck.assets()`. On change: re-parse, rebuild the HTML, broadcast `reload`. A parse error shows a diagnostics overlay in the browser instead of the slides.
- **Mermaid and math rendering in export:** cached per source hash for the length of one export.

## Security
- **Untrusted input:** decks may come from anywhere. The parser and `apply` never panic, and are fuzzed (NFR-05). Limits with error diagnostics: file over 20 MB, over 5,000 slides, over 2,000 elements on one slide, any attribute value over 4 KB.
- **HTML:** raw HTML is filtered by the allowlist (FR-EXP-07). Exported presentations carry a Content Security Policy meta tag: `default-src 'self' data: blob:`, scripts limited to the inline runtime by hash, and `frame-src` limited to `https://www.youtube-nocookie.com` and `https://player.vimeo.com` for embeds. Embed `src` must be https, and YouTube links are rewritten to the nocookie domain.
- **Paths:** the CLI's resolver rejects absolute paths and anything that escapes the deck folder after canonicalizing, with a `W-MISSING-ASSET` warning. `serve` applies the same rule.
- **serve:** binds 127.0.0.1 by default. `--host 0.0.0.0` prints a warning that the deck is visible on the network. GET only, no directory listing, and it never writes.
- **No network** in the core or render crates. Fonts come from disk or the bundle only.

## Logging and observability
- **Library:** no logging. Problems surface as diagnostics and errors.
- **CLI:** `RUST_LOG` through `env_logger` for debugging, off by default. Logs carry file paths, lines and timings, never deck text or notes.

## Testing strategy
- **Golden decks** in `tests/decks/`, at least 25: one per element kind, masters and layouts, RTL and mixed text, CRLF, BOM, broken YAML, unknown directives and attributes, ordinary comments, a plain `---` note, a newer version, a 200-slide stress deck. Each must round-trip byte for byte (FR-FMT-02).
- **Snapshots** (`insta`) of the parsed model, diagnostics, slide HTML and flattened SVG.
- **Property tests** (`proptest`): random decks and random op sequences. Check that apply plus inverse returns the same bytes, and that every op changes only the spans of its targets (FR-FMT-03, FR-OPS-02).
- **Fuzzing** (`cargo-fuzz`): `fuzz_parse` and `fuzz_ops`, 10 million runs each, nightly and before release (NFR-05).
- **Conformance vectors** in `conformance/`, one JSON file per case: `{ name, input, ops, expect_text, expect_model, expect_diagnostics, timeline_samples }`. Run by `cargo test -p s-md-slides --test conformance` and published per release (FR-PKG-03).
- **Image references:** PNG output for the sample decks compared to stored references, failing above 0.5% of pixels differing.
- **Performance:** `criterion` benchmarks for the NFR-01 numbers on the 200-slide deck, checked in CI against a 20% regression limit.
- **WASM size gate:** `scripts/wasm-size.sh` builds the core for `wasm32-unknown-unknown` with the release profile and `wasm-opt -Oz`, and fails above 1.5 MB, warning above 1 MB (FR-PKG-02).
- **Agent check:** before each release, a fresh Claude Code session with only the skill writes a five-slide deck, which must pass `validate` (FR-AGENT-01).
- **How to run:** `cargo test --workspace`, `cargo insta review`, `cargo +nightly fuzz run fuzz_parse -- -runs=10000000`, `cargo bench`, `scripts/wasm-size.sh`.

## Environment and configuration
- `S_MD_SLIDES_FONTS` : extra font folder for native export. Default: none.
- `RUST_LOG` : CLI debug logging. Default: off.
- `NO_COLOR` : plain CLI output.
No secrets anywhere. No config files: deck settings live in front matter.

## Migrations and seed data
- **Format versioning:** this spec is format version 1. A future version 2 ships with an `s-md-slides migrate` command, out of scope now (FR-FMT-05).
- **Sample decks** in `examples/`: `hello.md` (three slides), `pitch-16x9.md` (theme, masters, layouts, animations), `carousel-4x5.md` (Arabic and English, rtl deck), `all-elements.md` (one of every kind). They double as golden decks and as the skill's examples.
- **Starter theme:** `themes/default.json`, light, with Noto fonts.

---

### Open questions
None.

### Assumptions confirmed
1. **Separator : RESOLVED.** Only a line that is exactly `---` splits slides. Horizontal rules inside slide text use `***` or `___`.
2. **Directives : RESOLVED.** A comment is a directive only if its kind is known or it carries `id=`. Every other comment is ordinary text.
3. **Presenter view : RESOLVED.** A second window synced with `BroadcastChannel`, opened with P.
