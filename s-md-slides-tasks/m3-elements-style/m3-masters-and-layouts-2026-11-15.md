---
id: "m3-masters-and-layouts-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:27:39.000Z"
labels: ["engine", "m3"]
order: 3
---

# Masters and layouts

Masters with `{slide}`, `{total}`, `{title}`. Layouts and placeholders, `AddSlide` from a layout, `master=none`, `hide`. FR-STYLE-03, FR-STYLE-04.

## Done when

- [x] Footer shows 3 / 12 on slide 3 of 12
- [x] `add_slide(layout="title-body")` makes two text elements

## Result

Done 2026-11-16 in `crates/core/src/kinds.rs` and `theme.rs`. Checked by `crates/core/tests/style.rs`.
