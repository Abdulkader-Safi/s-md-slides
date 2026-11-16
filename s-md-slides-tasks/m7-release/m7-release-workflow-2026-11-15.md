---
id: "m7-release-workflow-2026-11-15"
status: "done"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T11:53:58.000Z"
labels: ["engine", "m7"]
order: 7
---

# Release workflow

Publish three crates, CLI binaries and `conformance-<version>.zip`. FR-PKG-01.

## Done when

- [ ] `v0.1.0` publishes cleanly to crates.io

## Result

Changed 2026-11-16 to the user's release flow: `cargo publish` by hand from the Mac (core, render, CLI), then the `v0.1.0` tag runs cargo-dist. That builds five targets, shell and PowerShell installers and checksums, and creates the GitHub release; `conformance.yml` attaches the conformance zip. A local `dist build` for Apple silicon gives a working 19 MB binary with the license, notices and font license beside it.
