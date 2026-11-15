---
id: "m1-source-map-and-writer-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:45:32.000Z"
labels: ["engine", "m1"]
order: 2
---

# Source map and writer

Byte spans for every structural piece, dirty tracking, offset shifting, splice-only writer. FR-FMT-02, FR-FMT-03, FR-FMT-04.

## Done when

- [x] Every golden deck round-trips byte for byte
- [x] Moving one element in a 50-slide deck changes one line

## Result

Done 2026-11-15 in `crates/core` (`deck.rs`, `attrs.rs`, `frontmatter.rs`, `ops.rs`, `diag.rs`). Checked by `crates/core/tests/acceptance.rs` and the proptest suite in `tests/roundtrip.rs`.
