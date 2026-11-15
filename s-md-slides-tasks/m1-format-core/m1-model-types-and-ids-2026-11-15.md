---
id: "m1-model-types-and-ids-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:45:32.000Z"
labels: ["engine", "m1"]
order: 3
---

# Model types and ids

Deck, FrontMatter, Slide and Element, with every kind. Id generation and assignment on load, written on the next save of that element. Canvas presets and direction. FR-FMT-07, FR-CANVAS-01, FR-CANVAS-02, FR-DIR-01.

## Done when

- [x] Canvas forms parse to the right sizes
- [x] Missing ids are assigned in memory and written only on edit

## Result

Done 2026-11-15 in `crates/core` (`deck.rs`, `attrs.rs`, `frontmatter.rs`, `ops.rs`, `diag.rs`). Checked by `crates/core/tests/acceptance.rs` and the proptest suite in `tests/roundtrip.rs`.
