# Architecture

`schema-next` turns NOTA structure into assembled schema.

## Pipeline

1. `nota-next::Document` parses source into blocks.
2. `SchemaEngine` records the document's `StructureHeader`: a compact
   first-two-level witness emitted by the NOTA delimiter pass.
3. `SchemaEngine` validates the root object count.
4. `MacroRegistry` dispatches position-aware macros for imports, input enum,
   output enum, namespace declarations, struct fields, and enum variants. Its
   structural expectations are `nota-next` macro-node definitions: schema-next
   supplies schema positions and handlers, while nota-next supplies pattern
   matching, named captures, and no-match diagnostics.
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

`Asschema` is the typed, macro-free data endpoint produced by lowering a real
`.schema` file. It is also a live artifact: `Asschema::to_nota` writes the
assembled schema as legal NOTA, `Asschema::from_nota_source` reads that NOTA
back as the same typed value, and `Asschema::to_binary_bytes` /
`Asschema::from_binary_bytes` archive and restore the same value through rkyv.

The canonical `.asschema` text is a known-root document, not an outer record:
the document contains the six `Asschema` root fields in order. The reader uses
the root struct type it was asked to decode, so the fourth and fifth document
objects are the input and output enum bodies directly. They are not serialized
as `(Input ...)` or `(Output ...)` records; those names come from the expected
root fields.

This uses the `nota-next` body codec. `Asschema::from_nota_source` delegates
to `NotaSource::parse_body`, and `Asschema::to_nota` delegates to the
`#[nota(known_root)]`-derived body encoder. The root body mechanism belongs to
NOTA; schema-next only implements the named-field projection that turns the
input/output body slots into `EnumDeclaration` values named by the root type.

Tests prove the endpoint by asserting the Rust data directly and by
round-tripping the produced `Asschema` through NOTA and rkyv:
`Declaration::{visibility, name, value}`, `Visibility::{Public, Private}`,
`TypeDeclaration::{Struct, Enum, Newtype}` and
`TypeReference::{String, Integer, Boolean, Path, Plain, Vector, Optional, Map}`.
The previous checked-in assembled-schema text fixture surface stays removed:
the live serialized form comes from the typed data object, not from hand-kept
golden `.asschema` text.

Asschema names emit through their own `Name` codec, not through the ordinary
`String` codec. A symbol-safe name is written bare (`Entry`,
`schema:spirit:Entry`) so declarations and references read as schema symbols;
only non-symbol names fall back to bracket-string text. Actual `String`
type-reference values still use the normal NOTA string surface at value
positions.

`AsschemaArtifact` is the artifact owner. It wraps the assembled value and
reads or writes `.asschema` NOTA text plus `.asschema.rkyv` binary bytes. The
artifact object is the handoff surface for downstream code generation: callers
may still inspect `artifact.asschema()`, but build paths can now materialize
and consume the serialized artifact explicitly instead of using a private
lowerer-to-emitter value.

`schemas/core.asschema` is checked in as the assembled artifact for
`schemas/core.schema`. Tests lower `core.schema` through the live engine and
compare the result to the checked-in artifact, then round-trip the artifact
through NOTA and rkyv. This makes the schema substrate visible in review as
three data stages: authored `.schema`, assembled `.asschema`, and downstream
emitted Rust.

`AsschemaStore` is the SEMA persistence surface for assembled schema. It owns a
redb database, stores rkyv-archived `Asschema` values in the
`assembled-schemas` table keyed by schema identity, reads them back through the
same `Asschema::from_binary_bytes` path, and exports `.asschema` NOTA through
`AsschemaArtifact`. The store does not parse authored schema and does not
render text itself; it persists binary typed values and delegates projection to
the artifact object.

Namespace declarations are assembled as ordinary data-carrying visibility
objects: `(Public Name Value)` for exported top-level types and
`(Private Name Value)` for module-local types. The Rust storage keeps a
dedicated `Declaration` struct today, but the canonical data shape is the same:
visibility, declared name, and type value.

A struct value in asschema is a field-name -> type-reference map. The Rust
`StructFieldMap` stores the map in source order because generated Rust field
order and rkyv layout are load-bearing, but the semantic object is the brace
map shape that authored `{ field Type TypeName * }` struct bodies lower into.

A newtype value in asschema is a single contained type reference, not a
one-field struct map with an invented field name. Its long form is
`(Public Topic { String })`, not `(Public Topic { text String })`, and the
Rust emitter consumes the contained reference directly when emitting a tuple
newtype.

## Core Macro Schema

`schemas/core.schema` is the schema-level description of the macro substrate.
It declares macro pattern and template bodies as typed object trees: captures,
rest captures, atoms, delimiter nodes, and ordered child vectors. That makes
the macro shape itself schema data instead of a string blob.

