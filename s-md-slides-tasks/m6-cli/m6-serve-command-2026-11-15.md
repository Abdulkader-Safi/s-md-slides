---
id: "m6-serve-command-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T08:27:42.000Z"
labels: ["engine", "m6"]
order: 3
---

# serve command

notify, tiny_http, SSE reload keeping the current slide, diagnostics overlay, 127.0.0.1 default. FR-CLI-03.

## Done when

- [x] A Vim edit reaches the browser within 500 ms on the same slide
- [x] The deck's modified time never changes
- [x] `/../secret` returns 404

## Result

Done 2026-11-16 in `crates/cli`. Checked by `crates/cli/tests/cli.rs`.
