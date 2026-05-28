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
- The root schema is positional. Current MVP shape:
  - field 1: input enum body, for example `((Record Entry) Reindex)`
  - field 2: output enum body, for example `((Recorded Receipt) Rejected*)`
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
  `TypeReference::from_block` lowers a bare PascalCase symbol to `Plain` and a
  parenthesised explicit macro-marker form to a collection — `(@Vec (T))` →
  `Vector`, `(@KeyValue (K V))` → `Map`, `(@Option (T))` → `Optional`. The
  `@` head is a macro marker atom, not a schema symbol; the inner positions
  recurse, so collections nest. An unknown head or wrong argument count is a typed
  `SchemaError::UnknownTypeReferenceForm`; an empty parenthesis is
  `SchemaError::EmptyTypeReference`. Lowering is pure semantics over nota-next's
  already-parsed blocks — not a hand-rolled text parser.
- Collection references reach every reference position. Struct fields accept an
  explicit pair `(fieldName TypeReference)` for a directly-typed collection
  field (`(services (@Vec (Service)))`); a bare PascalCase field stays the legacy
  plain shape with its name derived from the type. Enum-variant payloads, root
  input/output variant payloads, and import sources all lower their type through
  `TypeReference::from_block`. A schema with no collection lowers
  byte-identically to the pre-collection engine.