The built-in registry reads `schemas/builtin-macros.macro-library` as one
serialized `MacroLibrary` value and builds executable macro handlers from that
same noun. `schemas/builtin-macros.schema` remains the bootstrap source: tests
parse it through the declarative reader as a `MacroLibrary` and require exact
equality with the checked-in artifact.

The authored bootstrap source and the serialized artifact share the same
library noun. `MacroLibrary` owns `Vec<MacroLibrarySourceEntry>`, and the
current source-entry enum has one case: `SchemaMacro(SchemaMacro)`. Therefore
the source notation `(SchemaMacro Name Position Pattern Template)` is modeled
as a tagged source-entry variant carrying a definition payload, not as a bare
string sentinel or as a second artifact-only enum.

The near target is to lower the core macro schema to asschema data, emit its
Rust type, and replace the hand-written `MacroLibrary` noun with the
schema-emitted macro table type directly. The macro table is already real
serializable data and is already the runtime load path; the remaining loop is
making its Rust noun schema-emitted.

Declarative macro expansion preserves structural NOTA objects while lowering.
Pattern captures store the matched `Block` values, rest captures store ordered
`Block` vectors, and templates expand into an owned object tree before
`AssembledTemplate` lowers the result. Compact notation remains only as a
diagnostic string for `MacroContext`; the live expansion path does not emit a
template string and parse it back through `Document::parse`.

The current structural expansion object can lower template-owned atoms and
delimiter nodes plus captured source blocks. Registry dispatch for arbitrary
type-reference macro invocations still operates on source `Block` values; the
fully shared version belongs with the nota-next macro-node substrate once it
exposes owned structural macro objects.

## Strict Key/Value Schema Syntax

The authored syntax preserves NOTA brace meaning: every brace is a key/value
map. Schema sugar may shorten values, but it must not turn a brace entry into
one logical declaration object.

- Root input/output positions are known by the schema reader and are written
  as bare bracket bodies: `[]`, `[(Record Entry)]`, or
  `[(Record Entry) Observe]`. The root does not carry labels; position supplies
  `Input` and `Output`.
- Namespace braces contain `TypeName Value` pairs. `Topic String` and
  `Topics (Vec Topic)` are newtype declarations; `Entry { topic Topic }` is a
  struct declaration; `Kind [Decision Correction]` is an enum declaration.
- Struct braces contain field-name -> type-reference pairs. `topic Topic` is
  explicit. `Topics *` derives the field name from an already-defined type and
  lowers to `topics: Topics`.
- Enum bodies are bracket/vector structure. Each object in that vector is a
  variant signature: a bare PascalCase symbol for a unit variant or a
  parenthesized `(Variant PayloadType)` record for a data-carrying variant.
  A variant signature is one object, so the bracket remains a homogeneous
  vector of variant-signature objects.

Composite type references such as `(Vec Entry)`, `(Optional Entry)`, and
`(Map (Key Value))` still lower at reference positions to `TypeReference`
data. If a composite appears unnamed as a struct field, the field/type name can
be derived from the composite shape when it does not collide.

Inline PascalCase declarations are strict pairs inside struct maps:
`Receipt { recordIdentifier RecordIdentifier }` creates a private
module-local `Receipt` type and the containing struct field derives the
field name `receipt`. Top-level declarations are public; inline PascalCase
declarations are private and appear before the containing public type in
`Asschema.namespace`.

The default authored-schema path accepts brace key/value pairs, square-bracket
enum bodies, parenthesized references, and bare root bracket bodies. Earlier
forms that require a declaration to repeat its own name are not registered in
the default parser.

## Macro Node Structural Matching

`MacroNodeDefinition` is the schema-side wrapper for "what can appear here."
Its structural cases are `nota-next::MacroNodeDefinition` values: each case is
a serializable pattern over atoms, delimiters, literals, rest captures, and
named captures. Schema-next contributes the schema `MacroPosition` and the
handler that turns a match into an `Asschema` fragment; nota-next owns the
shape matcher.

The namespace declaration node makes the strict brace model executable:

```nota
Entry { Topics * }     ; symbol key + brace value -> struct declaration case
Kind [Decision]        ; symbol key + bracket value -> enum declaration case
Topic String           ; symbol key + reference value -> newtype declaration case
```

`KeyValueDeclarationMacro::matches` delegates through the nota-next pattern
registry instead of merely checking "is this a pair." When no registered macro
matches at a node position with known cases,
`SchemaError::UnsupportedMacroNodeStructure` reports the schema position,
expected macro-node cases, and actual shape using the nota-next no-match
diagnostic. Type-reference positions retain their existing
`UnknownTypeReferenceForm` path so unknown collection heads remain precise.

