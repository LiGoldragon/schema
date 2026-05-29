# Architecture

`schema-next` turns NOTA structure into assembled schema.

## Pipeline

1. `nota-next::Document` parses source into blocks.
2. `SchemaEngine` records the document's `StructureHeader`: a compact
   first-two-level witness emitted by the NOTA delimiter pass.
3. `SchemaEngine` validates the root object count.
4. `MacroRegistry` dispatches position-aware macros for imports, input enum,
   output enum, namespace declarations, struct fields, and enum variants.
5. `Asschema` is emitted as the ordered macro-free endpoint.

## Raw Core Schema Reading

`RawSchemaFile` is the bottom layer used to inspect a core schema before
schema lowering. It takes a path plus source text, derives the root type name
from the file stem (`core.schema` -> `Core`), parses the source with
`nota-next`, and requires one root brace object.

The input file is still `.schema`, and `.schema` must be legal NOTA. Tests
that prove schema-file behavior use real `.schema` fixtures and parse them
through `nota-next::Document` before the raw schema reader interprets the
known root shape.

That root brace object is a native NOTA key/value map. Odd positions are
datatype names; even positions are raw datatype objects. The values preserve
the delimiter shape that the first NOTA pass saw:

- atom -> `RawNotaDatatype::Atom`
- pipe text -> `RawNotaDatatype::Text`
- `(...)` -> `RawNotaDatatype::Record`
- `[...]` -> `RawNotaDatatype::Vector`
- `{...}` -> `RawNotaDatatype::KeyValue`
- `(|...|)` -> `RawNotaDatatype::PipeParenthesis`
- `{|...|}` -> `RawNotaDatatype::PipeBrace`

This layer intentionally does not decide that a bracket is a string or that a
parenthesis is a tagged node. Those are schema expectations applied by later
readers. The raw layer only preserves the data object that the schema reader
will consume.

## Assembled Schema Endpoint

`Asschema` is currently the typed in-memory endpoint produced by lowering a
real `.schema` file. The previous checked-in assembled-schema text fixture
surface has been removed from active code because it confused raw NOTA bracket
structure with higher schema semantics.

Tests now prove the endpoint by asserting the Rust data directly:
`Declaration::{visibility, name, value}`, `Visibility::{Public, Private}`,
`TypeDeclaration::{Struct, Enum, Newtype}` and
`TypeReference::{String, Integer, Boolean, Path, Plain, Vector, Optional, Map}`. A
later serialized assembled schema format must be designed from the raw-NOTA
floor rather than reviving the obsolete vector-record fixture shape.

Namespace declarations are assembled as ordinary data-carrying visibility
objects: `(Public Name Value)` for exported top-level types and
`(Private Name Value)` for module-local types. The Rust storage keeps a
dedicated `Declaration` struct today, but the canonical data shape is the same:
visibility, declared name, and type value.

A struct value in asschema is a field-name -> type-reference map. The Rust
`StructFieldMap` stores the map in source order because generated Rust field
order and rkyv layout are load-bearing, but the semantic object is the brace
map shape that `Name@{ field@Type ... }` expands into.

## Core Macro Schema

`schemas/core.schema` is the schema-level description of the macro substrate.
It declares macro pattern and template bodies as typed object trees: captures,
rest captures, atoms, delimiter nodes, and ordered child vectors. That makes
the macro shape itself schema data instead of a string blob.

The current built-in registry still reads `schemas/builtin-macros.schema`
through the hand-written declarative macro reader. The near target is to lower
the core macro schema to asschema data, emit its Rust type, and make the macro
registry consume a typed macro table value instead of bespoke parser structs.

## At-Binding Declaration Syntax

The authored syntax is name-first `@` binding:

- `Name@{ ... }` lowers to the same struct declaration data.
- `Name@[ ... ]` lowers to enum declaration data.
- `name@Type` binds a field/member name to a reference.
- `name@(Composite Type)` binds a field/member name to a structural reference
  without losing the referenced object's parentheses.

The `@` marker belongs to declaration binding. It is not a macro-call marker;
schema-node macro calls remain tagged values read against a known expected
type. The root `.schema` object is the exception: its struct type is known from
the filename and its positions, so it is written as the root value rather than
wrapped in `RootName@{...}`.

