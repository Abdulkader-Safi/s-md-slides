---
id: "m5-mermaid-math-and-posters-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T08:03:03.000Z"
labels: ["engine", "m5"]
order: 5
---

# Mermaid and math to SVG, poster frames

mermaid-rs-renderer and ratex-svg to SVG, a code box plus warning on failure. Video and embed poster frames.

## Done when

- [x] Native export shows the diagram and the poster

## Result

Done 2026-11-16 in `crates/render`. Checked by `crates/render/tests/render.rs` and reference PNGs in `engine/tests/reference/`.