Schema-next is now a consumer of the NOTA-layer macro mechanism for structural
cases. Delimited captures from nota-next expose inner `NotaBody` streams, and
the built-in root imports, root namespace, root enum, and struct-field map
readers strip matched delimiters before semantic lowering. The next convergence
work is to route successful `MacroMatch` captures directly into schema
handlers, then load the schema macro vocabulary from serialized Asschema data
instead of constructing the bootstrap registry in Rust.

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
- `Asschema` stores the known `Input` and `Output` enum declarations as
  direct fields, then stores visibility-tagged namespace declarations in
  `Vec` order. The two roots are heterogeneous product positions, not a
  homogeneous vector of root wrappers.
- `AsschemaStore` persists assembled schemas as rkyv bytes in a redb-backed
  `.sema` database and re-exports NOTA through `AsschemaArtifact`; the store
  never owns a parallel text format.
- Active code does not keep hand-written assembled-schema text fixtures.
  `Asschema` can serialize itself to NOTA and rkyv after lowering, and tests
  read those serialized forms back through the same typed object.
- Checked-in core artifacts are allowed because they are first-class pipeline
  outputs, not hand-maintained witness text. `schemas/core.asschema` and
  `schemas/builtin-macros.macro-library` are freshness-checked against their
  source inputs and consumed through typed artifact objects.
- The root schema is positional. Current MVP shape:
  - field 1: input root enum body, for example `[(Record Entry) Reindex]`
  - field 2: output root enum body, for example `[(Recorded Receipt) (Rejected Rejection)]`
  - field 3: namespace map `{ TypeName Value ... }`
  - optional leading field: imports map `{ Local dependency-crate:module:Type }`
- Input and output roots are actor reaction languages. They declare
  the variants a component can receive and emit; the Rust emission
  layer turns those variants into executor methods and signal-frame
  route headers.
- The same root shape is used for Signal, Nexus, and SEMA schema files:
  input, output, and namespace, with optional leading imports. The macro
  engine lowers all three planes uniformly; the runtime meaning differs after
  lowering.
- A namespace key followed by a square-bracket value defines an enum
  declaration: `Kind [Decision Correction]`.
- Braces are key/value maps only. They are not enum sugar.
- A namespace key followed by a brace value defines a struct declaration:
  `Entry { topic Topic Topics * }`. `TypeName *` derives the field name from
  an existing type; explicit `field TypeReference` remains available when the
  field name differs. A namespace key followed by an atom or parenthesized
  reference defines a newtype: `Topic String`, `Topics (Vec Topic)`.
- Square brackets are NOTA vector/bracket structure. At enum-body positions
  they contain homogeneous variant-signature objects: bare symbols for unit
  variants and parenthesized `(Variant PayloadType)` records for data variants.
- The root `Schema` name is implicit when reading a `.schema` file. Nested
  enum, struct, and newtype definitions still carry their own names.
- `schemas/root.schema` describes that known root `Schema` type.
- Schema names may be qualified with single colons (`crate:module:Type`). The
  local part drives derived field names; the full name remains available for
  global disambiguation and future import resolution.
- Root namespace braces hold key/value declaration pairs such as
  `Entry { topic Topic }` and `Kind [Decision Correction]`. Redundant
  doubled-name key/value pairs and parenthesized `(Name Body)` entries are
  rejected.
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
  written as strict pairs such as `serviceVector (Vec Service)`,
  `byTopic (Map (Topic RecordIdentifier))`, and
  `optionalCache (Optional Cache)`. Enum-variant payloads, root input/output
  variant payloads, and import sources all lower their type through
  `TypeReference::from_block`.
- Inline PascalCase declaration pairs insert a private declaration before the
  containing public declaration in `Asschema.namespace`. `Entry { Receipt
  { recordIdentifier RecordIdentifier } later Receipt }` declares private
  `Receipt` first and then public `Entry`, so later fields can reuse the
  inline type by name.
- `SchemaNode` is the data model for macro calls before execution. It reads a
  parenthesized object as a tagged/data-carrying node: first object is the tag,
  second object is the data. This prevents macro invocation from being a hidden
  parser branch; macro calls can be inspected and represented as assembled data.

## Syntax Schema Layer

`SyntaxSchema` is the typed layer directly above `RawSchemaFile`. It reads the
strict key/value authored surface without invoking macro lowering:

1. `RawSchemaFile` parses a `.schema` file as legal NOTA and preserves raw
   delimiter objects.
2. `SyntaxSchema` reads the raw datatype map into declaration objects.
3. Brace values become struct field maps; square-bracket values become enum
   bodies; atom or parenthesized reference values become aliases/newtypes.
4. `(Vec T)`, `(Map (K V))`, and `(Optional T)` are Schema type-reference
   objects and lower into composite type references.
5. The root map key is the declaration name. There is no second declaration
   name inside the value.

The proof fixture is `tests/fixtures/syntax-layer/schema.schema`; the tests in
`tests/syntax_layer.rs` assert the raw-to-syntax result directly.