Composite type references such as `(Vec Entry)`, `(Optional Entry)`, and
`(Map (Key Value))` still lower at reference positions to `TypeReference`
data. If a composite appears unnamed as a struct field, the field/type name can
be derived from the composite shape when it does not collide.

Inline PascalCase declarations are private by default. For example,
`Entry@{ Receipt@{ recordIdentifier@RecordIdentifier } later@Receipt }`
inserts `Receipt` into the module-local declaration table as private, derives
the field name `receipt`, and lets the later field reference the same local
type. Top-level declarations are public by default.

The pipe-family forms `Name {| Name ... |}` and `Name (| Name ... |)` are a
legacy compatibility surface only. `Name@(...)` also remains accepted as an
enum declaration compatibility shape, but new authored schema should use
`Name@[]` so parentheses stay reserved for composite/reference and macro-call
argument objects.

## Schema Package Entry

`SchemaPackage` is the first crate-local module loader. It expects a crate root
with a `schema/` directory and loads `schema/lib.schema` as the entrypoint.
Additional module schemas are addressed by name through
`schema/<module>.schema`; colon-qualified module names map to nested paths.
The loaded module receives an identity such as `spirit-next:lib`.

This loader is also the floor for cross-crate import resolution. A consumer
build script registers dependency schema directories on `ImportResolver`
(normally from Cargo `DEP_<CRATE>_SCHEMA_DIR` values). `SchemaEngine` then
turns each import declaration into a resolved import by loading the dependency
module schema and checking that the imported type is declared there.

## Constraints

- `MacroPosition` is passed into both `matches` and `lower`.
- `SchemaMacro` receives a `MacroObject` input (`Block` or namespace `Pair`)
  so each macro declares the object shape it consumes at its position.
- `MacroRegistry` is the engine dispatch path for schema sections and nested
  type-body lowering. Concrete macro fields on `SchemaEngine` or root macros
  are not the design.
- `MacroContext` records positions and applied macro names as diagnostics.
  Tests prove lowering by asserting on typed `Asschema` data, not by treating
  trace strings as the load-bearing witness.
- `MacroContext` records the NOTA `StructureHeader` so tests can prove schema
  lowering consumed the source's first-pass structural shape.
- `Asschema` stores root declarations and visibility-tagged namespace
  declarations in `Vec` order; lookup maps are derived.
- Active code does not keep assembled-schema text fixtures. The current
  serialized file-level witness is `.schema`, parsed as NOTA first and then
  lowered.
- The root schema is positional. Current MVP shape:
  - field 1: named input root enum, for example `Input@[Record@Entry Reindex]`
  - field 2: named output root enum, for example `Output@[Recorded@Receipt Rejected@Rejection]`
  - field 3: namespace map `{ }`
  - optional leading field: imports map `{ Local dependency-crate:module:Type }`
- Input and output roots are actor reaction languages. They declare
  the variants a component can receive and emit; the Rust emission
  layer turns those variants into executor methods and signal-frame
  route headers.
- The same root shape is used for Signal, Nexus, and SEMA schema files:
  input, output, and namespace, with optional leading imports. The macro
  engine lowers all three planes uniformly; the runtime meaning differs after
  lowering.
- `Name@[...]` defines authored enum declarations. The older
  `Name@(...)` and `Name (| Name ... |)` forms are compatibility syntax.
- Braces are key/value maps only. They are not enum sugar.
- `Name@{ field@Reference ... }` defines authored struct declarations; the
  one-field form remains a newtype struct. Lowercase/camelCase names bind
  fields. PascalCase names declare or reference schema types. The older
  `Name {| Name ... |}` pipe-brace form is compatibility syntax.
- Plain square brackets are NOTA vector/bracket structure. They may appear as
  macro payload data or value data, but they are not the authored declaration
  syntax for a schema datatype.
- The root `Schema` name is implicit when reading a `.schema` file. Nested
  enum, struct, and newtype definitions still carry their own names.
- `schemas/root.schema` describes that known root `Schema` type.
- Schema names may be qualified with single colons (`crate:module:Type`). The
  local part drives derived field names; the full name remains available for
  global disambiguation and future import resolution.
