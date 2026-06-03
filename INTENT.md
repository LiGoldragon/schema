# Intent

`schema-next` is the new schema implementation.

Psyche intent:

*The schema-derived stack uses separate repositories for nota-next,
schema-next, and schema-rust-next rather than one combined integration
repository.*

*Macros lower schema positions into assembled schema. The lowering logic must
know where the node appeared; the same delimiter shape can mean different
things at different macro positions.*

*Schema macro expansion is structural matching over NOTA objects at macro
positions. The delimiter shape, number of contained objects, and symbol
qualification predicates select which macro variant lowers the object into
assembled schema.*

*Brace namespace and enum forms are sugar implemented as macros. A brace object
is a key/value map at the NOTA layer; when a schema position supplies the
surrounding type/field name, the brace body can lower to enum-like assembled
schema without repeating that name in the authored source.*

*Schema files are read as a known root struct. The top-level schema shape
supplies the root type name and field positions; inner variable vectors contain
macro objects that expand by position into assembled schema.*

*A macro invocation is itself data: a tagged/data-carrying schema node at a
known macro position. A parenthesized node such as `(Normalize [Topic])` has a
tag (`Normalize`) and a raw vector data payload (`[Topic]`); that raw vector is
macro payload data, not the `Vec` type constructor. Macro definitions and macro
calls should be represented as data-bearing structs and enum variants before
execution so the macro table can be serialized, deserialized, tested, and
eventually pre-assembled.*

*The NOTA delimiter pass emits a compact first-two-level structure header.
Schema lowering records that header before macro dispatch so textual schema
triage and binary signal-header triage follow the same structural idea.*

*Schema-created base enums define the reaction and action types for actor
input and output. Execution matches those generated variants to select replies,
route work, and drive signal/executor/SEMA behavior.*

*Schema files create the data types of the libraries they serve. Signal,
Nexus, SEMA, upgrade, and mail-event vocabularies should be declared as schema
objects first, then implemented as methods or traits on the generated Rust
nouns. Nexus is the execution-IO schema plane for internal effects, external
calls, and UI surfaces; it replaces the older executor wording in the runtime
triad.*

*Signal, Nexus, and SEMA schemas are symmetric at the authored-schema level:
each has imports/exports, input, output, and namespace. Import/export paths use
the single-colon namespace that mirrors Rust modules. Schema-next lowers the
same structural shape for all three; later runtime layers decide whether the
plane communicates, executes, or owns durable state.*

*Nexus owns in-flight mail state. Signal schemas describe what crosses the
wire, SEMA schemas describe durable state commands and replies, and Nexus
schemas describe the execution action that holds mail while it is being
processed between those two planes.*

*Async mail is actor-object flow. Schema-next's job is to preserve the objects
that later code will act on: sent message events, Nexus mail, SEMA work, SEMA
replies, and processed message events. A schema lowering that collapses those
objects into untyped procedural steps has lost intent.*

*NOTA owns raw structure and serialization shapes; Schema owns the type-name
vocabulary. Square brackets remain raw NOTA vector structure and schema field
lists; they are not the syntax for declaring a `Vec` type. Schema type-reference
objects include `(Vec T)`, `(Map (K V))`, and `(Optional T)`, lowering to
`Vector`, `Map`, and `Optional` in assembled schema, alongside scalar
type-reference names such as `String`, `Integer`, `Boolean`, and `Path`. The inner
positions recurse, so `(Vec (Optional Topic))` and
`(Map (NodeName (Vec Service)))` nest. Collection references appear at every
reference position: struct fields, enum-variant payloads, root input/output
variant payloads, and import sources. User-declared macros may still appear at
type-reference positions, but built-in type-name keywords are Schema
vocabulary, not raw NOTA vocabulary.*

