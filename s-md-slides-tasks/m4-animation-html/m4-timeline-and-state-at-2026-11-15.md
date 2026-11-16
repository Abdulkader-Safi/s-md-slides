---
id: "m4-timeline-and-state-at-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:41:13.000Z"
labels: ["engine", "m4"]
order: 1
---

# Timeline and state_at

Steps, durations and `state_at(t)`. FR-ANIM-01, FR-ANIM-02, FR-ANIM-04.

## Done when

- [x] Orders 1, 1, 2 give two steps

## Result

Done 2026-11-16 in `crates/core/src/timeline.rs`. Every entry carries its from and to state; `state_at` blends them with ease-out cubic, and the runtime will use the same numbers. Checked by `crates/core/tests/timeline.rs`.
