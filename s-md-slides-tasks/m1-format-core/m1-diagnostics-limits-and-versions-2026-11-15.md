---
id: "m1-diagnostics-limits-and-versions-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:45:32.000Z"
labels: ["engine", "m1"]
order: 4
---

# Diagnostics and limits

Diagnostic codes with line and column. Limits: 20 MB, 5,000 slides, 2,000 elements per slide, 4 KB attribute values. Read-only for `s-md-slides: 2`. FR-FMT-05, FR-FMT-06.

## Done when

- [x] `x=abc` on slide 3 gives one error at the right line and column
- [x] A version 2 deck is read-only

## Result

Done 2026-11-15 in `crates/core` (`deck.rs`, `attrs.rs`, `frontmatter.rs`, `ops.rs`, `diag.rs`). Checked by `crates/core/tests/acceptance.rs` and the proptest suite in `tests/roundtrip.rs`.
