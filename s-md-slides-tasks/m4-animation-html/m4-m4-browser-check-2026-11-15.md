---
id: "m4-m4-browser-check-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T07:38:58.000Z"
labels: ["engine", "m4"]
order: 6
---

# M4 browser check

Present `examples/pitch-16x9.md` and `examples/carousel-4x5.md` from one offline HTML file in each target browser (Chrome, Safari, Firefox).

## Done when

- [x] Both decks present fully in every target browser

## Result

Chrome and Firefox pass every check in `runtime/test/check.mjs` (locally and in CI). Safari 26.6 passes all 21 checks in `runtime/test/safari.mjs` (run by the user on 2026-11-16 after turning on Allow Remote Automation).

> [!done]- Safari
> Passes. Run it with `node runtime/test/safari.mjs` from `engine/` (needs Develop > Allow Remote Automation).
