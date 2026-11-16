---
id: "m2-op-json-round-trip-2026-11-15"
status: "done"
priority: "high"
assignee: "claude"
dueDate: null
created: "2026-11-15T12:00:00.000Z"
modified: "2026-11-15T10:11:35.000Z"
labels: ["engine", "m2"]
order: 4
---

# Op JSON round-trip

Serde for every op, tagged with `"op"`. ts-rs types.

## Done when

- [x] Every op round-trips through JSON

## Result

Done 2026-11-15. Every op variant round-trips through JSON (`every_op_variant_round_trips_through_json`). TypeScript types live in `engine/bindings/`, generated with `cargo test -p s-md-slides --features ts --lib`; CI fails when they are stale.
