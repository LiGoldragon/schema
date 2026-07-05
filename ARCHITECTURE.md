# Architecture

`schema` is the schema macro engine and typed semantic schema data model for the
schema-derived stack. It turns NOTA structure into typed schema source data and
does not emit Rust source code itself; that is `schema-rust`'s job.

## Direction

Durable direction the psyche has set for this repo, kept beside the architecture
it shapes. The implemented surface is described in the sections that follow; the
direction below states where the design is going and which choices are settled,
including where the authored syntax is meant to evolve past its current form.

### What schema is

- Schema is the macro-language source of truth for component data, wire,
  storage, and upgrade behavior. Every Spirit, Signal, Nexus, and SEMA operation
  is declared in a `.schema` file first; generated schema types are the source
  of every operation data type, and handwritten Rust only implements behavior on
  those generated nouns (Spirit `1aam`, `ycmd`). Hand-written Rust is the engine
  logic alone — the decision-making bodies of actor methods; every structural
  thing (NOTA form, rkyv form, type definitions, enum variants, field accessors)
  emits from schema, so the engine never reinvents the data-structure wheel.
- Schema is the programmable composability layer over NOTA (Spirit `er9w`). NOTA
  is a thin structure-sensing library carrying no meaning; schema interprets its
  quote-free delimiter structure into typed input/output interfaces. The same
  NOTA text means different things under different schemas, and schemas express
  composable data types and data-driven behavior trees for Signal, Nexus, SEMA,
  and sub-engine behavior — a reusable declaration bedrock, not only a Spirit
  contract format.
- Schema is self-hosting all the way down (Spirit `g2xr`, `khbv`, `sanf`). NOTA's
  own grammar is described by the foundational schema, which generates the Rust
  that interprets NOTA; the authoring language is itself a macro expanding to
  assembled-schema data, so the schema-of-schemas generates its own assembled and
  short-syntax forms. The meta-schema is written in the schema language's own
  format and the language is specified by its own `.schema` file. The bootstrap
  Rust reader stays hand-written until the self-description lowers through the
  engine cleanly.
- The schema language is its own compiler-compiler (Spirit `vpbx`). The compiler
  definition itself — reference grammar, dispatch precedence, built-in heads,
  shape vocabulary, and emission rules — is kept as typed data that *generates*
  the schema compiler (`schema`/`schema` and its Rust emitter
  `schema-rust`/`schema-rust`) rather than being hand-written, bottoming
  out in the `nota` seed reader. The governing rule is to push as much of
  the compiler definition into data as possible, so the hand-written surface
  shrinks toward the irreducible bootstrap seed and every other compiler choice
  is an editable schema artifact rather than embedded Rust control flow.
