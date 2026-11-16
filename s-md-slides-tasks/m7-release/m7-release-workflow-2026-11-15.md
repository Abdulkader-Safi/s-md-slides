---
id: "m7-release-workflow-2026-11-15"
status: "review"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T09:54:29.000Z"
labels: ["engine", "m7"]
order: 7
---

# Release workflow

Publish three crates, CLI binaries and `conformance-<version>.zip`. FR-PKG-01.

## Done when

- [ ] `v0.1.0` publishes cleanly to crates.io

## Result

Workflow written 2026-11-16: a `v0.1.0` tag runs the tests, checks the tag matches the crate version, builds the binary for macOS (Apple silicon and Intel), Linux and Windows with notices, publishes core, render and CLI in that order, and attaches the binaries, `conformance-0.1.0.zip` and the notices to a GitHub release. All three crates package and build from their packages locally, and all three names are free on crates.io.

> [!warning] Publishing cannot be undone. It needs a `CARGO_REGISTRY_TOKEN` secret and the go-ahead before the tag is pushed.
