---
id: "m4-assets-and-embed-mode-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T07:03:27.000Z"
labels: ["engine", "m4"]
order: 5
---

# Assets and embed mode

AssetResolver, data-URI embed mode, asset listing with missing warnings, plus video and embed output. FR-ASSET-01 to FR-ASSET-03.

## Done when

- [x] Embedded HTML works offline with the deck folder deleted

## Result

Done 2026-11-16. Checked by `crates/core/tests/export.rs` and `runtime/test/check.mjs` in Chrome and Firefox (also in CI).
