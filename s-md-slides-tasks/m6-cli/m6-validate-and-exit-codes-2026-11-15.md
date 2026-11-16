---
id: "m6-validate-and-exit-codes-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T08:27:42.000Z"
labels: ["engine", "m6"]
order: 1
---

# validate and exit codes

clap commands, exit codes 0 to 4, `file:line:col` output, `--json`, NO_COLOR. FR-CLI-01.

## Done when

- [x] A broken deck exits 1 with file:line:col messages

## Result

Done 2026-11-16 in `crates/cli`. Checked by `crates/cli/tests/cli.rs`.
