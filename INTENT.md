# INTENT — schema

*The psyche's intent for the `schema` repo, synthesised from primary
workspace intent records. Verbatim psyche quotes appear in italics
where the exact wording is load-bearing. Companion to
`ARCHITECTURE.md` (structural shape) and `AGENTS.md` (agent contract).
Maintenance: `skills/repo-intent.md` and `skills/intent-manifestation.md`.*

## What this repo is

`schema` exists because schema work now has its own repository. It
gives the schema-language and macro work a typed substrate instead
of keeping schema knowledge scattered across design reports and
macro internals.

## Schema defines data types only

Per psyche 2026-05-26 (records 713-715, Maximum certainty),
**`.schema` files declare data types — nothing else.** Effects,
fan-out targets, and effect tables are runtime dispatch / logic, not
authored schema content. *"Schema defines data types ONLY — no
effects no fan-out targets no effect tables."* Composer machinery
for those concerns may live behind the scenes; the AUTHORED schema
file has no Features section.

## The namespace is a key-value map of user-defined types

Per record 714: the NOTA schema namespace section is a key-value
**MAP** `{key value key value}` of user-defined types — not a flat
sequence of separate declarations. Keys are the type names; values
are the type definitions.

- **Enums declare inline** as `EnumName (Variant1 Variant2 …)`. Unit
  variants are bare PascalCase; data-carrying variants are
  `(VariantName field …)` parens with the inner shape.
- **Structs declare** as `StructName [FieldType1 FieldType2 …]`
  (positional fields).
- Variants may themselves be enums or data carriers — nesting is
  ordinary.
- **Universal `Unknown(String)` injection** on `*Response` enums is
  behind-the-scenes macro work, not a user-authored declaration.

Canonical authored example:
`/git/github.com/LiGoldragon/signal-persona-spirit/spirit.schema`.

## Language shape — positional, no labels

- **Records are positional.** Type first, then fields in declared
  order. The `(key value)` shape from Lisp/Clojure/JSON is not NOTA.
- **The schema language is itself NOTA**, so every NOTA discipline
  applies (bracket-only strings, three-case PascalCase, named enums
  rather than integer codes). See `repos/nota/INTENT.md`.

## What the repo provides

1. Represent the NOTA schema language as typed Rust data.
2. Lower authored schemas into `AssembledSchema`, the explicit
   machine object consumed by short-header generation, code
   emission, and version projection.
3. Support cross-schema imports (in-repo relative imports today;
   cross-crate resolution deferred).
4. Test the schema language end-to-end from actual `.schema` files
   using local relative imports before treating the Rust-only model
   as sufficient.
5. Stay library-shaped until the runtime schema registry / triad
   authority is explicitly settled.

## Schema language is downstream of NOTA

The schema language is layered on NOTA. NOTA discipline (positional
records, bracket-only strings, PascalCase rules, named enums) lives
in `repos/nota/INTENT.md`; the schema language consumes those as
laws.

## Open intent needing later settlement

- Whether this repository also owns the eventual `nota-box` crate
  surface or whether `nota-codec` keeps the wire container and
  `schema` only owns metadata.
- Whether a future schema daemon triad is required before schema
  metadata is queried at runtime.
- Whether schema imports begin as path references only, Cargo
  symbolic references, or both.
- The shape of upgrade-grammar primitives (`Migrate`, `RenamedFrom`,
  `Drop`, `Custom`, `Untranslatable`) and whether they live as
  schema-level annotations or move under the per-actor migration
  bridge mechanism that lives in the daemon crate (per
  `repos/persona-spirit/INTENT.md` §"Database upgrades are auto-
  migration on load").

## See also

- `ARCHITECTURE.md` — structural shape.
- Primary workspace: `INTENT.md` §"The schema-driven stack" —
  workspace-shape framing.
- `repos/nota/INTENT.md` — the NOTA language design the schema
  language consumes.
- `repos/signal-frame/INTENT.md` — the composer that consumes
  `AssembledSchema` and emits wire-side code.
- `repos/persona-spirit/INTENT.md` §"Every actor has a schema" —
  the canonical actor-schema discipline this language supports.
