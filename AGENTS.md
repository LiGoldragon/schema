# Agent instructions — schema

You **MUST** read AGENTS.md at `github:ligoldragon/lore` — the workspace contract.

## Repo role

`schema` is the typed schema-language substrate for Persona signal
contracts. It is a Rust library that models the six-position authored
`.schema` file, validates imports and namespace declarations, lowers
uniform route headers into `AssembledSchema`, and derives fixed-root versus
ordered-box layout metadata for macro consumers.

## Carve-outs worth knowing

- This repository is a library surface right now, not a daemon triad.
  A future schema daemon/working-signal/policy-signal triad can grow from
  this once runtime registry authority is settled.
- Keep schema names as full English words and avoid ancestry-heavy names.
  Inside this crate, `Schema`, `Declaration`, `Variant`, `Header`, and
  `TypeExpression` are enough.
- Keep namespace definitions in `Namespace`, not as comments. `Namespace`
  mirrors NOTA's `{key value ...}` map form and rejects route-root/data-type
  name collisions.
- Header roots use the uniform v13 shape `(Root [SubVariant ...])`; do not
  reintroduce a scalar `(Root Payload)` form.
- The crate does not parse NOTA text yet. Parser work belongs behind the
  same typed model after the document shape is stable.
- The crate does not own signal-frame dispatch, version projection, or
  daemon upgrade orchestration. It supplies schema metadata those systems
  consume.
