---
id: "m3-themes-and-role-resolution-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:27:39.000Z"
labels: ["engine", "m3"]
order: 2
---

# Themes and role resolution

Inline and JSON themes (through the resolver), roles, `resolved_style`, `themes/default.json`. FR-STYLE-01, FR-STYLE-02.

## Done when

- [x] `color=accent1` resolves to the theme accent

## Result

Done 2026-11-16 in `crates/core/src/kinds.rs` and `theme.rs`. Checked by `crates/core/tests/style.rs`.
