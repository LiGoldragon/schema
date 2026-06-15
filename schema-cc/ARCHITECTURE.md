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
                                          ReferenceDispatch (Rust tokens) ──▶ schema-next's resolver source
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
- **Generate** — `From<&ValidatedReferenceGrammar>` for `ReferenceDispatch`:
  schema-next's REAL parenthesis resolver, emitted as a method body over
  schema-next's own types (`TypeReference`/`SchemaError`/`MacroRegistry`/
  `MacroContext`/`Block`). The precedence-ordered dispatch that was hand-written
  in schema-next's `from_parenthesis_objects` is emitted from the declared order
  via `proc-macro2`/`quote` + one `prettyplease` pass (the schema-rust-next
  emission style). Each built-in arm dispatches to a uniform `resolve_<snake>`
  construction method that stays in schema-next; the reserved-head guard is
  derived from the grammar's built-in set; the `DeclaredMacro` + `Application`
  markers map to schema-next's `from_macro_or_application` tail.

## Noun model (Rust discipline)

Behavior lives on the data-bearing types, never free functions or ZST holders:
`ReferenceGrammar` and `ValidatedReferenceGrammar` own validate/generate via
`TryFrom`/`From`; head names and arities are newtypes; errors are one
`thiserror` `Error` enum in `src/error.rs`. One concern per file
(`grammar.rs`, `validate.rs`, `dispatch.rs`, `error.rs`); tests under `tests/`.

## How schema-next consumes it

schema-next takes `schema-cc` as a `[build-dependencies]` workspace `path` dep.
schema-next's `build.rs` reads the canonical grammar
(`schemas/reference-grammar.nota`), decodes + validates it through schema-cc,
emits `ReferenceDispatch`, and writes it to the COMMITTED, freshness-gated
`src/reference_resolver_generated.rs`: with `SCHEMA_NEXT_UPDATE_RESOLVER` set the
build (re)writes the file; unset, it byte-compares and fails on drift. The
library `include!`s that file, so the generated `resolve_parenthesis_reference`
becomes the method `from_block_with_registry`'s `Parenthesis` arm calls. The
hand-written `from_parenthesis_objects` match is retired. Byte-equivalence is
proven by schema-next's full test suite — `tests/identity.rs` blake3
hash-stability holds, so the generated dispatch behaves identically.

The earlier v0 standalone resolver (abstract `Resolution`/`ResolveError`
placeholders with `todo!()` arms) was retired with this wiring: it was a second
emission mechanism that could silently drift from the consumed one, and the real
dispatch subsumes its structure-and-precedence proof. Migrating further
definition (built-in heads as data, the shape vocabulary, emission rules) remains
the staged next step in report `652`.
