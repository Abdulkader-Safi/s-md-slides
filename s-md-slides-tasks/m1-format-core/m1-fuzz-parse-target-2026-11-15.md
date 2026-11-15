---
id: "m1-fuzz-parse-target-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:52:36.000Z"
labels: ["engine", "m1"]
order: 6
---

# fuzz_parse target

cargo-fuzz target over `Deck::parse` and `deck.text()`.

## Done when

- [x] 1 million cases with no panic

## Result

Done 2026-11-15. `engine/fuzz`, target `fuzz_parse`. The first run found a wrong line number for YAML errors with a lone CR (fixed, input kept as a seed). The rerun did 1,000,000 cases in 88 seconds with no panic.
