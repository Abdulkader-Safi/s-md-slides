---
id: "m1-ci-workflow-2026-11-15"
status: "todo"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T12:00:00.000Z"
labels: ["engine", "m1"]
order: 7
---

# CI workflow

GitHub Actions: fmt, clippy, tests on macOS, Windows and Linux, plus the WASM size gate. FR-PKG-02.

## Done when

- [ ] CI green on all three OSes
- [ ] WASM job fails above 1.5 MB
