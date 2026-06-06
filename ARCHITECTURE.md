# Architecture

`schema-next` turns NOTA structure into typed schema source data.

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
5. `SchemaSource` lowers into `Schema`, the ordered semantic schema value used
   by Rust emission and schema upgrade logic.

## Authored Schema Source

`SchemaSource` is the current typed authored-language value produced after raw
`nota-next::Document` parsing. The target schema pipeline is that authored
`.schema` deserializes into Rust datatypes that fully define the schema; that
schema-in-Rust value is rkyv-serializable; and Rust interface code is lowered
from that typed value. `SchemaSource` reads a full schema module document —
optional imports, input root enum, output root enum, and namespace — into
source-language nouns:

- `SourceImports`
- `SourceRootEnum`
- `SourceNamespace`
- `SourceDeclarationValue`
- `SourceStructBody`
- `SourceEnumBody`
- `SourceVariantSignature`
- `SourceReference`

`SchemaSourceArtifact` owns `.schema` text file IO. Its writer emits one
canonical source projection from the typed source object, and the reader parses
that projection back through NOTA before rebuilding `SchemaSource`. This is the
authored-source side of the schema-in-Rust pipeline: source text is a projection
of a typed source object, not a string handed directly to every later stage.
`SchemaSource` and `SchemaSourceArtifact` also own the rkyv archive boundary
for that source value. The binary archive stores the typed schema-in-Rust nouns
only: imports, root enums, namespace declarations, source references,
struct-field bodies, enum bodies, and variant payloads. Parser spans, raw block
helpers, structural match wrappers, and resolver/lowering helpers are not part
of that archive.

`SchemaModuleSource::lower` decodes into `SchemaSource` first and lowers that
typed source object directly through `SchemaEngine`. Package/module callers
therefore have a named source value and a round-trippable source artifact
before semantic `Schema`.

Root input/output headers are resolved against the typed source namespace before
semantic schema is emitted. A bare header entry such as `Lookup` remains one
variant-signature object in the bracket vector; if the namespace declares
`Lookup RecordIdentifier`, the assembled root variant becomes
`Lookup(Plain Lookup)`, and `Lookup` is an exported newtype object. A
parenthesized inline declaration such as `(Lookup { RecordIdentifier * })`
also creates an exported `Lookup` declaration before the root enum is lowered.
This resolution happens on `SchemaSource` data, not by rewriting the user's
source string into the older `(Lookup RecordIdentifier)` pair form.

Enum variant entries in authored source are typed structural NOTA nodes.
`SourceVariantSignature` implements `nota-next::StructuralMacroNode` and uses
the same ordered `EnumVariants` structural cases as codec-facing
`nota-next::StructuralVariant` values: a bare PascalCase atom is a unit/header
variant, a parenthesized two-object block is a data-carrying variant, and the
four-object forms `(Variant Payload opens StreamName)` /
`(Variant Payload belongs StreamName)` attach subscription lifecycle metadata.
After the expected enum type selects the structural case, Schema decodes the
captures into either a reference payload or an inline declaration payload, plus
an optional `StreamRelation`, and `SchemaSourceArtifact` writes the same
structural form back out. That keeps schema sugar inside NOTA instead of making
it a separate one-way lowering language.

Stream declarations are typed source metadata in the namespace map, not
namespace Rust data types. The source form is `StreamName (Stream { token Token
opened Snapshot event Event close Close })`; it lowers to semantic
`StreamDeclaration` data on `Schema::streams()` and is excluded from
`Schema::namespace()`. This keeps push-subscription features visible in schema
while preventing stream lifecycle records from masquerading as ordinary payload
types.

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

## Semantic Schema

`Schema` is the semantic schema-in-Rust value produced by lowering a real
`.schema` source value. It is not a serialized text artifact and it is not an
Asschema compatibility projection. Authored `.schema` text remains the NOTA
projection owned by `SchemaSourceArtifact`; the semantic `Schema` value is the
typed data object consumed by Rust emission, schema upgrade logic, symbol-path
queries, and semantic assertions.

`Schema` archives through rkyv with `Schema::to_binary_bytes` and
`Schema::from_binary_bytes`. There is intentionally no `Schema::to_nota`, no
semantic `.asschema` / `.asschema.rkyv` artifact owner, and no schema store in this
crate. Asschema is retired, not preserved as a compatibility endpoint: the
`.asschema` text artifact, the `.asschema.rkyv` binary, the `AsschemaArtifact`
owner, and the redb-backed semantic store are removed outright. The pipeline is
`.schema` -> schema-in-Rust (`SchemaSource` source nouns own resolution) ->
`Schema` -> Rust. The text/binary source artifact lives at
`SchemaSourceArtifact`, and database work lives in production SEMA engines, not
in schema-next.

Tests prove the endpoint by asserting the Rust data directly and by
round-tripping the produced `Schema` through rkyv:
`Declaration::{visibility, name, value}`, `Visibility::{Public, Private}`,
`TypeDeclaration::{Alias, Struct, Enum, Newtype}` and
`TypeReference::{String, Integer, Boolean, Path, Plain, Vector, Optional, Map}`.
The semantic-schema text fixture surface stays removed: there is no checked
`.asschema` file and no hand-kept golden semantic-schema text.

