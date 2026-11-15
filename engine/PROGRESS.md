# Progress

Build spec: [spec/06-build-spec.md](spec/06-build-spec.md). Task board: `../s-md-slides-tasks/`.

## Milestones

| Milestone                     | Status              |
| ----------------------------- | ------------------- |
| M0 de-risk                    | in progress (PR #1) |
| M1 format core                | not started         |
| M2 operations                 | not started         |
| M3 elements and style         | not started         |
| M4 animation and HTML export  | not started         |
| M5 native render              | not started         |
| M6 CLI                        | not started         |
| M7 conformance, docs, release | not started         |

## Decisions

- 2026-11-15: the Cargo workspace lives in `engine/`, not at the git root. The git repo also holds the task board, and the spec puts the GPUI app, npm package and Obsidian plugin in their own repos later.
- 2026-11-15: MSRV is 1.95 (latest stable 1.97.1 minus two).
- 2026-11-15: one branch and one PR per milestone. Commits after each task.

## M0 results

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
