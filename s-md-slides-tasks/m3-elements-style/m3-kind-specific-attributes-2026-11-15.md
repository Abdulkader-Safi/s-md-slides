---
id: "m3-kind-specific-attributes-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:27:39.000Z"
labels: ["engine", "m3"]
order: 1
---

# Kind-specific attributes

Attributes and body validation for text, image, shape, code, table, diagram, math, video, embed. Alt-text warning. FR-EL-01 to FR-EL-09.

## Done when

- [x] Each kind parses its attributes
- [x] Missing alt text gives a warning

## Result

Done 2026-11-16 in `crates/core/src/kinds.rs` and `theme.rs`. Checked by `crates/core/tests/style.rs`.
