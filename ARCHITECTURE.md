# schema — architecture

## Role

`schema` hosts the typed model for Persona's NOTA schema language. The
crate represents the fixed six-position authored `.schema` file, validates
imports and local namespace declarations, lowers route headers into
`AssembledSchema`, and derives metadata used by macro code for layout and
version projection.

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

`AssembledSchema` currently contains:

- import bindings with resolved imported names;
- explicit routes with leg, root slot, root name, endpoint slot, endpoint
  name, and body;
- local and imported type entries;
- feature metadata copied from the authored schema.

The route table is the object future short-header generation consumes. The
parser does not emit dispatch tables directly from raw authored text.

## Upgrade Model

Upgrade knowledge belongs to the next schema. The current library models an
`Upgrade` feature with `Migrate`, `RenamedFrom`, `Drop`, `Custom`, and
`Untranslatable` annotations.

`AssembledSchema::plan_upgrade_from` compares the next assembled schema to a
previous assembled schema. It currently infers identity projections and
additive enum-variant projections. Changed records require an explicit
annotation. Removed types require `Drop` or `Untranslatable`.

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

- NOTA text parsing. A reader will lower NOTA source into this model.
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
├── error.rs        # typed error enum
├── expression.rs   # primitive/container/named type expressions
├── feature.rs      # feature metadata
├── header.rs       # uniform route headers
├── import.rs       # import directives and resolved bindings
├── layout.rs       # fixed-root versus ordered-box planning
├── name.rs         # schema identifier validation
├── section.rs      # namespace map
└── upgrade.rs      # upgrade annotations and plans

tests/
└── document.rs     # validation, lowering, layout, upgrade behavior
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

The crate is now an MVP typed model for the v13 schema-language shape. It is
ready for parser and macro work to depend on the six-position structure,
uniform header routes, `AssembledSchema`, import collision checks, and basic
upgrade planning. It is not yet a parser, code generator, runtime schema
registry, or database upgrade tool.
