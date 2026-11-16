---
id: "m2-fuzz-ops-and-proptest-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T06:19:54.000Z"
labels: ["engine", "m2"]
order: 5
---

# fuzz_ops and proptest

fuzz_ops target and proptest over all op types.

## Done when

- [x] 10,000 generated cases byte-identical after apply plus inverse
- [x] Each op touches only its targets' spans

## Result

Done 2026-11-16. Proptest: 10,000 decks, every op type, about 35,000 successful ops, each inside its target and undone exactly. `fuzz_ops`: 1,000,000 runs, no panic.
