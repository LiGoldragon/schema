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

*A type at a reference position may be a collection or option, not only a
bare name. Square brackets remain raw NOTA vector structure and schema field
lists; they are not the schema syntax for declaring a `Vec` type. The typed
NOTA data objects are `(Vec T)`, `(Map (K V))`, and `(Optional T)`, lowering to
`Vector`, `Map`, and `Optional` in assembled schema. The inner positions
recurse, so `(Vec (Optional Topic))` and `(Map (NodeName (Vec Service)))`
nest. Collection references appear at every reference position: struct
fields, enum-variant payloads, root input/output variant payloads, and import
sources. User-declared macros may still appear at type-reference positions,
but the built-in composite type objects belong to the NOTA datatype layer
that schema reads.*

*Reserved scalar pass-throughs belong in assembled schema, not only in Rust
emission. `Text`, `Integer`, and `Boolean` lower to scalar `TypeReference`
variants; `Plain(Name)` is reserved for declared schema types and importable
namespace names. Scalar names cannot be re-declared in a schema namespace.*

*Cross-crate schema imports are resolved through Cargo-exposed dependency
schema directories, not duplicated locally. A schema import source uses the
single-colon path `crate:module:Type`; `ImportResolver` loads the dependency's
schema module, confirms the type is declared there, and records a resolved
import so Rust emission can reference the dependency type by alias.*

*Assembled schema is defined before authored-schema sugar. `Asschema` is the
typed in-memory macro-free endpoint produced by lowering real `.schema` files.
Current tests assert that typed data directly rather than preserving assembled
schema text fixtures.*

*A core schema file can be read one layer lower than schema lowering: as raw
NOTA datatype data. In that mode the root struct name is derived from the
filename, mirroring Rust modules, and the file does not restate that root
name. The root body is a native brace key/value map of datatype names to raw
NOTA datatype objects.*

*.schema files are NOTA documents. Every `.schema` fixture used to prove a
schema-language behavior must first be legal and parseable by `nota-next`
before any schema-specific lowering or raw-schema reading is applied.*

*NOTA delimiters keep structural meaning before schema applies semantic
expectations. Square brackets are bracket/vector structure at the raw layer,
but a schema position typed as `String` or a string newtype may read that
bracket form as text. Parentheses are raw record/struct structure; they become
tagged schema nodes only when the expected type is `SchemaNode` or another
tag-plus-payload struct.*

This repository owns the schema macro engine and the ordered assembled schema
data model. It does not emit Rust source code.

Current implementation target:

- Macros are registered in `MacroRegistry`, including nested struct-field and
  enum-variant macros.
- The engine lowers root schema sections through the registry, not through
  hard-coded macro fields.
- A macro consumes a typed `MacroObject` at a `MacroPosition`; namespace
  declarations are pairs, while root sections are blocks.
- The context records which macros ran. Tests use that trace as the witness
  that Spirit schema lowering is really macro-dispatched.