Schema names emit through their own `Name` codec, not through the ordinary
`String` codec. A symbol-safe name is written bare (`Entry`,
`schema:spirit:Entry`) so declarations and references read as schema symbols;
only non-symbol names fall back to bracket-string text. Actual `String`
type-reference values still use the normal NOTA string surface at value
positions.

`SymbolPath` is the typed identity projection for schema positions. It is a
newtype over ordered `Name` segments and can be derived from a `Schema` root
variant, namespace type, struct field, or enum variant. Its NOTA form is
structured data such as `(SymbolPath [spirit-next:lib Input Record])`; its
human `Display` form may join the same segments as
`spirit-next:lib/Input/Record`. Trace names, help entries, description
namespace keys, and future indexes should use this typed path surface instead
of inventing ad hoc path strings.

The stored path stays a segment vector so deeper schema positions can grow
without changing the binary object. Position meaning is recovered through the
owning schema: `Schema::symbol_path_position` validates the component segment
and classifies the local path as a namespace type, root variant, struct field,
or enum variant. Consumers that need role semantics ask the semantic schema
instead of guessing from segment count.

Namespace declarations are assembled as ordinary data-carrying visibility
objects: `(Public Name Value)` for exported top-level types and
`(Private Name Value)` for module-local types. The Rust storage keeps a
dedicated `Declaration` struct today, but the canonical data shape is the same:
visibility, declared name, and type value.

A bare reference value in schema is an alias. `Topic String`,
`Lookup RecordIdentifier`, and `Rejected SignalRejection` preserve exported
schema names without adding nominal Rust wrappers around identical payload
types.

A struct value in schema is a field-name -> type-reference map. The Rust
`StructFieldMap` stores the map in source order because generated Rust field
order and rkyv layout are load-bearing, but the semantic object is the brace
map shape that authored `{ field Type TypeName * }` struct bodies lower into.

A newtype value in schema is a single contained type reference, not a
one-field struct map with an invented field name. Its long form is
`(Public Topic { String })`, not `(Public Topic { text String })`, and the
Rust emitter consumes the contained reference directly when emitting a tuple
newtype.

Struct-body lowering has the same rule before the semantic `Schema` value is
written: a body that produces exactly one field becomes `Newtype`, and a body
that produces two or more fields stays `Struct`. The source may spell that
single field as a derived member (`Entry { Topic * }`) or an explicit one-field
wrapper (`Wrapper { value Topic }`); once the body has only one contained
reference, the field label is not part of the type.

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

The near target is to lower the core macro schema to schema data, emit its
Rust type, and replace the hand-written `MacroLibrary` noun with the
schema-emitted macro table type directly. The macro table is already real
serializable data and is already the runtime load path; the remaining loop is
making its Rust noun schema-emitted.

Declarative macro expansion preserves structural NOTA objects while lowering.
Pattern captures store the matched `Block` values, rest captures store ordered
`Block` vectors, and templates expand into an owned object tree before
`MacroExpansionTemplate` lowers the result. Compact notation remains only as a
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
  as bare bracket bodies: `[]`, `[Record Observe]`, or explicit signatures
  such as `[(Record Entry) Observe]`. The root does not carry labels; position supplies
  `Input` and `Output`.
- Namespace braces contain `TypeName Value` pairs. `Topic String` and
  `Topics (Vec Topic)` are alias declarations; `Entry { topic Topic }` is a
  struct declaration; `Kind [Decision Correction]` is an enum declaration.
- A brace declaration with one field lowers as a newtype. `Entry { Topic * }`
  and `Wrapper { value Topic }` both describe one contained `Topic` reference;
  only a multi-field brace remains a named-field struct.
- Struct braces contain field-name -> type-reference pairs. `topic Topic` is
  explicit. `Topics *` derives the field name from an already-defined type and
  lowers to `topics: Topics`.
- Enum bodies are bracket/vector structure. Each object in that vector is a
  variant signature: a bare PascalCase symbol for a unit variant or a
  parenthesized `(Variant PayloadType)` record for a data-carrying variant.
  A variant signature is one object, so the bracket remains a homogeneous
  vector of variant-signature objects.
- At root input/output positions, a bare PascalCase variant may resolve to a
  same-named exported namespace declaration. The source header says `Lookup`;
  the namespace says what `Lookup` is. Inline root declarations are also
  accepted and are inserted into the exported namespace before assembly.

Composite type references such as `(Vec Entry)`, `(Optional Entry)`, and
`(Map (Key Value))` still lower at reference positions to `TypeReference`
data. If a composite appears unnamed as a struct field, the field/type name can
be derived from the composite shape when it does not collide.

Inline PascalCase declarations are strict pairs inside struct maps:
`Receipt { recordIdentifier RecordIdentifier }` creates a private
module-local `Receipt` type and the containing struct field derives the
field name `receipt`. Top-level declarations are public; inline PascalCase
declarations are private and appear before the containing public type in
`Schema.namespace`.