- Schema is a superset of Cap'n-Proto-style spec languages (Spirit `tace`),
  adding its own module system, a macro system, and shape-driven node-type
  matching (structure plus member count plus member types resolve a node's type)
  without becoming general-purpose. Its scope is the workspace's Rust subset
  only. The recursive-parser-plus-dispatch-plus-lowering model is a reusable
  library for any NOTA DSL; schema is its first application. The structural
  patterns (deep typed trees, closed variants, recursive composition, separated
  syntax/verification/semantics) emerged first in the psyche's Aski language work
  — `aski-core` rkyv contract types for the compiler/verifier/semantics triad and
  the core parse-tree primitives — and the schema engine is the second iteration
  of those insights (Spirit `ospz`; an earlier STT artifact misheard Aski as
  ASCII).

### Foundational model

- Everything is a struct (Spirit `umsv`, `2cuo`). A unit variant is a zero-field
  struct, a data variant a one-field struct, an enum a single-field struct
  holding the active variant; a `Vec` is a single-field struct and a vector is
  structurally an enum (a choice over its members). Maps are reserved for
  namespaces. This supersedes the abandoned at-sigil for macro invocation:
  positional typing against the schema-of-schemas resolves variant-versus-macro
  with no sigil. The schema is one recursive shape — a root struct with
  macro-expanded fields down to scalar leaves — and recursion terminates at
  built-in macros over scalar leaves (integers, unit-variant enums, booleans,
  typed-newtype strings). Bool's `True`/`False` is a two-unit-variant enum, which
  is why NOTA capitalizes booleans.
- The schema language handles only the workspace Rust subset (Spirit `b05y`):
  named-field structs, named-variant enums (bare, data-carrying, struct-variant,
  nested), single-field newtype structs, and scalar primitives. No tuples —
  tuples are poorly-defined structs and excluded. Domain concepts should be typed
  beyond raw `String` through typed-string newtypes, and `EnumIdentifier` is a
  core constrained-string type.
- A struct field's role is its type, so no two fields share a type — dimensional
  correctness, `Height` versus `Width` (Spirit `a5tg`). Center design on schema
  types: prefer value enums that name a field's closed set of cases over
  stringly-typed fields, and thread schema-emitted types end to end rather than
  re-deriving them by hand. Schema enum variants are themselves enums or
  data-carrying enums — the mandatory two-layer enum-of-enums structure (Spirit
  `wvpg`). An interface is a root enum with more than one variant; a root naming
  only one operation is an incomplete design and a newtype, not an interface, and
  interface chain depth measures design realism (Spirit `2f04`).
- Separate a thing's categorical KIND (one-of-N essence, no off) from its
  additive CAPABILITY vector (Spirit `gjr1`). Kind is an explicit categorical
  choice validated at projection with a typed error, never modelled as
  pre-selected defaults an additive vector overrides, since an additive vector
  cannot express un-selection.

### Authored-syntax direction

The current implemented surface (described under Strict Key/Value Schema Syntax
and Constraints) uses brace key/value struct bodies and the `*` star shorthand.
The settled direction is to move struct bodies to bare positional type lists:

- Schema is purely positional (Spirit `6wwf`, `a5tg`, `5jac`). A name appears
  only where something is genuinely keyed — a namespace map entry by its key, a
  sum variant by its tag. A positional slot, such as a struct's fields or the
  Input/Output enum positions, is bare because position already says what it is.
  An entry is just a list of type names (`Entry (Topic Kind Summary)`); the field
  name derives by lowercasing the type name. The name-value struct form and the
  star shorthand are the retirement target, not the destination. When an explicit
  field name differs from the type-derived name it uses a dot differentiator
  (`source.TypeReference`), and dot syntax is invalid when the explicit role
  equals the type-derived name. Repetition is a keyed collection, never repeated
  fields.
- The built-in type-name vocabulary lives in Schema, not NOTA (Spirit `iypq`):
  the scalars `String`, `Integer`, `Boolean`, `Path` and the composites `Vec`,
  `Optional`, `Map`, `Set`, `Box` are Schema's type-reference vocabulary
  alongside struct/enum declarations. NOTA stays pure structure plus the codec
  and the value literals `None`/`(Some x)`. The `(Vec X)` tagged form and the
  value-versus-type distinction (`[X]` is a vector value, `(Vec X)` the type)
  hold but live at the Schema layer. `TypeReference` application uses the flat
  head form `(Map K V)`.
- Schema-next needs a `Bytes` primitive (ideally fixed-size byte arrays) beyond
  `String`/`Integer`/`Boolean`/`Path` so binary values — BLS keys, signatures,
  digests, fingerprints, nonces — type as bytes, plus a hash-identifier type
  whose value is bytes but whose NOTA projection is a canonical short string code,
  generalizing the 12-byte base36 `RecordIdentifier` (Spirit `yp29`).
- `DomainScope` uses ordinary schema-emitted recursive enum semantics, staying
  typed all the way through matching, expansion, and equivalence; generated code
  never hand-parses path shortcuts, hides stop markers, or degrades values into
  `Debug` strings or `Vec<String>` paths (Spirit `94sj`). Domain subdomains are
  mandatory to a leaf, so a domain value is always complete; `DomainScope` is a
  separate generated prefix language over the same recursive domain tree — a
  `ScopeOf` construct where stopping early is the prefix semantics rather than
  optional subdomain data, with no `Some`/`None` exposed (Spirit `izib`). Domain
  relations live as first-class schema-declared vocabulary in a dedicated
  taxonomy schema surface, a new schema interface kind alongside the plane
  schemas, and the relation/expansion mechanism is reusable across components such
  as the mind (Spirit `mn3k`). Within that taxonomy, `Technology` divides into two
  parallel clusters, `Hardware` and `Software`; every child of `Technology` is a
  cluster with no bare leaves, so all hardware is a queryable scope mirroring the
  software cluster (Spirit `tw15`).

### Settled architecture choices

- Reading NOTA-shaped structure above the raw structural parser must go through
  typed structural macro nodes. Surviving hand-parsing sites (such as a schema
  macro library that parses by hand) are design violations to fix, not
  acceptable code. If a structural macro node cannot express a needed shape,
  that signals the NOTA design was not implemented properly and must be
  surfaced to the psyche rather than worked around (Spirit `v0n6`).
- The schema-layout description is fully content-addressable: its hash is its
  identity, and any edit to the schema changes the address, which is the
  version the version-control layer consumes (Spirit `wrjl`). The hash is
  computed on the semantic schema-in-Rust value, never on `.schema` text, so
  formatting-only source edits do not move the address.
- One consistent cryptographic basis spans the version-control and backup
  system: blake3 for all content addressing, with no component diverging in
  hash function (Spirit `x0ja`). The whole-schema and family-closure hash kinds
  are domain-separated through distinct blake3 `derive_key` contexts so they can
  never collide.
- A type's kind is announced by its declaration form, never inferred from
  syntactic position (Spirit `3742`); the generic and trait/impl pipe forms
  extend this (Spirit `hh3z`, `bpyu`). Reaction frames are workspace-universal:
  the Work/Action types are declared once as generics and bound per component,
  not re-authored (Spirit `zjmc`). Code generation is build-time only (Spirit
  `9rjq`), and generated schema types are the source of every operation data
  type while handwritten Rust implements only behavior on those nouns (Spirit
  `5hjv`).
- Schema declares data types only — no effects, fan-out targets, effect tables,
  or storage descriptors (Spirit `hl1z`). Effects are runtime dispatch and logic,
  outside schema's lane; runner concurrency mode is likewise a runtime/deployment
  choice the public contract does not encode, and schema declares semantic
  constraints (ordering, idempotence, cancellability, read fan-out, single-writer)
  only when those semantics are real (Spirit `i9xk`). Schema files carry
  structure, not explanatory comments — declarations are self-explanatory through
  the type names they declare, and prose lives in the Rust implementation files
  (Spirit `bw9v`). Schema files use the `.schema` extension (Spirit `b0v3`).
- Schema namespace additions follow append-only discipline (new names at the end
  of the available namespace), and enum-slot planning minimizes database rewrites
  so future enum compilation and upgrade logic stays upgrade-compatible (Spirit
  `9yxh`). Components treat recompiling to change an enum set as trivial:
  zero-downtime upgrade is a design goal, so schema and enum changes are not a
  cost to design around (Spirit `uuh7`). Schema-defined execution consults
  current state and trusted owner/core authority messages before mutating
  storage; the single-owner model remains the race-avoidance discipline (Spirit
  `yngr`).
- Schema declaration is the source of truth that drives codegen, which produces
  not only the Rust types and impls but per-version namespace-slot-assignment
  tables and version-diff auto-marking types for upgrade migration on recompile
  (Spirit `mqlb`). Compilation auto-assigns slot numbers and applies size
  optimization, and the Rust macro becomes a consumer of structured schema data
  rather than a parser of Rust syntax. The direction is firm; the exact shape
  still needs iterative design.
- Streaming is full schema-derived push: a component subscription opens by
  delivering the current matching snapshot, then pushes every relevant change
  until the subscription closes, because a future-deltas-only subscription leaves
  clients without the current state (Spirit `brgo`). Schema-next gains an
  event/stream root with `opens`/`belongs` relations; schema-rust emits the
  event frame into signal-frame's streaming body plus an observable-set pub-sub,
  and the push action and subscriber registry live in triad-runtime. The default
  `SubscribePolicy` is `TerminateAtHandover`: subscriptions end at handover and
  clients reconnect to the next version.
- Schema-next keeps one lowering engine — the most correct lowering path, not
  dual paths or the smallest patch — and schema-derived Rust emission targets
  that engine rather than rewiring into the old signal macro implementation
  (Spirit `58bv`). The schema-derived stack (nota, schema,
  schema-rust, `signal-frame.schema`, `spirit.schema`) does not reference the
  separate Nexus NOTA-using vocabulary track; schema macros are plain NOTA records
  dispatched by position and shape in the schema `MacroRegistry`, not by the
  reserved NOTA sigils (Spirit `5mxn`).
- Every component repository gets a concept schema file starting at version 0.1,
  with Spirit, Orchestrate, Mind, and Persona as preferred pilots; schema files
  remain the source of data-type truth and may live beside Rust source when
  simplest (Spirit `ddlv`). The schema-codegen capability set is closed (types,
  generic-frame expansion, payload structs/newtypes/enums, standard impls,
  role/marker traits, shape-computed constants, method-bearing trait surface,
  actor wiring, plane carriers, opt-in `Deref`); the next phase is
  integration/migration, not language design — flip scalar-newtype impls
  default-on, integrate onto code-repo main, and port all components, with Spirit
  as a copyable contract-daemon/engine-stack exemplar rather than an all-in-one
  pilot (Spirit `t5wx`). Runtime binaries stay small, carrying only strict rkyv
  wire/storage contracts; the schema/NOTA compiler is build-time-only and never
  linked into runtime, and the NOTA text codec is an optional edge feature absent
  from the daemon. Creating new repositories with clean names is an acceptable
  option if branch names and old ancestry become too confusing — an option to
  evaluate, not an immediate rename directive (Spirit `neib`).

## Pipeline

1. `nota::Document` parses source into blocks.
2. `SchemaEngine` records the document's `StructureHeader`: a compact
   first-two-level witness emitted by the NOTA delimiter pass.
3. `SchemaEngine` validates the root object count.
4. `MacroRegistry` dispatches position-aware macros for imports, input enum,
   output enum, namespace declarations, struct fields, and enum variants. Its
   structural expectations are `nota` macro-node definitions: schema
   supplies schema positions and handlers, while nota supplies pattern
   matching, named captures, and no-match diagnostics.
5. `SchemaSource` lowers into `Schema`, the ordered semantic schema value used
   by Rust emission and schema upgrade logic.

## Authored Schema Source

`SchemaSource` is the current typed authored-language value produced after raw
`nota::Document` parsing. The target schema pipeline is that authored
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
When that root inline struct declares direct PascalCase fields, such as
`(Record { Topic String Description String })`, the fields `Topic` and
`Description` are exported too; later inline payloads and the trailing
namespace can use `Topic *` and `Description *`. Duplicate declarations are
rejected. This resolution happens on `SchemaSource` data, not by rewriting the
user's source string into the older `(Lookup RecordIdentifier)` pair form.

Enum variant entries in authored source are typed structural NOTA nodes.
`SourceVariantSignature` implements `nota::StructuralMacroNode` and uses
the same ordered `EnumVariants` structural cases as codec-facing
`nota::StructuralVariant` values: a bare PascalCase atom is a unit/header
variant, a parenthesized one-object block is a same-named data-carrying
variant, a parenthesized two-object block is the explicit different-payload
form, and the four-object forms `(Variant Payload opens StreamName)` /
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

Family declarations follow the same metadata precedent. The source form is
`FamilyName (Family { record RecordType table table-name key Domain })`; it
lowers to semantic `FamilyDeclaration` data on `Schema::families()` and is
excluded from `Schema::namespace()`. The family name is the stable identity of
a stored record family; the record name must resolve to a declared namespace
type, root enum, or import (`SchemaError::FamilyRecordNotFound` otherwise, on
both lowering paths); the `TableName` is only the current storage coordinate;
and the key kind is the closed `FamilyKey` structural keyword choice
`Domain | Identified`, mirroring a SEMA engine's keyed versus identified table
registration. Duplicate family names and duplicate table names are typed
errors. The family's version address comes from the existing content-identity
surface: `Schema::family_closure(record)` over the declared record type.

## Raw Core Schema Reading

`RawSchemaFile` is the bottom layer used to inspect a core schema before
schema lowering. It takes a path plus source text, derives the root type name
from the file stem (`core.schema` -> `Core`), parses the source with
`nota`, and requires one root brace object.

The input file is still `.schema`, and `.schema` must be legal NOTA. Tests
that prove schema-file behavior use real `.schema` fixtures and parse them
through `nota::Document` before the raw schema reader interprets the
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
in schema.

Tests prove the endpoint by asserting the Rust data directly and by
round-tripping the produced `Schema` through rkyv:
`Declaration::{visibility, name, value}`, `Visibility::{Public, Private}`,
`TypeDeclaration::{Alias, Struct, Enum, Newtype}` and
`TypeReference::{String, Integer, Boolean, Path, Plain, Vector, Optional, Map}`.
The semantic-schema text fixture surface stays removed: there is no checked
`.asschema` file and no hand-kept golden semantic-schema text.

`Schema` also carries its content identity. `Schema::content_hash` is the
blake3 hash of the schema's canonical rkyv bytes wrapped in `ContentHash`; any
edit to the semantic schema moves the address, and that address is the version
the version-control layer consumes. `Schema::family_closure` builds a
`FamilyClosure` for one named declaration or root enum: the root name plus
every declaration transitively reachable through type references — struct
fields, enum variant payloads, newtype/alias references,
`Vec`/`Map`/`Optional`/`ScopeOf` element references, stream-relation stream
declarations — each group sorted canonically by name. A reachable cross-crate
import contributes its stable identity (the local alias plus the
`crate:module:Type` import declaration), not the dependency's declarations.
`FamilyClosure::content_hash` hashes the closure's rkyv bytes. The two hash
kinds are domain-separated through distinct blake3 `derive_key` contexts, so a
whole-schema hash and a family hash can never collide. Both hashes are over
semantic values, never `.schema` text: formatting-only source differences
(whitespace, comments) produce identical hashes. Coverage boundaries: relation
declarations point AT declarations rather than being reachable from them, so a
relation edit moves only the whole-schema hash, never a family hash; and the
whole-schema hash covers the full semantic value including `SchemaIdentity`
(component name + authored version string) and resolved imports, so it is not
a pure-structure address — the family hashes are. Content identity lands
beside `SchemaIdentity`'s hand-authored version string; it does not replace
it.

Schema names emit through their own `Name` codec, not through the ordinary
`String` codec. A symbol-safe name is written bare (`Entry`,
`schema:spirit:Entry`) so declarations and references read as schema symbols;
only non-symbol names fall back to bracket-string text. Actual `String`
type-reference values still use the normal NOTA string surface at value
positions.

Schema-defined type trees are normally six or seven layers deep —
enum-of-struct-of-enum-of-struct and so on — so deep nesting is the normal shape,
not an exception, and the engine must traverse and emit through arbitrary depth
via canonical recursive walks with no ad-hoc per-layer handling (Spirit `3itj`).
The same types are referenced in many places throughout the tree through a TYPE
INDEX: a flat index of named types the tree refers into, so a type appearing in
many positions resolves to one canonical declaration rather than being
re-described at each site.

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

Both readings of that library are typed codecs over the same value. The
artifact projection decodes through the derived `NotaDecode` path, and the
bootstrap notation decodes through the derived `StructuralMacroNode` path:
`MacroLibrarySourceEntry` carries `#[shape(head = "SchemaMacro", body)]`, the
headed tail decodes as the `SchemaMacro` definition body, and inside that body
the position atom is a keyword structural node on `MacroPosition`, the pattern
and template decode through their own typed nodes, and the name is a schema
symbol. There is no positional record wrapper and no variant-name string
comparison; a malformed definition fails with the structural no-match
diagnostics. `MacroLibrary::to_source` writes the same bootstrap notation back
out, so decode -> encode -> decode is a fixpoint over both projections.

`MacroTemplate` is the typed expansion-template enum: `Type(TypeTemplate)`,
`Fields(Vec<MacroTemplateObject>)`, `Variants(Vec<MacroTemplateObject>)`, and
`Reference(MacroTemplateObject)`, with `TypeTemplate` carrying the `Struct` /
`Enum` / `Newtype` kinds. The template's output kind is part of its decoded
structure, so an unknown head is rejected when the library is read, and
expansion dispatches on the enum instead of matching extracted head strings.
The pattern and template payload objects (`MacroPatternObject`,
`MacroTemplateObject`) are leaf structural nodes: they mirror one NOTA object
of any delimiter shape with `$name` / `$*name` capture atoms, below the
variant-shape dispatch layer, the same way `SourceVariantName` is a leaf node
for the authored source codec.

The near target is to lower the core macro schema to schema data, emit its
Rust type, and replace the hand-written `MacroLibrary` noun with the
schema-emitted macro table type directly. The macro table is already real
serializable data and is already the runtime load path; the remaining loop is
making its Rust noun schema-emitted.

Declarative macro expansion preserves structural NOTA objects while lowering.
Pattern captures store the matched `Block` values, rest captures store ordered
`Block` vectors, and template payloads expand into owned object trees before
the typed `MacroTemplate` variant lowers the result. Compact notation remains
only as a diagnostic string for `MacroContext`; the live expansion path does
not emit a template string and parse it back through `Document::parse`.

The current structural expansion object can lower template-owned atoms and
delimiter nodes plus captured source blocks. Registry dispatch for arbitrary
type-reference macro invocations still operates on source `Block` values; the
fully shared version belongs with the nota macro-node substrate once it
exposes owned structural macro objects.

## Strict Key/Value Schema Syntax

The authored syntax preserves NOTA brace meaning: every brace is a key/value
map. Schema sugar may shorten values, but it must not turn a brace entry into
one logical declaration object.

- Root input/output positions are known by the schema reader and are written
  as bare bracket bodies: `[]`, `[Record Observe]`, or direct inline payloads
  such as `[(Record { Topic String Description String }) Observe]`. The root
  does not carry labels; position supplies `Input` and `Output`.
- Namespace braces contain `TypeName Value` pairs. `Topic String` and
  `Topics (Vector Topic)` are alias declarations; `Entry { topic Topic }` is a
  struct declaration; `Kind [Decision Correction]` is an enum declaration.
- A brace declaration with one field lowers as a newtype. `Entry { Topic * }`
  and `Wrapper { value Topic }` both describe one contained `Topic` reference;
  only a multi-field brace remains a named-field struct.
- Struct braces contain field-name -> type-reference pairs. `topic Topic` is
  explicit. `Topics *` derives the field name from an already-defined type and
  lowers to `topics: Topics`.
- Enum bodies are bracket/vector structure. Each object in that vector is a
  variant signature: a bare PascalCase symbol for a unit variant,
  parenthesized `(Variant)` for a same-named data-carrying variant, or
  parenthesized `(Variant PayloadType)` only when the payload name differs.
  A variant signature is one object, so the bracket remains a homogeneous
  vector of variant-signature objects.
- At root input/output positions, a bare PascalCase variant may resolve to a
  same-named exported namespace declaration. The source header says `Lookup`;
  the namespace says what `Lookup` is. Inline root declarations are also
  accepted and are inserted into the exported namespace before assembly; direct
  PascalCase fields inside root inline struct payloads become exported
  declarations available to later entries.

Composite type references such as `(Vector Entry)`, `(Optional Entry)`, and
`(Map Key Value)` still lower at reference positions to `TypeReference`
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
Its structural cases are `nota::MacroNodeDefinition` values: each case is
a serializable pattern over atoms, delimiters, literals, rest captures, and
named captures. Schema-next contributes the schema `MacroPosition` and the
handler that turns a match into an `Schema` fragment; nota owns the
shape matcher.

The namespace declaration node makes the strict brace model executable:

```nota
Entry { Topics * }     ; symbol key + brace value -> struct declaration case
Kind [Decision]        ; symbol key + bracket value -> enum declaration case
Topic String           ; symbol key + reference value -> alias declaration case
```

`KeyValueDeclarationMacro::matches` delegates through the nota pattern
registry instead of merely checking "is this a pair." When no registered macro
matches at a node position with known cases,
`SchemaError::UnsupportedMacroNodeStructure` reports the schema position,
expected macro-node cases, and actual shape using the nota no-match
diagnostic. Type-reference positions retain their existing
`UnknownTypeReferenceForm` path so unknown collection heads remain precise.

Schema-next is now a consumer of the NOTA-layer macro mechanism for structural
cases. Delimited captures from nota expose inner `NotaBody` streams, and
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
  - field 1: input root enum body, for example `[Record Reindex]`
  - field 2: output root enum body, for example `[Recorded (Rejected Rejection)]`
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
  reference defines an alias: `Topic String`, `Topics (Vector Topic)`.
- Square brackets are NOTA vector/bracket structure. At enum-body positions
  they contain homogeneous variant-signature objects: bare symbols for unit
  variants, parenthesized `(Variant)` records for same-named data variants,
  and parenthesized `(Variant PayloadType)` records only when the payload type
  name differs.
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
  `schema-rust` can emit a `pub use` alias and keep one type identity
  across the crate boundary.
- `TypeReference` at a reference position is an enum:
  `String`, `Integer`, `Boolean`, `Path`, `Plain(Name)`, `Vector(Box<TypeReference>)`,
  `Map(Box, Box)`, and `Optional(Box<TypeReference>)`. `String`, `Integer`,
  `Boolean`, and `Path` are reserved scalar leaves, so they are not user namespace
  declarations and cannot be shadowed by schema types. `Plain(Name)` now means
  "a declared type by name." `TypeReference::from_block` lowers a bare scalar
  symbol to its scalar variant, a different bare PascalCase symbol to `Plain`,
  `(Vector T)` to `Vector`, `(Map K V)` to `Map`, and `(Optional T)` to
  `Optional`. These names are Schema type-reference vocabulary over
  nota's already-parsed structures, not raw NOTA keywords. The inner
  positions recurse, so `(Vector (Optional Topic))` and
  `(Map String (Vector Service))` nest. Parentheses with another head are
  dispatched to the user macro registry. An unknown head or wrong native
  argument count is a typed `SchemaError::UnknownTypeReferenceForm`. Lowering
  is pure semantics over nota's already-parsed blocks — not a hand-rolled
  text parser.
- Collection references reach every reference position. Struct fields are
  written as strict pairs such as `serviceVector (Vector Service)`,
  `byTopic (Map Topic RecordIdentifier)`, and
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
- `UpgradeObject` in `src/upgrade.rs` is the schema-change unit:
  `{ previous_identity: SchemaIdentity, next_identity: SchemaIdentity, edits: Vec<SchemaEdit> }`.
  `SchemaEdit` is a closed enum `AddField | ChangeFieldType | AddVariant`.
  `UpgradeObject::apply` runs edits in order; identity mismatch rejects with
  `SchemaError::SchemaEditIdentityMismatch` (a typed error, not a string log).
  The return is an `UpgradeReceipt` carrying the identity transition plus each
  edit's typed per-edit receipt — typed receipts, not string logs.
- Schema lowering is single-path: there is exactly one set of lowering semantics
  (the typed `SchemaSource` path). The document entry point reparses a `Document`
  into `SchemaSource` and lowers that; no second hand-mirrored lowerer exists.
  This means a document and its `SchemaSource` cannot lower to different schemas,
  so the verbose explicit-payload source form and the bare-name form are
  byte-identical after lowering. Tests in `tests/typeref_structural_macro.rs`
  witness the byte-identity property for `TypeReference` canonical forms.

## Syntax Schema Layer

`SyntaxSchema` is the typed layer directly above `RawSchemaFile`. It reads the
strict key/value authored surface without invoking macro lowering:

1. `RawSchemaFile` parses a `.schema` file as legal NOTA and preserves raw
   delimiter objects.
2. `SyntaxSchema` reads the raw datatype map into declaration objects.
3. Brace values become struct field maps; square-bracket values become enum
   bodies; atom or parenthesized reference values become aliases.
4. `(Vector T)`, `(Map K V)`, and `(Optional T)` are Schema type-reference
   objects and lower into composite type references.
5. The root map key is the declaration name. There is no second declaration
   name inside the value.

The proof fixture is `tests/fixtures/syntax-layer/schema.schema`; the tests in
`tests/syntax_layer.rs` assert the raw-to-syntax result directly.

## Generics, Traits, and Component Code Generation

This is the construct vocabulary that lets the schema GENERATE component code
instead of hand-wiring it. The CONSTRAINTS below are settled (what we want); the
implementation of several pieces is deliberately OPEN (noted), so this section
states the shape we are aiming at without pinning the exact mechanism.

### Constraints (settled)

A type's kind is explicit on its declaration form, never inferred from syntactic
position (Spirit `3742`). The closed delimiter set carries every construct:
`Name { … }` is a struct, `Name [ … ]` an enum, `Name <ref>` a newtype, and the
two reserved pipe forms carry the constructs the positional forms cannot —
`Name (| [params] body |)` is a GENERIC declaration (Spirit `hh3z`; the inner
`[…]`/`{…}` selects enum/struct), and `{| … |}` is the TRAIT/IMPL construct
(Spirit `bpyu`). A generic's parameters and body live together inside the
`(| |)` so the binders scope the body structurally — there is no key/value
side-channel threading binders from a separate object. A generic is USED by the
flat `(Head Arg…)` application form, name-resolved against its explicit
declaration exactly as `(Vector T)` is known; use-site name resolution is
legitimate, not guessing.

Reaction frames are workspace-universal (Spirit `zjmc`): the Work/Action types
are declared once as generics and BOUND per component by applying them at the
component's roots. Re-authoring them per component is a design failure.

Code generation is EXPANSION, not generic-alias: a component root is expanded by
positional binder→argument substitution into a concrete enum with empty
parameters, which flows through the existing concrete-enum emitters
(constructors, `From`, accessors, rkyv/NOTA codecs) for zero new machinery. The
genericity does not persist as a `pub type Input = Work<…>` alias in the
component's output — each component owns a concrete interface. Generation is
build-time only (Spirit `9rjq`).

The data / hand-written boundary (Spirit `5hjv`): generated schema types are the
source of every operation data type; handwritten Rust implements only BEHAVIOR
on those generated nouns. A method body is itself data — an expression tree over
the application form (Spirit `4itr`/`7c71`) — but only for the marker case and a
small FIXED, named mechanical family (payload projection, the auto-emitted
constructors, `From` legs, accessors). Genuine business logic (the decision
plane, the store, the guardian, the signal-query matchers) stays hand-written;
arbitrary Rust expression bodies are never modeled as data.

The generic-substrate fence: the four-plane separation of authored `.schema`
source artifact (`SchemaSourceArtifact`), semantic schema value (`Schema`),
Rust emitter (`schema-rust`), and durable SEMA storage survives any generic
lift. A `SerializableArtifact<T>` or `SemaStore<T>` abstraction must not
collapse these planes by merging source-artifact, semantic-schema, and emission
concerns into a single parameterized type. Each plane has its own ownership
contract and must remain independently evolvable.

### Open (how — deliberately unsettled)

- The `fn` / signature sub-construct for trait DECLARATIONS: signatures datify
  cleanly, but default method bodies are real Rust — the inside of `{| |}` for a
  trait declaration is recognized but not designed.
- The `{| |}` impl shape detail: the optional-slot semantics
  (`{| [params]? Trait Target [body]? |}`, shape-discriminated, no keywords),
  where-clauses, and whether impls are a `{| |}` object or a top-level
  `(Impl …)` section are not finally reconciled.
- Generic impl-header emission `impl<P..> Type<P..> where P: Bound` plus a
  parameter-bound vocabulary is genuinely NEW codegen, not a free consequence of
  generics-as-data; until it lands, parameterized-declaration inherent impls stay
  suppressed.
- The named mechanical-body family beyond payload projection is not enumerated.
- Role→trait binding: an explicit data table versus the current structural
  inference from variant names is undecided.

The generics leg (declaration, use, expansion) is prototyped and proven green;
the trait/impl leg is designed-but-not-integrated. Per Spirit (low-certainty
design record, review certainty as integration proceeds): this section's OPEN
items are the parts still being settled.

## Runtime Planes the Schema Describes

The same four-position schema shape (Imports, Input, Output, Namespace) describes
three runtime planes, each its own language sharing that shape and the same
single-colon import/export mechanism; the languages differ only in which types
and macros fit each position (Spirit `2v9u`, `rmv8`). The three planes are
Signal (the wire/message plane), Nexus (the execution-IO plane for internal
effects, external calls, and UI panels — and the keeper of in-flight mail state
between Signal ingress and SEMA replies), and SEMA (the durable-state plane).
Each plane has its own engine and traits but shares one pattern: run code over an
input message, return an output message. Rust emission splits the planes into
separate namespaces so payloads use plain `Input`/`Output` names without plane
prefixes (Spirit `7118`, `rmv8`).

A schema document's root is a data-carrying `Plane` enum with `Signal`, `Nexus`,
and `Sema` variants carrying the actual plane messages, so runtime code matches
directly on the plane rather than pairing a thin kind tag with a separate
envelope (Spirit `ugig`). The schema type itself — which plane a schema declares
— is a `Kind` enum (`schema::Kind`), each variant carrying that plane's
four-position body, giving the three typed engines their dispatch point and
mirroring the SEMA runtime envelope at the message level (Spirit `z9kv`; the
`Kind`-versus-`Plane` naming is still open). Engine trait signatures type-check on
plane membership: a signal-plane message cannot pass where a nexus or sema
message is expected, so the trait chain cannot be mis-wired on either the order
or the plane axis.

A component schema reads as a sectioned three-part structure (parts two and three
optionally recursive): SPECIFYING (imports and exports / name-sharing), INPUT
(operations and queries received), and OUTPUT (responses emitted), each part's
first sub-part a header or derived from the assembled schema (Spirit `rmqo`). Per
component the struct order is optional metadata/imports, then short headers
(regular signal, owner signal, SEMA operation) each a vector of variants, then the
namespace map, optionally a final extension vector; the top-level outer vector
holds distinct typed sub-vector sections, not one flat mixed vector. Header
declarations come first and drive receive/dispatch triage, are derivable from the
actual assembled data type rather than authored separately, and usually begin with
an enum-like variant decision because the first structural match creates the
routing namespace, mirroring the binary signal header (Spirit `m76h`). Every
PascalCase name in a schema header — Input/Output root bodies, `NexusWork`,
`NexusAction`, `SemaWriteInput`, `SemaReadInput` — becomes an exported top-level
type importable by library consumers, so the header IS the component's public
type-level interface: each variant is a named, defined, exported contract verb or
reply (Spirit `w6y1`). Under the current macro grammar a double-wrapped
`((Parse Expression) (Render Expression))` Input/Output form reads its heads as
macro/operator heads, not enum variants, so that form is denied as schema grammar
until a distinct enum-variant context or explicit enum form is established (Spirit
`oe6s`).

Most schemas are plain; a component's top-level schema is reactive. A plain BASE
schema (root struct plus a namespace of type definitions) says what data IS; the
REACTIVE schema extends it with the reaction surface — input in, nexus through,
sema state, output back — and says how data MOVES, analogous to Nix's built-in
derivation extended by higher-level builders (Spirit `mimk`). A loaded file's
root kind (base versus reactive) is known a priori from the load context
(extension/filename), like a Nix derivation, so the file supplies only positional
values at known slots with no root, field, or type names; a base file is just its
namespace, a reactive file is bare input enum, output enum, namespace, with no
`Input`/`Output` labels even in `root.schema` (Spirit `l1ip`).

### Engines and the request pipeline

The daemon has three execution centers whose traits encode order (Spirit `o8x5`,
`str0`). Each engine trait's method count matches the number of distinct wire
events its plane handles:

- `SignalEngine` (messaging; the first gate) has two methods: triage on input
  arrival, which rejects invalid messages back to the client, and reply on output
  emission.
- `NexusEngine` (the mail keeper and translator that holds being-processed mail
  and converts Signal to and from SEMA) has one method: execute, a synchronous
  compute step. Nexus may host an internal recursive engine for future runtime
  control.
- `SemaEngine` (state) has two methods: apply for writes and observe for parallel
  reads.

Push only on success; push methods take the next engine as a parameter,
sequencing Signal to Nexus to SEMA and then reversing. The full request pipeline
is: Signal triages wire input, Nexus computes, SEMA performs the durable op,
Nexus receives the SEMA reply and decides next steps, Nexus returns the reply to
Signal, and Signal frames the reply to the wire client (Spirit `ooxy`). It is
asynchronous throughout, and rolling origin identifiers thread through the whole
pipeline so each layer routes responses back to the correct waiter, including
partial SEMA responses for multi-op work.

### Effect table and the match-matrix surface

The schema declares an EFFECT TABLE: a closed mapping from message types to
effect types, with replies declared per message (Spirit `udjq`). Dispatch is
structural match through the interact-trait contact point — message to mapped
effect to mapped reply — so effects, messaging, and interactions are all mapped
and internal work composes them: match always, map always, match rather than
compute. When a match between two domains has no defined behavior in a cell, the
response is a typed error or help message (`Unavailable`/`Unauthorized`/
`NotImplemented`) that lives in the domain enum as part of the trait surface, so
the caller can match on it rather than hit an exception or unhandled default
(Spirit `xiqa`). The error surface is part of the trait surface.

The current target assumes at most seven root data-carrying variants for a
component surface; substantially more is pressure to split the component or
surface (Spirit `ujb2`).

## Contracts, Channels, and the Component Triad

A contract is a channel: each channel gets its own schema, so one component has
multiple `.schema` files — one per channel — superseding one-schema-per-component
(Spirit `26e7`). A component is at least three plane schemas, and major actors get
their own schema; every part worth describing gets a schema the psyche can
inspect and interact with, and signal-frame itself is schema.

Components have two schema categories (Spirit `nm97`). EXTERNAL schemas describe
surfaces outside daemon runtime: wire contracts to other components (the
signal vocabulary surviving the socket boundary) and database/storage contracts
(state surviving process exit). INTERNAL schemas describe surfaces within runtime:
actor message vocabularies and inter-actor channel contracts sharing process
memory. Each schema describes one channel, and internal-schema changes break
neither database nor wire.

The component triad splits accordingly (Spirit `f8ds`, `l6zw`, `26e7`). The
client-facing Signal types live in the `signal-<component>` contract repo as the
canonical wire vocabulary — only Signal `Input`/`Output` roots, their record
types, and the wire codec — and the daemon imports them. Nexus and SEMA
interfaces live in the daemon repo as runtime-internal `.schema` files (for
example `cloud/schema/nexus.schema`), each with its own imports/exports/namespace
and importing the wire Signal IO; they are not separate crates or repos.
`SignalEngine`/`NexusEngine`/`SemaEngine` live in the daemon, which imports the
contract `Input`/`Output`; clients send and receive only Signal messages. SEMA
may extract to a contract repo when a daemon gains a scale-out database. The
schema type/macro-substrate library may co-locate in the triad or sit in a
dedicated schema-types repo consuming the same compiled NOTA artifact — only
crate layout differs.

Every component derives two vocabularies from one `.schema` file (Spirit `c8b3`):
an internal effect language for engine-internal actors over each other and
storage (the actor-mediator surface), and an external wire language for signal
contracts via signal sockets (signal-frame envelope, `ShortHeader`, dispatch).
The `signal_channel` macro emits the wire surface, the SEMA operations, and the
SEMA lowering operations together.

Schema files are strict typed component interface contracts (Spirit `xbc2`):
their input and output roots plus imports and namespace define the messaging/API
surface the engine implementation must use, forcing agents and generated code to
communicate through schema-emitted objects rather than ad hoc messages. The root
type of a schema is the message surface — what is sent — and each schema declares
its own input and output enums and uses colon-path namespace imports for shared
types (Spirit `fhe8`). Common workspace identifiers (record, forward, send) are
stored as enum-encoded composite names so the wire form is the discriminator, not
the string, making the namespace composable across components and enabling
multilingual labels (Spirit `8u1o`). Schema variant namespaces reserve explicit
numeric ranges so the encoded form reflects the logical split before deeper
payload parsing: Input and Output are partitions of one wire tag space, so a
single tag byte identifies both the variant and its direction — the basis for
input/output dispatch at the message layer (Spirit `fry8`).

## Schema as a System

The schema component is a full component triad (Spirit `xbu8`). Its daemon loads
a schema environment from a manifest that selects the module versions the import
system uses; resolves schemas, namespaces, core definitions, and macro libraries
from that environment; parses source files with source-map awareness of where
each block begins and ends and what type it has; and serves as the extensible
schema language-server surface for inspection, editing, code generation, upgrade,
and future schema-aware features. The library and macro faces consume the same
environment and schema values rather than maintaining a separate interpretation.

The workspace ships a library of precompiled schemas (Spirit `uzxp`). A standard
namespace — the core — is always implicitly loaded and contains the macros and
built-in types; other schemas load as needed. Precompilation means schemas do not
re-parse at every interpretation site; they live as in-memory namespace tables,
and the precompiled-schemas surface is what emitters and interpreters consume.
The schema daemon is the runtime arm of that library: it resolves and caches
schemas in memory, owns the namespace surface, and is the single point through
which agents and code emitters resolve namespace references, so all cross-schema
resolution flows through it (Spirit `cbtg`). Built-in core macros live in the
basic schema library and are always imported when a schema imports the schema
module — no opt-in — while user-defined macros are lazy-loaded by name from
explicit imports: core macros are non-negotiable substrate, user macros are
extensions that compose with the core (Spirit `wx5c`).

The schema-derived runtime supports an optional testing/instrumentation build
surface where generated objects and engine-interface usage emit structured trace
events to a logging socket, proving actual runtime use of the Signal, Nexus, and
SEMA interfaces during tests rather than mere symbol existence (Spirit `xqkv`).

Schema-stack presentations, tests, and design reports show the schema-to-interface
path: each component interface, the schema that creates it, the path to the
generated interface and component communication, plus the derived code, typed
interfaces, traits, impls, and derivation boundaries where flaws can be spotted
(Spirit `hckx`). Schema examples should include multi-variant headers, not only
one-element vectors (Spirit `h9xd`).

## Macro Dispatch and Composition

Macros are sugar in the schema layer (Spirit `506w`): the brace-enum form expands
to the canonical paren-list form, both lowering to the same assembled schema,
where variants are already `(VariantName payload-option)` records in a
homogeneous vector. The macro engine dispatches on multiple criteria — delimiter,
internal shape, root-object count, symbol qualification, and combinations —
choosing the most specific; the macro shape itself is two-position, a pattern it
matches and a template for how it expands. Enum-body vectors honor homogeneity:
each element is one variant declaration, a bare PascalCase symbol (unit) or a
parenthesized `(Name Type)` record (data), and the older interleaved at-suffix
form is retired as dishonest.

Macro dispatch is two-phase (Spirit `pul9`): a structure-match macro first asks
what shape the `NotaValue` is (enum short-syntax? struct? newtype?), then the
matching per-shape transformation macro applies its lowering — they compose
because the same syntactic placement can carry different shapes. Each macro
carries its own schema-reading logic, and this is the schema language's extension
mechanism: new shapes arrive as new macros enter the precompiled core or
per-schema imports. Schema macro resolution maps nodes onto a closed macro space
whose variants are the built-in type forms (`Enum`, `Struct`, `Vec`, `Option`,
`KeyValue`, `Newtype`) via dual dispatch (Spirit `rfg9`): look at the head keyword
first and dispatch reserved built-ins directly, skipping structural mapping;
otherwise resolve a user type declaration by structural shape. This adds a keyword
fast-path over the structure-and-position `MacroRegistry`.

Macros compose from reusable micro-macros — small composable units named in
specific positions within larger macros, such as an enum-short-syntax applier, a
struct-short-syntax applier, or a structure-match dispatcher (Spirit `d6if`,
`qe84`). When you see repetition, extract it into a library or the schema macro;
prefer pushing logic back into the schema macro, because the more the schema
generates the better, as long as it stays clear and readable and shows the logic
as object shapes (Spirit `xprx`). The lowering engine producing assembled schema
is built from reusable data-carrying macro-variant lowerers at node-definition
points: each macro-schema variant carries a struct defining its input type, and at
a node-definition the built-in engine dispatches by shape to the matching variant,
which consumes its input struct to produce lowered output — extensible by adding
variants plus their input structs, not by hard-coding shape rules (Spirit `sd7x`).

The bootstrap core schema (the schema-schema) is implemented in ordinary Rust
first and exposed as the macro interface for writing schema macros (Spirit
`sanf`): it names macro inputs by type — a macro's input type IS its name — so
macros inspect NOTA block structure and emit assembled-schema data, and people
build their own macros against this core. Every schema file is loaded with the
schema-schema attached implicitly; built-in macros bootstrap expansion before the
user-declared macro space expands.

Two open macro-frontier questions are recorded but not settled. The enum-versus-
macro-invocation ambiguity on parenthesized heads — a collection macro on one
element type and a two-variant enum are structurally identical parens at the same
position — needs an explicit discriminator (a reserved operator-word set, a sigil
on operator heads, or moving collections onto suffix sigils plus brace, leaving
the paren pure enum); only the paren is doubly loaded, struct bracket and map
brace being unambiguous, and which discriminator wins is open (Spirit `b0s4`).
Macro bodies also need an explicit binding-and-reference mechanism for assigned
symbols, with the firm constraint that references stay visible in the schema
language (a `$`-style sigil is a candidate), plus single-object sigil sugar where
a leading sigil lets one NOTA object stand for a larger pair or field expression
when the macro case is obvious (Spirit `e8iu`).
