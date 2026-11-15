---
id: "m0-lossless-writer-spike-2026-11-15"
status: "todo"
priority: "critical"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T12:00:00.000Z"
labels: ["engine", "m0"]
order: 2
---

# Lossless writer spike

Scanner, source map and writer for front matter, separators, text elements and their bodies only. Add `SetGeometry`, `SetBody` and `RemoveElement` with inverses. Run proptest on 1,000 generated decks with random op sequences.

## Done when

- [ ] Zero byte differences across all proptest cases
