---
id: "m7-benchmarks-2026-11-15"
status: "done"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T10:54:42.000Z"
labels: ["engine", "m7"]
order: 5
---

# Benchmarks

criterion benchmarks for NFR-01.

## Done when

- [x] 200 slides parse plus serialize under 50 ms
- [ ] One op plus serialize under 5 ms

## Result

One op plus save: 1.9 ms after the one-line fast path (was 5.0 ms). Every fast edit is checked against a full parse in debug builds.

Parse plus save: 4.5 ms (budget 50). PNG: 45 ms (budget 300). Parse sped up from 6.4 ms by moving attributes instead of cloning them and dropping a hash set per element.