*Reserved scalar pass-throughs belong in assembled schema, not only in Rust
emission. `String`, `Integer`, `Boolean`, and `Path` lower to scalar `TypeReference`
variants; `Plain(Name)` is reserved for declared schema types and importable
namespace names. Scalar names cannot be re-declared in a schema namespace.
`Bool` is not a scalar spelling, and `Text` is not required as a scalar floor
right now; a schema may still declare `Text` as its own string newtype when it
needs that domain noun.*

*Cross-crate schema imports are resolved through Cargo-exposed dependency
schema directories, not duplicated locally. A schema import source uses the
single-colon path `crate:module:Type`; `ImportResolver` loads the dependency's
schema module, confirms the type is declared there, and records a resolved
import so Rust emission can reference the dependency type by alias.*

*Assembled schema is defined before authored-schema sugar. `Asschema` is the
typed macro-free endpoint produced by lowering real `.schema` files, and it is
itself live data: it must read/write legal NOTA through the shared NOTA codec
and read/write binary rkyv bytes. Rust emission consumes that assembled data
object, not hidden parser state or hand-kept assembled-schema text fixtures.*

*Asschema names are schema symbols, not ordinary string text. A single name
string that qualifies as a NOTA symbol candidate must emit as a bare symbol
(`Entry`, `schema:spirit:Entry`), not as a bracket string (`[Entry]`). Bracket
strings remain the fallback for non-symbol text and for actual `String`
values.*

*Asschema notation must truthfully represent the underlying data shape. A
homogeneous vector is only used for one element type, never to hold different
schema positions such as input and output. Semantically empty wrapper records
are not introduced to make a representation fit; root input and output are
known product fields of `Asschema`, while namespace declarations remain a
homogeneous vector of declaration objects.*

*The canonical `.asschema` text artifact is read as the known `Asschema` root
struct. It therefore writes the root fields as document root objects, not as one
outer parenthesized record. The input and output positions are known fields and
serialize their enum bodies directly; they are not labeled data-carrying
variants named `Input` or `Output`.*

*Known-root `.asschema` reading goes through the NOTA body codec, not through
ad hoc field-string joins. `Asschema` derives the body-aware NOTA codec with
`#[nota(known_root)]`; schema-next only supplies the semantic projection for
the named input and output enum fields. NOTA owns the body parse/format
boundary while schema owns the field semantics.*

*The assembled schema artifact is a first-class data object. `AsschemaArtifact`
wraps an `Asschema` value and owns the read/write methods for `.asschema`
NOTA files and `.asschema.rkyv` binary files, so downstream Rust emission can
consume a serialized assembled-schema artifact instead of relying on a private
in-memory handoff from schema lowering. Core schema artifacts that define the
schema substrate itself are checked into the repository and freshness-checked
against their source schema so review sees authored schema, assembled schema,
and emitted Rust as separate stages.*

*Asschema can persist through SEMA storage as the same typed object. The store
surface writes rkyv-archived `Asschema` bytes keyed by schema identity and
re-exports the recovered typed object as `.asschema` NOTA through
`AsschemaArtifact`, so durable storage, binary archive, and text artifact stay
projections of one value.*

*A core schema file can be read one layer lower than schema lowering: as raw
NOTA object data. In that mode the root struct name is derived from the
filename, mirroring Rust modules, and the file does not restate that root
name. The root body is a native brace key/value map of datatype names to raw
NOTA objects.*

*.schema files are NOTA documents. Every `.schema` fixture used to prove a
schema-language behavior must first be legal and parseable by `nota-next`
before any schema-specific lowering or raw-schema reading is applied.*

*Authored schema source is its own typed value before assembled schema.
`SchemaSource` reads a `.schema` document into source-language data — imports,
input root enum, output root enum, and namespace declarations — and writes a
canonical `.schema` text projection back out. This source codec is separate
from raw NOTA parsing and separate from `Asschema` serialization: raw NOTA
preserves delimiter truth, `SchemaSource` preserves authored schema meaning
and sugar, and `Asschema` is the macro-free assembled program.*

