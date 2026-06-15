# ARCHITECTURE — schema-cc

What `schema-cc` is and how it is built. Design rationale and the migration
roadmap live in designer report `652` (the leans + open questions). Read
`INTENT.md` first.

## Three tiers, bottoming out in the seed

```
SEED (frozen, hand-written)   nota-next   — block parser + the one structural derive; context-free
   │ decodes (no registry needed)
   ▼
DEFINITION (typed data)       schema-cc   — ReferenceGrammar, built-in heads, shape vocabulary, emission rules
   │ generates (emits Rust)
   ▼
COMPILER (generated)          schema-next / schema-rust-next — resolution, lowering, emission
   │ resolves
   ▼
USER schemas                  (Vector T), (Bag $X), (Foo A B) …
```

The bootstrap has no cycle: a `schema-cc` definition (e.g. a `ReferenceGrammar`
value) is written in NOTA using only shapes the **seed** decodes directly, so the
seed reads the definition without the registry-aware resolver; the definition
then generates that resolver; the resolver handles everything user-declared.

## The pipeline (build-time only)

`schema-cc` is a build-time generator — it never links into a runtime binary
(Spirit `9rjq`). One direction of typed flow:

```
NOTA definition text ──▶ nota-next decode ──▶ ReferenceGrammar (typed value)
                                                     │ TryFrom (validate)
                                                     ▼
                                          ValidatedReferenceGrammar
                                                     │ From (emit)
                                                     ▼
                                          ResolverModule (Rust tokens) ──▶ generated resolver source
```

- **Decode** — `ReferenceGrammar` derives nota-next's `StructuralMacroNode`, so
  the definition round-trips NOTA; no hand-rolled parser (the format already has
  one: the seed).
- **Validate** — `TryFrom<ReferenceGrammar>` produces a `ValidatedReferenceGrammar`
  carrying the invariant the generator relies on: the application catch-all is
  unique and last, no built-in/declared-macro head collides, arities are sane.
  This is the conflict check that match-arm ordering could not express, lifted to
  declared data (the registry-aware analogue of nota-next's
  `StructuralVariantSet::validate_no_silent_conflicts`).
- **Generate** — `From<&ValidatedReferenceGrammar>` for the emitted resolver
  module: the precedence-ordered dispatch that today is hand-written in
  schema-next's `from_parenthesis_objects`, emitted from the declared order via
  `proc-macro2`/`quote` + one `prettyplease` pass (the schema-rust-next emission
  style).

## Noun model (Rust discipline)

Behavior lives on the data-bearing types, never free functions or ZST holders:
`ReferenceGrammar` and `ValidatedReferenceGrammar` own validate/generate via
`TryFrom`/`From`; head names and arities are newtypes; errors are one
`thiserror` `Error` enum in `src/error.rs`. One concern per file
(`grammar.rs`, `validate.rs`, `generate.rs`, `error.rs`); tests under `tests/`.

## What is NOT here (v0 boundary)

The prototype proves the `ReferenceGrammar` → resolver generation path and the
validator standalone (generate + prove equivalence to schema-next's current
hand-written ordering). Re-wiring `schema-next` to *consume* the generated
resolver, and migrating further definition (built-in heads as data, the shape
vocabulary, emission rules), are the staged next steps in report `652`.
