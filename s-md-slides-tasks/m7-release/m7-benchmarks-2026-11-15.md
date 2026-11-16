---
id: "m7-benchmarks-2026-11-15"
status: "review"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T09:50:40.000Z"
labels: ["engine", "m7"]
order: 5
---

# Benchmarks

criterion benchmarks for NFR-01.

## Done when

- [x] 200 slides parse plus serialize under 50 ms
- [ ] One op plus serialize under 5 ms

## Result

> [!warning] One op plus save measures 5.0 ms on an M2 Pro, so it will miss 5 ms on an M1. Needs a decision, see PROGRESS.md.

Parse plus save: 4.5 ms (budget 50). PNG: 45 ms (budget 300). Parse sped up from 6.4 ms by moving attributes instead of cloning them and dropping a hash set per element.
