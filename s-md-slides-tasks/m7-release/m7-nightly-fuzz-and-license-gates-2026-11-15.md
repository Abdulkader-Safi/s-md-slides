---
id: "m7-nightly-fuzz-and-license-gates-2026-11-15"
status: "done"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T11:53:58.000Z"
labels: ["engine", "m7"]
order: 6
---

# Nightly fuzz and license gates

10-million-case fuzz workflow, cargo-deny, cargo-about notices. NFR-05, NFR-09.

## Done when

- [ ] Both fuzz targets pass 10 million runs
- [x] No GPL or AGPL

## Result

Done 2026-11-16. cargo-deny passes in CI (no GPL or AGPL), and cargo-about notices ship in every release archive. The nightly workflow runs 10 million release runs per target plus an hour of `fuzz_ops` with debug assertions. Fuzzing found two bugs in the one-line fast path, both fixed with regression tests. The user chose to release 0.1.0 before the local 10-million runs finished (see PROGRESS.md); they keep running.
