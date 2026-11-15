---
id: "m0-wasm-size-spike-2026-11-15"
status: "todo"
priority: "critical"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T12:00:00.000Z"
labels: ["engine", "m0"]
order: 1
---

# WASM size spike

Build a core crate stub with the scanner stub, serde-saphyr, pulldown-cmark, serde_json and the runtime string for `wasm32-unknown-unknown` with the release profile, then wasm-bindgen-cli and `wasm-opt -Oz`. Measure the size.

## Done when

- [ ] Under 1 MB: pass
- [ ] 1 to 1.5 MB: continue and log it in PROGRESS.md
- [ ] Over 1.5 MB: replace serde-saphyr with a flat front matter scanner and re-measure. Still over: stop and report