- Root namespace braces hold self-named declaration objects such as
  `Entry@{...}` and `Kind@[...]`. Redundant `Entry Entry@{...}` key/value
  pairs and parenthesized `(Name Body)` entries are rejected.
- Schema objects are the shared language for Signal, Nexus, SEMA,
  upgrade, and mail-event surfaces. Signal is the wire/message plane,
  Nexus is the execution-IO plane for internal effects, external calls, and
  UI panels, and SEMA is the durable-state plane. Nexus also owns in-flight
  mail state between Signal ingress and SEMA replies. If runtime code needs a
  new verb or event, the schema gets the data type first; Rust implements
  behavior on the generated object.
- Imports and exports are schema objects too. Their paths use the workspace's
  single-colon namespace (`crate:module:Type`) so assembled schema can mirror
  the Rust module tree without inheriting Rust's `::` syntax.
- Cross-crate imports resolve through `ImportResolver`, not ad hoc text
  substitution. A local import alias names a dependency type by
  `crate:module:Type`; resolution records the dependency Rust path so
  `schema-rust-next` can emit a `pub use` alias and keep one type identity
  across the crate boundary.
- `TypeReference` at a reference position is an enum:
  `String`, `Integer`, `Boolean`, `Path`, `Plain(Name)`, `Vector(Box<TypeReference>)`,
  `Map(Box, Box)`, and `Optional(Box<TypeReference>)`. `String`, `Integer`,
  `Boolean`, and `Path` are reserved scalar leaves, so they are not user namespace
  declarations and cannot be shadowed by schema types. `Plain(Name)` now means
  "a declared type by name." `TypeReference::from_block` lowers a bare scalar
  symbol to its scalar variant, a different bare PascalCase symbol to `Plain`,
  `(Vec T)` to `Vector`, `(Map (K V))` to `Map`, and `(Optional T)` to
  `Optional`. These names are Schema type-reference vocabulary over
  nota-next's already-parsed structures, not raw NOTA keywords. The inner
  positions recurse, so `(Vec (Optional Topic))` and
  `(Map (String (Vec Service)))` nest. Parentheses with another head are
  dispatched to the user macro registry. An unknown head or wrong native
  argument count is a typed `SchemaError::UnknownTypeReferenceForm`. Lowering
  is pure semantics over nota-next's already-parsed blocks — not a hand-rolled
  text parser.
- Collection references reach every reference position. Struct fields are
  written as `field@(Composite Reference)` inside an at-brace declaration:
  `serviceVector@(Vec Service)`, `byTopic@(Map (Topic RecordIdentifier))`,
  `optionalCache@(Optional Cache)`. Enum-variant payloads, root input/output
  variant payloads, and import sources all lower their type through
  `TypeReference::from_block`.
- Inline PascalCase at-declarations at a type-reference position lower to a
  `Plain` reference and insert a private declaration before the containing
  public declaration in `Asschema.namespace`. This makes
  `Entry@{ Receipt@{ ... } later@Receipt }` declare private `Receipt` first
  and then public `Entry`, so later fields can reuse the inline type by name.
- `SchemaNode` is the data model for macro calls before execution. It reads a
  parenthesized object as a tagged/data-carrying node: first object is the tag,
  second object is the data. This prevents macro invocation from being a hidden
  parser branch; macro calls can be inspected and represented as assembled data.

## Syntax Schema Layer

`SyntaxSchema` is the typed layer directly above `RawSchemaFile`. It proves the
new delimiter contract without skipping into the older macro engine:

1. `RawSchemaFile` parses a `.schema` file as legal NOTA and preserves raw
   delimiter objects.
2. `SyntaxSchema` reads the raw datatype map into declaration objects.
3. `@` braces at datatype declaration position are struct field lists.
   Plain square brackets are rejected there.
4. `(Vec T)`, `(Map (K V))`, and `(Optional T)` are Schema type-reference
   objects and lower into composite type references.
5. `Name@[...]` is an enum declaration and `Name@{...}` is a struct
   declaration. The declared name must match the namespace key, so the raw map
   key and the self-named declaration cannot silently drift.

The proof fixture is `tests/fixtures/syntax-layer/schema.schema`; the tests in
`tests/syntax_layer.rs` assert the raw-to-syntax result directly.
