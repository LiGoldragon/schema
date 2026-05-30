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

*The assembled schema artifact is a first-class data object. `AsschemaArtifact`
wraps an `Asschema` value and owns the read/write methods for `.asschema`
NOTA files and `.asschema.rkyv` binary files, so downstream Rust emission can
consume a serialized assembled-schema artifact instead of relying on a private
in-memory handoff from schema lowering.*

*A core schema file can be read one layer lower than schema lowering: as raw
NOTA object data. In that mode the root struct name is derived from the
filename, mirroring Rust modules, and the file does not restate that root
name. The root body is a native brace key/value map of datatype names to raw
NOTA objects.*

*.schema files are NOTA documents. Every `.schema` fixture used to prove a
schema-language behavior must first be legal and parseable by `nota-next`
before any schema-specific lowering or raw-schema reading is applied.*

*NOTA delimiters keep structural meaning before schema applies semantic
expectations. Square brackets are bracket/vector structure at the raw layer,
but a schema position typed as `String` or a string newtype may read that
bracket form as text. Parentheses are raw record/struct structure; they become
tagged schema nodes only when the expected type is `SchemaNode` or another
tag-plus-payload struct.*

*Authored schema declarations use name-first `@` binding. A namespace entry
declares a struct with `Type@{ @Reference explicit@Reference ... }` and an
enum with `Type@[Variant @SameNamePayload Variant@Payload ...]`. Plain `[]`
and `()` remain legal NOTA values, but only the `@[` binding form is the
declaration syntax for a schema enum datatype; parentheses remain the
composite/type-reference and macro-call argument form (`(Vec Entry)`,
`(Optional Kind)`, `(Map (Key Value))`). Lowercase/camelCase member names bind
explicit fields; `@PascalCase` derives a field or data-carrying variant from
an existing type. PascalCase names declare or reference schema types. The `@`
is a declaration/binding sigil, not the macro-call sigil rejected by the
schema-node model. The schema root is always the known root struct whose name
comes from the filename, so it does not need a delimiter or `@` wrapper.*

*Assembled schema namespace entries are visibility-tagged data objects. The
canonical NOTA shape is `(Public Name Value)` or `(Private Name Value)`, with
the first payload field carrying the declared name and the second payload field
carrying the struct/enum/newtype value. Top-level authored declarations lower
to public declarations. Inline PascalCase declarations lower to private,
module-local declarations, derive their field name from the type name, and may
be referenced later in the same module.*

*A struct declaration's assembled value is a key/value map from field name to
type reference. The implementation may preserve source order internally for
Rust field order and rkyv layout, but the semantic object is a brace map:
field key -> type-reference value.*

*A newtype declaration's assembled value is a single contained type reference,
not a one-field map with an invented field name. The intended long-form
notation is `(Public Topic { String })`, not `(Public Topic { text String })`,
and Rust emission treats it as a real tuple newtype.*

*The earlier pipe-family declaration syntax remains a compatibility surface
in the parser and macro engine while existing fixtures migrate. It is not the
authored-schema target.*

*For `name@( ... )`, the parenthesized body is resolved at the
assembled-schema reference layer: recognized type-reference heads such as
`Vec`, `Optional`, and `Map` remain composite references, and future user macro
heads can use the same argument shape. Enum declarations use `Name@[...]`
instead, so declaration shape and composite/reference shape are no longer
overloaded. Unnamed composites used as fields may derive names such as
`vec_of_entry` / `VecOfEntry` when there is no conflict.*

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
  captures, atoms), not opaque strings. The built-in macro registry is not yet
  loaded from that asschema data; that is the next step toward fully
  serializable macro tables.
