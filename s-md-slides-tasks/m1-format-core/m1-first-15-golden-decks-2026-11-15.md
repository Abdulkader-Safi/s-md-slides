---
id: "m1-first-15-golden-decks-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:46:35.000Z"
labels: ["engine", "m1"]
order: 5
---

# First 15 golden decks

Write 15 golden decks in `tests/decks/` with insta snapshots of the model.

## Done when

- [x] All 15 round-trip byte for byte

## Result

Done 2026-11-15. 15 decks in `engine/tests/decks/`, snapshots in `crates/core/tests/snapshots/`, reviewed by hand. Every deck round-trips byte for byte, and a move plus undo on every element restores the exact bytes.
