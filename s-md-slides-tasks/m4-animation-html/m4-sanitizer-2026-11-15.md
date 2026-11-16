---
id: "m4-sanitizer-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:48:03.000Z"
labels: ["engine", "m4"]
order: 3
---

# Sanitizer

Allowlist filter on pulldown-cmark HTML events with warnings. FR-EXP-07.

## Done when

- [x] `<script>` and `<img onerror>` removed with a warning

## Result

Done 2026-11-16 in `crates/core/src/export/`. Checked by `crates/core/tests/export.rs`.
