# Agent instructions — schema

You **MUST** read AGENTS.md at `github:ligoldragon/lore` — the workspace contract.

## Repo role

`schema` is the typed schema-language substrate for Persona signal
contracts. It starts as a Rust library that models resolved NOTA schema
documents, validates their type references, and derives fixed-root versus
ordered-box layout metadata for macro consumers.

## Carve-outs worth knowing

- This repository is a library surface right now, not a daemon triad.
  A future schema daemon/working-signal/policy-signal triad can grow from
  this once runtime registry authority is settled.
- Keep schema names as full English words and avoid ancestry-heavy names.
  Inside this crate, `Document`, `Declaration`, `Variant`, and
  `TypeExpression` are enough.
- The crate does not parse NOTA text yet. Parser work belongs behind the
  same typed model after the document shape is stable.
- The crate does not own signal-frame dispatch, version projection, or
  daemon upgrade orchestration. It supplies schema metadata those systems
  consume.
