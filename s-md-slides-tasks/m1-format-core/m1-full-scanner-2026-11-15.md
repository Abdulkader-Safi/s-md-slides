---
id: "m1-full-scanner-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:45:32.000Z"
labels: ["engine", "m1"]
order: 1
---

# Full scanner

Fences, BOM, CRLF, front matter, sections, directives, attributes, bodies, implicit text and notes. FR-FMT-01, FR-FMT-08, FR-NOTES-01.

## Done when

- [x] The sample deck parses into the expected model

## Result

Done 2026-11-15 in `crates/core` (`deck.rs`, `attrs.rs`, `frontmatter.rs`, `ops.rs`, `diag.rs`). Checked by `crates/core/tests/acceptance.rs` and the proptest suite in `tests/roundtrip.rs`.
