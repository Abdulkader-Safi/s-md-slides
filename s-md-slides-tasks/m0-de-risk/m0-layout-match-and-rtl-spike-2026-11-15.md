---
id: "m0-layout-match-and-rtl-spike-2026-11-15"
status: "in-progress"
priority: "critical"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T09:16:41.000Z"
labels: ["engine", "m0"]
order: 3
---

# Layout match and RTL spike

One slide with English and Arabic paragraphs plus a mixed-direction line. Export it as browser HTML and through parley to flattened SVG, resvg PNG and krilla PDF. Compare line breaks.

## Done when

- [ ] Line breaks match, or differ only at the last word, with the same fonts
- [ ] Arabic is shaped and right to left in the PNG
- [ ] Arabic text is selectable in the PDF
- [ ] If Arabic fails in krilla or resvg: stop and report before M1