*NOTA delimiters keep structural meaning before schema applies semantic
expectations. Square brackets are bracket/vector structure at the raw layer,
but a schema position typed as `String` or a string newtype may read that
bracket form as text. Parentheses are raw record/struct structure; they become
tagged schema nodes only when the expected type is `SchemaNode` or another
tag-plus-payload struct.*

*Authored schema braces are strict key/value maps. Namespace braces contain
`TypeName Value` pairs, not one-object declarations; struct braces contain
`fieldName TypeReference` pairs. A PascalCase key followed by `*` is the
derived-member shorthand: `Topics *` lowers to field `topics` with type
`Topics`. Root input/output positions are known by the schema reader and are
written as bare bracket bodies such as `[Record Observe]` or `[]`, never
as labeled root wrappers. Square-bracket namespace values define
enum bodies; brace namespace values define struct field maps; atom or
parenthesized reference values define aliases (`Topic String`,
`Topics (Vec Topic)`). Parentheses remain the composite/type-reference and
macro-call argument form (`(Vec Entry)`, `(Optional Kind)`,
`(Map (Key Value))`). The default parser accepts only this strict authored
surface; declaration forms that repeat their own name are removed from the
production lowering path.*

*Enum bodies are homogeneous vectors of variant-signature objects. A unit
variant is a bare PascalCase symbol, and a data-carrying variant is a
parenthesized record `(Variant PayloadType)`. This keeps each bracket member
one variant-signature object instead of smuggling key/value rhythm into a
vector delimiter.*

*Root input/output headers may list exported variant object names directly.
When a bare root variant name resolves to a declaration in the schema namespace,
the variant carries that same-named payload type. This lets the header read as
the component's exported operation vocabulary (`[Lookup Count]`) while the
namespace defines what each operation object is (`Lookup RecordIdentifier`,
`Count Query`). Root header entries may also define the payload object inline;
the inline declaration is inserted into the exported namespace before the root
enum is assembled.*

*Schema source lowering is allowed to be multi-pass over a block. The source
reader first preserves the authored objects, then collects candidate type names
from namespace entries and inline root declarations, then resolves variant
payload shorthand against that namespace before producing assembled schema.*

*Assembled schema namespace entries are visibility-tagged data objects. The
canonical NOTA shape is `(Public Name Value)` or `(Private Name Value)`, with
the first payload field carrying the declared name and the second payload field
carrying the alias/struct/enum/newtype value. Top-level authored declarations lower
to public declarations. Inline PascalCase declarations lower to private,
module-local declarations, derive their field name from the type name, and may
be referenced later in the same module.*

*A struct declaration's assembled value is a key/value map from field name to
type reference. The implementation may preserve source order internally for
Rust field order and rkyv layout, but the semantic object is a brace map:
field key -> type-reference value.*

*An alias declaration's assembled value is an exported name for an existing
type reference. Bare namespace bindings such as `Rejected SignalRejection`,
`Topic String`, and `Topics (Vec Topic)` lower to `TypeDeclaration::Alias`,
not to tuple newtypes. This preserves the symbol path and documentation/help
identity without forcing Rust consumers to wrap and unwrap a semantically
identical payload type.*

*A newtype declaration's assembled value is a single contained type reference,
not a one-field map with an invented field name. Authored brace bodies that
lower to exactly one field become `TypeDeclaration::Newtype`, and Rust
emission treats them as real tuple newtypes.*

*A struct body that lowers to one field is a newtype, not a named one-field
struct. Field names are derived only when there are multiple fields to access;
single self-named members such as `Entry { Topic * }`, explicit one-field
wrappers, and inline PascalCase one-field declarations all lower to
`TypeDeclaration::Newtype`.*

*The earlier declaration forms that repeat their own name are removed from the
default parser and macro engine. Explicit macro-library data can still describe
arbitrary NOTA patterns for experiments, but the authored schema path is strict
key/value syntax.*

