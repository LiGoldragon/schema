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

This is intentionally a small loader, not full cross-crate import resolution.
It proves the convention that schema lives beside the crate source, that the
crate name is the first namespace segment, and that schema modules are ordinary
files in a predictable folder.

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
  - field 1: imports/exports map `{ }`
  - field 2: input enum definition `(Input (...))`
  - field 3: output enum definition `(Output (...))`
  - field 4: namespace map `{ }`
- Input and output roots are actor reaction languages. They declare
  the variants a component can receive and emit; the Rust emission
  layer turns those variants into executor methods and signal-frame
  route headers.
- The same root shape is used for Signal, Nexus, and SEMA schema files:
  imports/exports, input, output, and namespace. The macro engine lowers
  all three planes uniformly; the runtime meaning differs after lowering.
- Parentheses define enums and variants. A named enum definition is
  `(Name (Variant ...))`.
- Brace pair bodies are enum sugar only at enum-variant positions. The body
  `{Variant Payload Variant Payload}` lowers through a macro to payload-carrying
  enum variants; odd brace counts are rejected rather than guessed.
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
