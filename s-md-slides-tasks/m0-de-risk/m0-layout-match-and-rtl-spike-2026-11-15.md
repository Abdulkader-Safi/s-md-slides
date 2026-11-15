---
id: "m0-layout-match-and-rtl-spike-2026-11-15"
status: "done"
priority: "critical"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:34:31.000Z"
labels: ["engine", "m0"]
order: 3
---

# Layout match and RTL spike

One slide with English and Arabic paragraphs plus a mixed-direction line. Export it as browser HTML and through parley to flattened SVG, resvg PNG and krilla PDF. Compare line breaks.

## Done when

- [x] Line breaks match, or differ only at the last word, with the same fonts
- [x] Arabic is shaped and right to left in the PNG
- [x] Arabic text is selectable in the PDF
- [ ] If Arabic fails in krilla or resvg: stop and report before M1

## Result

Approved 2026-11-15. The direct parley-to-krilla text path is the plan for M5.

- [x] Line breaks match Chrome exactly in all three boxes
- [x] Arabic shaped and right to left in the PNG (`engine/spikes/layout/results/native.png` next to `browser.png`)
- [x] Arabic renders correctly in the PDF and is selectable

> [!done]- Decided: PDF text path
> Arabic copy and search from the PDF depends on the viewer. Drawing parley glyphs straight into krilla gives the best text layer in every viewer tested (poppler gets every letter right), while krilla-svg keeps drawing everything else. This changes how text reaches krilla, so it needs a yes before M5. Full table in `engine/PROGRESS.md`.
