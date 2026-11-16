---
id: "m4-m4-browser-check-2026-11-15"
status: "review"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T07:03:27.000Z"
labels: ["engine", "m4"]
order: 6
---

# M4 browser check

Present `examples/pitch-16x9.md` and `examples/carousel-4x5.md` from one offline HTML file in each target browser (Chrome, Safari, Firefox).

## Done when

- [ ] Both decks present fully in every target browser

## Result

Chrome and Firefox pass every check in `runtime/test/check.mjs` (locally and in CI). Safari is waiting on its "Allow remote automation" setting.

> [!faq]- Needs you: Safari
> Turn on Safari > Settings > Advanced > "Show features for web developers", then Develop > "Allow Remote Automation". Then the same checks can run in Safari.