*Macro nodes are first-class structural expectations, not just prose around
registered Rust macros. A macro node definition carries named cases with
position, input object kind, delimiter constraints, object-count constraints,
key qualification rules, and value-shape rules. For namespace declarations,
the key/value pair itself is the semantic macro-node object: symbol+brace
matches a struct declaration, symbol+bracket matches an enum declaration, and
symbol+reference matches an alias declaration. Unsupported shapes should fail
as unsupported macro-node structure with the expected cases listed, not as
opaque parser magic.*

*The authored macro library is typed as a vector of source-entry enum
variants. A top-level `(SchemaMacro ...)` record is
`MacroLibrarySourceEntry::SchemaMacro(SchemaMacro)`, carrying the same
`SchemaMacro` payload type in source and in the serialized artifact; it is not
an untyped sentinel string checked by parser glue and it is not split into a
separate source/data enum pair.*

*The long-term macro-node mechanism belongs at the NOTA layer so other
consumers can reuse it. Schema-next may host the bootstrap structural cases
while the stack converges, but the target split is: nota-next owns structural
macro-node dispatch and typed matches/captures; schema-next registers the
schema vocabulary and lowers matches into assembled-schema fragments. Built-in
schema macros load through a serialized macro-library data artifact, with the
hand-authored macro source kept as a freshness-checked bootstrap source rather
than as the runtime path.*

*Schema-next's structural macro-node cases are expressed through nota-next
macro-node data. Schema still owns schema positions such as
`NamespaceDeclaration`, `StructFields`, and `EnumVariants`, but the accepted
shapes are `nota-next` patterns with named captures. Schema-next remains the
semantic consumer that lowers those matches into assembled-schema fragments.*

*Declarative macro expansion keeps matched NOTA structure as data through the
schema lowering path. Atom captures remain `Block` values, rest captures
remain ordered `Block` vectors, delimited captures expose the matched
delimiter's inner body stream, and template expansion lowers an owned
structural object tree instead of producing text that is parsed back into
blocks.*

*At reference positions, parenthesized bodies are resolved at the
assembled-schema reference layer: recognized type-reference heads such as
`Vec`, `Optional`, and `Map` remain composite references, and future user macro
heads can use the same argument shape. Enum declarations use namespace
key/value pairs with square-bracket values, so declaration shape and
composite/reference shape are no longer overloaded. Unnamed composites used as
fields may derive names such as `vec_of_entry` / `VecOfEntry` when there is no
conflict.*

This repository owns the schema macro engine and the ordered assembled schema
data model. It does not emit Rust source code.

Current implementation target:

- Macros are registered in `MacroRegistry`, including nested struct-field and
  enum-variant macros.
- The engine lowers root schema sections through the registry, not through
  hard-coded macro fields.
- A macro consumes a typed `MacroObject` at a `MacroPosition`; namespace
  declarations are pairs, while root sections are blocks.
- The context records which macros ran as diagnostics. Correctness tests prove
  lowering through the produced `Asschema` data, not by treating trace strings
  as a side-channel witness.
- `Asschema` derives the shared NOTA codec and rkyv archive surface wherever
  its data is non-recursive; recursive references use explicit `omit_bounds`
  archive annotations and object-owned codec methods so assembled schema stays
  serializable without adding parser magic.
- `schemas/core.schema` describes macro pattern/template payloads as typed
  schema data (`MacroPatternObject`, `MacroTemplateObject`, delimiter nodes,
  captures, atoms), not opaque strings. `schemas/core.asschema` is checked in
  as the assembled form of that source and freshness-checked by tests.
- `MacroLibrary` is the one typed noun for parsed built-in macros and the
  checked-in macro-library artifact. It contains macro definitions, pattern
  trees, template trees, delimiter values, captures, and atoms, and the
  checked-in `schemas/builtin-macros.macro-library` artifact is the runtime
  load path for built-in declarative macros. `schemas/builtin-macros.schema`
  remains the bootstrap source that tests use to prove artifact freshness. The
  remaining bootstrap step is replacing the hand-written macro-library noun
  with the same noun emitted from core-schema asschema.
