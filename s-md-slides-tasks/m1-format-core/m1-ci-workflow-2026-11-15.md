---
id: "m1-ci-workflow-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:54:59.000Z"
labels: ["engine", "m1"]
order: 7
---

# CI workflow

GitHub Actions: fmt, clippy, tests on macOS, Windows and Linux, plus the WASM size gate. FR-PKG-02.

## Done when

- [x] CI green on all three OSes
- [x] WASM job fails above 1.5 MB

## Result

Done 2026-11-15. `.github/workflows/ci.yml`: fmt, clippy, tests on macOS, Windows and Linux at Rust 1.95, and the WASM size gate (572 KB). All green.
