# schema — architecture

## Role

`schema` hosts the typed model for Persona's NOTA schema language. The
crate represents the fixed six-position authored `.schema` file, validates
imports and local namespace declarations, reads `.schema` files with local
relative imports, lowers route headers into `AssembledSchema`, and derives
metadata used by macro code for layout and version projection.

The crate is the schema-language substrate, not the eventual schema daemon.
It is consumed by macros and later by the runtime registry.

## Authored Shape

The authored file has no outer `(Schema ...)` wrapper. The file path and
parser mode already supply the type. The six fields are positional:

1. imports map
2. ordinary signal header
3. owner signal header
4. sema header
5. namespace map
6. features vector

Imports are a NOTA map from a local provenance binding to an import
directive. The supported MVP directives are `Import` and `ImportAll`.
Imported names enter the local namespace directly; the binding does not
create a qualified prefix.

Each header root uses the uniform v13 form `(Root [SubVariant ...])`.
Single-sub-variant roots still use the vector form. Header entries are route
selectors only. Body types are declared in the namespace and connected during
lowering.

The namespace is a flat `BTreeMap<Name, DeclarationBody>`. That means route
root body declarations reserve their root key. A schema cannot define both a
normal data type named `State` and a route-body declaration named `State` in
the same namespace.

## Lowered Shape

`Schema::assemble` resolves imports and lowers the authored schema into
`AssembledSchema`.

Lowering runs through the builtin schema engine. Each authored node is
translated into a data-carrying `BuiltinMacroVariant` at a
`NodeDefinitionPoint`: import map values become `ImportInput`, header roots
become `HeaderInput`, namespace values become `TypeInput`, and feature vector
items become `FeatureInput`. The input struct is the macro variant's payload;
the lowerer emits assembled fragments into a `LoweringContext`.

`AssembledSchema` currently contains:

- import bindings with resolved imported names;
- explicit routes with leg, root slot, root name, endpoint slot, endpoint
  name, body, and optional Sema engine class;
- local and imported type entries;
- feature metadata copied from the authored schema.

The route table is the object short-header generation consumes. A route can
project itself into the MVP 64-bit short header (`byte 0 = root slot`, `byte 1
= endpoint slot`) and `AssembledSchema` can resolve a route back from that
header plus leg. The parser does not emit dispatch tables directly from raw
authored text.

## Reader model — multi-pass NOTA-first

Per spirit record 549, the schema reader uses a multi-pass NOTA-first
model: one universal NOTA parser produces a generic `NotaValue` tree, then
context-sensitive macro passes interpret values per their
`NodeDefinitionPoint` position, then assembly produces the canonical
`AssembledSchema`. One syntax (NOTA), one parser, multiple semantic passes
layered on top.

The pass sequence:

1. **Pass 0 — lexical** (text → tokens). Lives in `nota-codec` as
   `Lexer::next_token`. No schema-specific behavior.
2. **Pass 1 — syntactic** (tokens → `NotaValue` tree). Aspirationally
   lives in `nota-codec`; current code builds a private tree assembler on
   top of the streaming `Decoder`. The intended home is `nota-codec`
   exposing `NotaValue` + `parse_str` + `Lexer::next_token_with_span` for
   every NOTA-reading client (schema, sema, intent records, lock files).
3. **Pass 2 — structural** (`NotaValue` sequence → schema document
   positions). A `.schema` file is six top-level values in sequence with
   no enclosing wrapper. Pass 2 reads six successive values matching
   imports / ordinary header / owner header / sema header / namespace /
   features.
4. **Pass 3 — macro identification**. Collect names; identify macro
   positions in the namespace; resolve imports for cross-schema references.
5. **Pass 4 — macro application**. Reuse `BuiltinMacroVariant` +
   `LoweringContext`. Imports run first because subsequent variants may
   reference imported names.
6. **Pass 5 — assembly**. Conceptually distinct: assembly, validation,
   UID minting, layout. Operationally folded into the lowering context's
   `finish()`.

Byte-equivalence with the canonical reader is the proof: the multi-pass
module's output is Debug-equivalent to `LoadedSchema::read_path` for all
three live schemas (spirit / version-handover / orchestrate). The current
single-pass implementation in `parser.rs` is already correct; the
multi-pass refactor clarifies pass boundaries and relocates Pass 1 down
to `nota-codec`. Full design + corrections in
`primary/reports/designer/334-v2-multi-pass-nota-first-schema-reader.md`.

## Macro fixed-point iteration

Per spirit record 569, schema macro application is iterative to a fixed
point. Each pass identifies macro positions in the namespace and applies
their lowerers; macros can introduce new macros into the namespace which
trigger further passes; iteration continues until no macro positions
remain and the namespace is pure typed enums, structs, and newtypes.

The lowering loop terminates when the namespace stabilises — when one
pass produces no new macro positions, the namespace is fully lowered.
`UpgradeRule` (landed via closed bead `primary-cklr`) is one of the
macro variants that participates in this fixed-point lowering loop.

## Namespace dependency order

Per spirit record 570, the schema namespace is dependency-ordered: most
basic definitions come first, derived definitions later; consumers
reference only earlier-declared names. This supports linear loading (per
record 553) and matches the schema-as-source-of-truth principle (record
551).

## Newtype shape

Per spirit record 571, a newtype in the schema language emits a Rust
single-tuple struct that wraps inner data of some named type (scalar,
vector, or other), exposing the inner trait implementations transparently
while letting the newtype carry its own trait impls. This is the
`Newtype` schema position; it folds into `TypeInput` rather than
carrying a separate `NewtypeDefinition` variant.

## Diff taxonomy and slot policy

