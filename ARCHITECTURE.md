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
`TypeDeclaration::{Struct, Enum, Newtype}` and
`TypeReference::{Plain, Vector, Optional, Map}`. A later serialized assembled
schema format must be designed from the raw-NOTA floor rather than reviving the
obsolete vector-record fixture shape.

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
- `MacroContext` records positions and applied macro names so tests can prove
  lowering used the macro path.
- `MacroContext` records the NOTA `StructureHeader` so tests can prove schema
  lowering consumed the source's first-pass structural shape.
- `Asschema` stores declarations in `Vec` order; lookup maps are derived.
- Active code does not keep assembled-schema text fixtures. The current
  serialized file-level witness is `.schema`, parsed as NOTA first and then
  lowered.
- The root schema is positional. Current MVP shape:
  - field 1: input enum body, for example `((Record Entry) Reindex)`
  - field 2: output enum body, for example `((Recorded Receipt) (Rejected Rejection))`
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
- Parentheses define enums and variants. A named enum definition is
  `(Name (Variant ...))`.
- Braces are key/value maps only. They are not enum sugar.
- Square brackets define structs and their fields. A named struct definition
  is `(Name [FieldType ...])`; the one-field form is a newtype struct.
- The root `Schema` name is implicit when reading a `.schema` file. Nested
  enum, struct, and newtype definitions still carry their own names.
- `schemas/root.schema` describes that known root `Schema` type.
- Schema names may be qualified with single colons (`crate:module:Type`). The
  local part drives derived field names; the full name remains available for
  global disambiguation and future import resolution.
- Root namespace braces are key/value maps only. Authored declarations use
  `Name Body`; parenthesized `(Name Body)` entries inside braces are rejected.
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
- `TypeReference` at a reference position is an enum: `Plain(Name)`,
  `Vector(Box<TypeReference>)`, `Map(Box, Box)`, `Optional(Box<TypeReference>)`.
  `TypeReference::from_block` lowers a bare PascalCase symbol to `Plain`,
  `(Vec T)` to `Vector`, `(Map (K V))` to `Map`, and `(Optional T)` to
  `Optional`. The inner positions recurse, so `(Vec (Optional Topic))` and
  `(Map (NodeName (Vec Service)))` nest. Parentheses with another head are
  dispatched to the user macro registry. An unknown head or wrong native
  argument count is a typed `SchemaError::UnknownTypeReferenceForm`. Lowering
  is pure semantics over nota-next's already-parsed blocks — not a hand-rolled
  text parser.
- Collection references reach every reference position. Struct fields accept a
  typed NOTA type-reference object directly (`(Vec Service)`,
  `(Map (Topic RecordIdentifier))`, `(Optional Cache)`) and derive a field name
  from that reference. The explicit lower-case pair `(fieldName TypeReference)`
  remains only as an escape hatch for uncommon field names. Enum-variant
  payloads, root input/output variant payloads, and import sources all lower
  their type through `TypeReference::from_block`.
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
3. Square brackets at datatype declaration position are struct field lists.
   They do not mean `Vec`.
4. `(Vec T)`, `(Map (K V))`, and `(Optional T)` are typed NOTA datatype
   objects and lower into composite type references.
5. `(| Name ... |)` is an enum declaration and `{| Name ... |}` is a struct
   declaration. The first item must match the namespace key, so the raw map key
   and the self-named declaration cannot silently drift.

The proof fixture is `tests/fixtures/syntax-layer/schema.schema`; the tests in
`tests/syntax_layer.rs` assert the raw-to-syntax result directly.
