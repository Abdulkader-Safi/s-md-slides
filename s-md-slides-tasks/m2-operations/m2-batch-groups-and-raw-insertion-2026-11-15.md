---
id: "m2-batch-groups-and-raw-insertion-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T10:09:18.000Z"
labels: ["engine", "m2"]
order: 2
---

# Batch and groups

`apply_batch` with one combined inverse. Group and Ungroup. FR-OPS-04, FR-EL-10.

## Done when

- [x] A batch of 20 moves undoes with one inverse
- [x] Moving a group writes only the members' lines

## Result

Done 2026-11-15 in `crates/core/src/ops.rs`. Checked by `crates/core/tests/ops.rs`: every op variant, its output, its undo, its errors and its JSON form.
