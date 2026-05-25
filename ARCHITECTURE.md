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

Before typed lowering, `Schema::parse_str` parses authored text into
`nota_codec::NotaValue` trees. That generic tree preserves the shape needed
for macro dispatch: ordered maps, vectors, records, record head tokens,
PascalCase identifiers, local `./*` import paths, and block strings.

`multi_pass` is the executable macro-front proof path. It first builds a
`SchemaDocument` from the six positional values, then builds a `MacroIndex`
that records import, header, namespace-type, and feature macro endpoints
before any macro fires. Later passes walk those indexed candidates in schema
precedence order. This is the foothold for lazy resolution and forward
references: the engine knows where a named macro endpoint lives before a
later macro asks to invoke it.

Lowering runs through the builtin schema engine. Each indexed node first
passes through `NodeDefinitionShape::recognize`, pairing its
`NodeDefinitionPoint` with the observed `nota_codec::NotaValue` shape.
Import map values become `ImportInput`, header roots become `HeaderInput`,
namespace values split into `NamespaceValueShape::{Enum, Record, Newtype,
Alias}` before becoming `TypeInput`, and feature vector items become
`FeatureInput`. The input struct is the macro variant's payload; the lowerer
emits assembled fragments into a `LoweringContext`.

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

## File Reader

`Schema::parse_str` parses one authored `.schema` document through
`nota_codec::parse_sequence` and `shape_parser`. The old streaming
`nota-codec::Decoder` reader remains available as
`Schema::parse_str_with_streaming_decoder` for equivalence tests while the
macro-front path finishes taking over. `LoadedSchema::read_path` reads a
file, recursively loads local relative imports, validates selected imports
against exported names, resolves `ImportAll`, and assembles the result.

The reader treats imports as schema dependencies, not as comments or include
text. Imported names enter the local namespace through the existing import
validation path, then appear as imported entries in `AssembledSchema`.

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
├── multi_pass.rs   # NotaValue-driven macro index + builtin macro pipeline
├── parser.rs       # compatibility streaming parser over nota-codec
├── reader.rs       # file reader + recursive local imports
├── section.rs      # namespace map
├── shape_parser.rs # primary Schema::parse_str NotaValue shape parser
└── upgrade.rs      # upgrade annotations and plans

tests/
├── document.rs     # validation, lowering, layout, upgrade behavior
├── multi_pass.rs   # shape parser / streaming equivalence and basic pipeline
├── multi_pass_pipeline.rs # live Spirit pipeline and macro-index assertions
├── nota_shape.rs   # NotaValue shape-predicate checks on real fixtures
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
collision checks, basic upgrade planning, and the first NotaValue-driven
macro index / micro-macro lowering path. It is not yet a code generator,
runtime schema registry, user macro loader, fixed-point macro expander, or
database upgrade tool.
