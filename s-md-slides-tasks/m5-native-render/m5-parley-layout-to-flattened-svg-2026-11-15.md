---
id: "m5-parley-layout-to-flattened-svg-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T08:03:03.000Z"
labels: ["engine", "m5"]
order: 1
---

# Parley layout to flattened SVG

Positioned text runs, bundled Noto fonts plus system fonts through fontique.

## Done when

- [x] Line breaks match the browser export

## Result

Done 2026-11-16 in `crates/render`. Checked by `crates/render/tests/render.rs` and reference PNGs in `engine/tests/reference/`.