Per spirit record 561, schema diff operations have three families —
**Add**, **Remove**, **Modify** — and Modify subdivides into
`ContainerEmbed`, `EnumWrap`, `Reorder`, and `KeyChange` sub-cases.
This taxonomy frames the upgrade-rule emission in §"Upgrade Model".

Per spirit record 562, **data-carrying variants take the first seven
enum slots (0-6); unit variants come after**. This slot-assignment
policy makes adding a new unit variant a no-op upgrade on the wire
format — the new unit variant lands after the existing units, no
mechanical upgrade needed. Enum-slot planning at authoring time
minimises database rewrites across compatible upgrades (record 557).

Per spirit record 563, enum space is pre-allocated by inner-type
semantics so each enum occupies the minimum bits its variant set
requires: Boolean fits in one bit, Option in two (None vs Some-with-
inner-tag), and micro-enums let SEMA fit smaller than rkyv with raw
enums by pre-breaking the encoding space by type. The wire form is the
discriminator, not the string — common workspace identifiers (record,
forward, send, and similar per record 564) are already stored as enum-
encoded composite names with a composable namespace across components.
This enables multilingual labels because the wire form is the
discriminator, not the string.

## File Reader

`Schema::parse_str` parses one authored `.schema` document through
`nota-codec::Decoder`. `LoadedSchema::read_path` reads a file, recursively
loads local relative imports, validates selected imports against exported
names, resolves `ImportAll`, and assembles the result.

The reader treats imports as schema dependencies, not as comments or include
text. Imported names enter the local namespace through the existing import
validation path, then appear as imported entries in `AssembledSchema`.

## Upgrade Model

Upgrade knowledge belongs to the next schema. The current library models an
`Upgrade` feature with `Migrate`, `RenamedFrom`, `Drop`, `Custom`, and
`Untranslatable` annotations, plus `UpgradeRule` as a `BuiltinMacroVariant`
landed for explicit schema-diff operation emission (closed bead
`primary-cklr`).

`AssembledSchema::plan_upgrade_from` compares the next assembled schema to a
previous assembled schema. It currently infers identity projections and
additive enum-variant projections. Changed records require an explicit
annotation. Removed types require `Drop` or `Untranslatable`.

The diff taxonomy (Add / Remove / Modify[ContainerEmbed | EnumWrap |
Reorder | KeyChange]) per record 561 frames how the planner emits typed
upgrade operations. The slot policy in §"Diff taxonomy and slot policy"
keeps adding a new unit variant a no-op upgrade.

This is an MVP planner for macro emission, not the runtime handover engine.
Runtime database copy, dual-write, and failure reporting belong to the
upgrade component and signal contracts.

## Boundaries

**Owns:**

- Schema document types: `Schema`, `Imports`, `Header`, `Namespace`,
  `Declaration`, `Variant`, `Payload`, `TypeExpression`, `Feature`,
  `Upgrade`, and `AssembledSchema`.
- Curly-brace NOTA map compatibility for schema names through `Name` as a
  `NotaMapKey`.
- Validation of duplicate import bindings, duplicate imported names,
  import-local collisions, duplicate declarations, duplicate variants, and
  named type references after import resolution.
- Conservative root-versus-box layout planning for data-carrying
  declarations.
- First-pass previous/next upgrade planning.

**Does not own:**

- `signal_channel!` code emission. That stays in the macro crate.
- Short-header frame bytes. Those stay in `signal-frame`.
- Version projection execution. That stays in `version-projection` and the
  macro-emitted impls.
- Daemon runtime, storage opening, live database copying, and handover
  orchestration.

## Code Map

```text
src/
├── lib.rs          # public exports
├── assembled.rs    # AssembledSchema, routes, assembled types
├── declaration.rs  # declarations, variants, payloads
├── document.rs     # Schema + validation + lowering
├── engine.rs       # builtin macro variants + lowering context
├── error.rs        # typed error enum
├── expression.rs   # primitive/container/named type expressions
├── feature.rs      # feature metadata
├── header.rs       # uniform route headers
├── import.rs       # import directives and resolved bindings
├── layout.rs       # fixed-root versus ordered-box planning
├── name.rs         # schema identifier validation
├── parser.rs       # .schema text parser over nota-codec
├── reader.rs       # file reader + recursive local imports
├── section.rs      # namespace map
└── upgrade.rs      # upgrade annotations and plans

tests/
├── document.rs     # validation, lowering, layout, upgrade behavior
├── reader.rs       # .schema files, local imports, file-based upgrade
└── fixtures/       # real .schema fixtures
```

## Invariants

Schema records are positional. Data-carrying variant fields are ordered
`TypeExpression` values; the model does not carry field labels.

Names are PascalCase identifiers because declarations and variants become
closed Rust enums or enum-like schema nodes.

Import collisions are loud. Two imports cannot introduce the same local
identifier, and an imported name cannot collide with a local namespace key.

Every lowered route has an endpoint slot. In a one-sub-variant header, the
endpoint slot is `0`; there is no scalar route form.

Validation stays conservative. Import-all references are fully checked when
assembly receives explicit resolved names. Unknown types, duplicate
declarations, duplicate variants, and impossible route-body lookups are
errors before code emission starts.

Layout planning is conservative. Built-in fixed-width primitives and unit
enums can stay in the root. Strings, bytes, containers, unresolved imported
types, and recursively variable declarations move to boxes until a later
resolved schema proves otherwise.

## Status

The crate is now an MVP parser and typed model for the v13 schema-language
shape. It is ready for macro work to depend on the six-position structure,
uniform header routes, local import loading, `AssembledSchema`, import
collision checks, and basic upgrade planning. It is not yet a code
generator, runtime schema registry, or database upgrade tool.
