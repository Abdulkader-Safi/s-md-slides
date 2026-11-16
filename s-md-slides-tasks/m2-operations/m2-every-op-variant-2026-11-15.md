---
id: "m2-every-op-variant-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T10:09:10.000Z"
labels: ["engine", "m2"]
order: 1
---

# Every Op variant

Validation and inverse for AddSlide, RemoveSlide, DuplicateSlide, MoveSlide, AddElement (structured or raw), RemoveElement, DuplicateElement, SetGeometry, SetAttrs, SetBody, SetZ, SetNotes, SetSlideAttrs, SetFrontMatter, AddDefinition, RemoveDefinition. FR-OPS-01 to FR-OPS-03.

## Done when

- [x] Invalid ops return an error and leave the deck unchanged
- [x] apply plus inverse is byte-identical

## Result

Done 2026-11-15 in `crates/core/src/ops.rs`. Checked by `crates/core/tests/ops.rs`: every op variant, its output, its undo, its errors and its JSON form.
