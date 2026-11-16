---
id: "m7-conformance-vectors-2026-11-15"
status: "done"
priority: "low"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-16T09:35:03.000Z"
labels: ["engine", "m7"]
order: 1
---

# Conformance vectors

JSON cases for every FR-FMT, FR-OPS and FR-ANIM-04 acceptance case. FR-PKG-03.

## Done when

- [x] Native core passes 100% in CI

## Result

Done 2026-11-16: 54 vectors in `engine/conformance/`, covering every FR-FMT, FR-OPS and FR-ANIM-04 check. The core passes all of them (`cargo test -p s-md-slides --test conformance`); regenerate with `UPDATE_CONFORMANCE=1` and review the diff.
