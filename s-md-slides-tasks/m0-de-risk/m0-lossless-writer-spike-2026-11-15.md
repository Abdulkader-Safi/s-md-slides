---
id: "m0-lossless-writer-spike-2026-11-15"
status: "done"
priority: "critical"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:13:53.000Z"
labels: ["engine", "m0"]
order: 2
---

# Lossless writer spike

Scanner, source map and writer for front matter, separators, text elements and their bodies only. Add `SetGeometry`, `SetBody` and `RemoveElement` with inverses. Run proptest on 1,000 generated decks with random op sequences.

## Done when

- [x] Zero byte differences across all proptest cases

## Result

Done 2026-11-15. 1,000 proptest decks, about 3,400 ops per run, zero byte differences after undo. Details and open questions in `engine/PROGRESS.md`.
