---
id: "m5-png-and-jpeg-2026-11-15"
status: "done"
priority: "medium"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T08:03:03.000Z"
labels: ["engine", "m5"]
order: 2
---

# PNG and JPEG

resvg and tiny-skia, jpeg-encoder, with scale and JPEG quality options. FR-EXP-04.

## Done when

- [x] Slide 3 at 2x is 3840 x 2160
- [x] Reference PNGs within 0.5% pixel difference

## Result

Done 2026-11-16 in `crates/render`. Checked by `crates/render/tests/render.rs` and reference PNGs in `engine/tests/reference/`.
