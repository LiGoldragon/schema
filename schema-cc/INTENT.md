# INTENT — schema-cc

`schema-cc` is the **schema compiler-compiler**: the definition of the schema
language and its compiler, kept as inspectable typed data, that **generates** the
schema compiler rather than hand-writing it. (Spirit `vpbx`.)

## Why it exists

The stack already turns declared schema data into Rust (`schema` →
`schema-rust`). But the **compiler itself** — the reference-resolution
dispatch, the built-in head table, the shape vocabulary, the emission rules — is
still hand-written Rust whose correctness rests on match-arm ordering pinned by
tests. That is the one place that escaped *a language is data* (Spirit `7c71`):
the dispatch precedence cannot be *read* as a single artifact, so a human, an
LLM, and the resolver can each interpret it differently. It is also the surviving
hand-parsing the workspace already calls a violation to fix (Spirit `v0n6`) and
the fragility flag from the Spirit-engine analysis (report `651`).

`schema-cc` closes that gap: it pushes as much of the compiler's own definition
as possible into typed data and **generates** the compiler from it — extending
the *precedence-as-generative-source* decision (Spirit `549v`) from the
reference-resolution surface to the whole compiler.

## The layering it completes

| Tier | What | How it is defined |
|---|---|---|
| **Seed** | `nota` — the NOTA block parser + the one structural derive | irreducible, hand-written, context-free |
| **Definition** | `schema-cc` — reference grammar + dispatch precedence, built-in heads, shape vocabulary, emission rules | **typed data**, decoded by the seed |
| **Compiler** | `schema` / `schema-rust` — resolution + lowering + Rust emission | increasingly **generated** from `schema-cc` data |

The bootstrap bottoms out cleanly: `schema-cc`'s own definitions are structural
forms the `nota` seed can decode **without** the registry-aware resolver, so
the seed reads the definition, the definition generates the resolver/emitter, and
those handle everything user-declared. No chicken-and-egg.

## First inhabitant

`ReferenceGrammar` — the parenthesis-reference dispatch precedence (built-in
heads → declared macros → the generic application catch-all) reified as an
ordered typed value that **generates** the resolver (it is not interpreted at
runtime), with a validator (catch-all last and unique; no built-in/macro head
collision; sane arities). Then more of the definition migrates in.

## Discipline

- **Build-time only.** `schema-cc` generates compiler code; it never links into a
  runtime binary. Runtime binaries carry only their strict rkyv contracts
  (Spirit `9rjq`).
- **Generate, do not interpret.** The whole stack is `declared data → emitted
  Rust`; `schema-cc` follows it. A runtime grammar-interpreter would be a second,
  inconsistent mechanism — and would drag compiler machinery toward the runtime.
- **Everything reading NOTA structure goes through typed structural nodes**; if a
  shape cannot be expressed, surface it to the psyche rather than work around it
  (Spirit `v0n6`).
- **Upstream of `schema`.** Dependency order is `nota` → `schema-cc` →
  `schema` → `schema-rust`; `schema-cc` must not depend on `schema`
  (it generates into it).