The default authored-schema path accepts brace key/value pairs, square-bracket
enum bodies, parenthesized references, and bare root bracket bodies. Earlier
forms that require a declaration to repeat its own name are not registered in
the default parser.

## Macro Node Structural Matching

`MacroNodeDefinition` is the schema-side wrapper for "what can appear here."
Its structural cases are `nota-next::MacroNodeDefinition` values: each case is
a serializable pattern over atoms, delimiters, literals, rest captures, and
named captures. Schema-next contributes the schema `MacroPosition` and the
handler that turns a match into an `Schema` fragment; nota-next owns the
shape matcher.

The namespace declaration node makes the strict brace model executable:

```nota
Entry { Topics * }     ; symbol key + brace value -> struct declaration case
Kind [Decision]        ; symbol key + bracket value -> enum declaration case
Topic String           ; symbol key + reference value -> alias declaration case
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
readers strip matched delimiters before semantic lowering. Authored
`SchemaSource` enum bodies already route through a typed `StructuralMacroNode`
consumer over `StructuralVariant` values derived from the shared
`EnumVariants` case list. The next convergence work is to make the remaining
schema source nodes structural macro node types, then move consumers off the
Schema compatibility endpoint.

## Schema Package Entry

`SchemaPackage` is the crate-local module loader. It expects a crate root with
a `schema/` directory. `load_lib` still loads `schema/lib.schema` as the
compatibility entrypoint, while `load_modules` scans every `.schema` file under
`schema/` and derives module identities from relative paths. For example,
`schema/nexus.schema` becomes `crate-name:nexus`, and
`schema/internal/effect.schema` becomes `crate-name:internal:effect`.
Colon-qualified module names keep the inverse mapping through
`schema/<module>.schema`.

Package lowering self-registers the current package with `ImportResolver`, so
separate plane files inside one daemon crate can import each other by the same
single-colon path used for dependency imports. The intended triad daemon shape
is ordinary crate-local files such as `schema/nexus.schema` and
`schema/sema.schema`, not per-plane crates. `schema/lib.schema` remains only an
entrypoint compatibility path for older one-file pilots.

This loader is also the floor for cross-crate import resolution. A consumer
build script registers dependency schema directories on `ImportResolver`
(normally from Cargo `DEP_<CRATE>_SCHEMA_DIR` values). `SchemaEngine` then
turns each import declaration into a resolved import by loading the dependency
module schema and checking that the imported name is declared there as either a
namespace type or an input/output root enum. Root imports matter because daemon
Nexus and SEMA schemas import the public signal contract roots without
re-declaring wire types.

Resolving an imported module preserves the caller's resolver. When
`nexus.schema` imports the local `sema.schema`, and `sema.schema` itself
imports contract-root types, the SEMA validation runs with the same package
and dependency map that Nexus received. The resolver is not reset at nested
module boundaries.

## Constraints

- `MacroPosition` is passed into both `matches` and `lower`.
- `SchemaMacro` receives a `MacroObject` input (`Block` or namespace `Pair`)
  so each macro declares the object shape it consumes at its position.
- `MacroRegistry` is the engine dispatch path for schema sections and nested
  type-body lowering. Concrete macro fields on `SchemaEngine` or root macros
  are not the design.
- `MacroContext` records positions and applied macro names as diagnostics.
  Tests prove lowering by asserting on typed `Schema` data, not by treating
  trace strings as the load-bearing witness.
- `MacroContext` records the NOTA `StructureHeader` so tests can prove schema
  lowering consumed the source's first-pass structural shape.
- `Schema` stores the known `Input` and `Output` enum declarations as
  direct fields, then stores visibility-tagged namespace declarations in
  `Vec` order. The two roots are heterogeneous product positions, not a
  homogeneous vector of root wrappers.
- Active code does not keep hand-written semantic-schema text fixtures and
  does not expose a semantic-schema store. `Schema` can serialize itself to
  rkyv after lowering; source text and source rkyv archives are owned by
  `SchemaSourceArtifact`.
- Checked-in artifacts are limited to authored schema source and macro-library
  artifacts. `schemas/core.schema` and `schemas/builtin-macros.macro-library`
  are freshness-checked against their source inputs and consumed through typed
  artifact objects.
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
  reference defines an alias: `Topic String`, `Topics (Vec Topic)`.
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
  single-colon namespace (`crate:module:Type`) so semantic schema can mirror
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
  containing public declaration in `Schema.namespace`. `Entry { Receipt
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
   bodies; atom or parenthesized reference values become aliases.
4. `(Vec T)`, `(Map (K V))`, and `(Optional T)` are Schema type-reference
   objects and lower into composite type references.
5. The root map key is the declaration name. There is no second declaration
   name inside the value.

The proof fixture is `tests/fixtures/syntax-layer/schema.schema`; the tests in
`tests/syntax_layer.rs` assert the raw-to-syntax result directly.
