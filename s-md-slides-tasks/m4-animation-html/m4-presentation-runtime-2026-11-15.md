---
id: "m4-presentation-runtime-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T07:03:27.000Z"
labels: ["engine", "m4"]
order: 4
---

# Presentation runtime

TypeScript runtime: controls, keys, swipe, presenter view, build steps, transitions, RTL swap, reduced motion, end screen, CSP. Built with esbuild, committed, CI checks it is current. FR-ANIM-03, FR-DIR-02, FR-EXP-02.

## Done when

- [x] Keyboard-only use works
- [x] axe-core finds no WCAG 2.2 AA violations

## Result

Done 2026-11-16. Checked by `crates/core/tests/export.rs` and `runtime/test/check.mjs` in Chrome and Firefox (also in CI).
