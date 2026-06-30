# ARCHITECTURE — schema-cc

What `schema-cc` is, why it exists, and how it is built. Design rationale and the
migration roadmap live in designer report `652` (the leans + open questions).

## What it is and why it exists

`schema-cc` is the schema **compiler-compiler**: the definition of the schema
language and its compiler, kept as inspectable typed data, that **generates** the
schema compiler rather than hand-writing it (Spirit `vpbx`).

The stack already turns declared schema data into Rust (`schema` →
`schema-rust`). But the compiler itself — reference-resolution dispatch, the
built-in head table, the shape vocabulary, the emission rules — was hand-written
Rust whose correctness rested on match-arm ordering pinned by tests. That is the
one place that escaped *a language is data* (Spirit `7c71`): the dispatch
precedence could not be read as a single artifact, so a human, an LLM, and the
resolver could each interpret it differently. It was also the surviving
hand-parsing the workspace calls a violation to fix (Spirit `v0n6`) and the
fragility flag from the Spirit-engine analysis (report `651`). `schema-cc` closes
that gap by pushing as much of the compiler's own definition as possible into
typed data and generating the compiler from it, extending the
precedence-as-generative-source decision (Spirit `549v`) from reference
resolution to the whole compiler.

The first inhabitant is `ReferenceGrammar` — the parenthesis-reference dispatch
precedence (built-in heads → declared macros → the generic application catch-all)
reified as an ordered typed value that generates the resolver, with a validator;
more of the definition migrates in from there.

## Discipline (direction)

- **Build-time only.** `schema-cc` generates compiler code; it never links into a
  runtime binary. Runtime binaries carry only their strict rkyv contracts
  (Spirit `9rjq`).
- **Generate, do not interpret.** The whole stack is `declared data → emitted
  Rust`, and `schema-cc` follows it. A runtime grammar-interpreter would be a
  second, inconsistent mechanism and would drag compiler machinery toward the
  runtime.
- **Everything reading NOTA structure goes through typed structural nodes**; if a
  shape cannot be expressed, surface it to the psyche rather than work around it
  (Spirit `v0n6`).
- **Upstream of `schema`.** Dependency order is `nota` → `schema-cc` →
  `schema` → `schema-rust`; `schema-cc` must not depend on `schema` — it
  generates into it, and the reverse edge would be a cycle.

## Three tiers, bottoming out in the seed

```
SEED (frozen, hand-written)   nota   — block parser + the one structural derive; context-free
   │ decodes (no registry needed)
   ▼
DEFINITION (typed data)       schema-cc   — ReferenceGrammar, built-in heads, shape vocabulary, emission rules
   │ generates (emits Rust)
   ▼
COMPILER (generated)          schema / schema-rust — resolution, lowering, emission
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
NOTA definition text ──▶ nota decode ──▶ ReferenceGrammar (typed value)
                                                     │ TryFrom (validate)
                                                     ▼
                                          ValidatedReferenceGrammar
                                                     │ From (emit)
                                                     ▼
                                          ReferenceDispatch (Rust tokens) ──▶ schema's resolver source
```

- **Decode** — `ReferenceGrammar` derives nota's `StructuralMacroNode`, so
  the definition round-trips NOTA; no hand-rolled parser (the format already has
  one: the seed).
- **Validate** — `TryFrom<ReferenceGrammar>` produces a `ValidatedReferenceGrammar`
  carrying the invariant the generator relies on: the application catch-all is
  unique and last, no built-in/declared-macro head collides, arities are sane.
  This is the conflict check that match-arm ordering could not express, lifted to
  declared data (the registry-aware analogue of nota's
  `StructuralVariantSet::validate_no_silent_conflicts`).
- **Generate** — `From<&ValidatedReferenceGrammar>` for `ReferenceDispatch`:
  schema's REAL parenthesis resolver, emitted as a method body over
  schema's own types (`TypeReference`/`SchemaError`/`MacroRegistry`/
  `MacroContext`/`Block`). The precedence-ordered dispatch that was hand-written
  in schema's `from_parenthesis_objects` is emitted from the declared order
  via `proc-macro2`/`quote` + one `prettyplease` pass (the schema-rust
  emission style). Each built-in arm dispatches to a uniform `resolve_<snake>`
  construction method that stays in schema; the reserved-head guard is
  derived from the grammar's built-in set; the `DeclaredMacro` + `Application`
  markers map to schema's `from_macro_or_application` tail.

## Noun model (Rust discipline)

Behavior lives on the data-bearing types, never free functions or ZST holders:
`ReferenceGrammar` and `ValidatedReferenceGrammar` own validate/generate via
`TryFrom`/`From`; head names and arities are newtypes; errors are one
`thiserror` `Error` enum in `src/error.rs`. One concern per file
(`grammar.rs`, `validate.rs`, `dispatch.rs`, `error.rs`); tests under `tests/`.

## How schema consumes it

schema takes `schema-cc` as a `[build-dependencies]` workspace `path` dep.
schema's `build.rs` reads the canonical grammar
(`schemas/reference-grammar.nota`), decodes + validates it through schema-cc,
emits `ReferenceDispatch`, and writes it to the COMMITTED, freshness-gated
`src/reference_resolver_generated.rs`: with `SCHEMA_UPDATE_RESOLVER` set the
build (re)writes the file; unset, it byte-compares and fails on drift. The
library `include!`s that file, so the generated `resolve_parenthesis_reference`
becomes the method `from_block_with_registry`'s `Parenthesis` arm calls. The
hand-written `from_parenthesis_objects` match is retired. Byte-equivalence is
proven by schema's full test suite — `tests/identity.rs` blake3
hash-stability holds, so the generated dispatch behaves identically.

The earlier v0 standalone resolver (abstract `Resolution`/`ResolveError`
placeholders with `todo!()` arms) was retired with this wiring: it was a second
emission mechanism that could silently drift from the consumed one, and the real
dispatch subsumes its structure-and-precedence proof. Migrating further
definition (built-in heads as data, the shape vocabulary, emission rules) remains
the staged next step in report `652`.
