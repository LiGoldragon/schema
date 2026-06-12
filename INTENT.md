# INTENT — schema-next

`schema-next` is the schema macro engine and typed semantic schema data model
for the schema-derived stack. It does not emit Rust source code.

Load-bearing constraints:

*Macros are position-aware structural matching over NOTA objects.* The
delimiter shape, contained object count, and qualification predicates select
macro variants that lower objects into semantic schema. A macro invocation is
data — a tagged/payload-carrying schema node at a macro position, represented
as data before execution so tables can be serialized, deserialized, tested,
and eventually pre-assembled.

*Schema files read as a known root struct.* The top-level schema shape supplies
the root type name and field positions; inner variable vectors contain macro
objects that expand by position into semantic schema.

*Reserved scalars belong in semantic schema, not Rust.* `String`, `Integer`,
`Boolean`, and `Path` lower to scalar `TypeReference` variants; `Plain(Name)`
is reserved for declared schema types and importable names. Scalar names cannot
be re-declared.

*Cross-crate schema imports resolve through Cargo-exposed dependency
directories.* A schema import source uses single-colon path syntax
`crate:module:Type`; `ImportResolver` loads the dependency's schema module,
confirms the name exists as either a namespace type or root enum, and records
the resolved import.

*NOTA owns raw structure; Schema owns type-name vocabulary.* Square brackets
are raw vector structure, not `Vec` type syntax. Schema type-reference objects
include `(Vec T)`, `(Map (K V))`, and `(Optional T)`, lowering to `Vector`,
`Map`, and `Optional` in semantic schema. Parentheses are composite-reference
and macro-call form.

*Semantic schema namespace entries are visibility-tagged.* The semantic shape
is public/private declaration data over `Name` plus `TypeDeclaration`; top-level
declarations lower to public, direct PascalCase field declarations inside root
inline payload structs lower to public, and nested inline PascalCase helper
declarations lower to private.

*Enum bodies are homogeneous vectors of variant-signature objects.* A unit
variant is a bare PascalCase symbol. A same-named data-carrying variant should
use the compact self-tagged form `(Variant)`; the explicit
`(Variant PayloadType)` form remains for intentionally different variant and
payload names. Bracket members remain one variant-signature object.
Data-carrying variants may also declare stream lifecycle relations:
`(Subscribe Payload opens StreamName)` and
`(Delta DeltaPayload belongs StreamName)`.

*Streams are schema metadata, not namespace data types.* A stream declaration
is authored as `StreamName (Stream { token Token opened Snapshot event Event
close Close })` inside the namespace map. It lowers to semantic
`StreamDeclaration` data and is excluded from namespace type declarations.
The `opens` and `belongs` variant relations point at these stream
declarations so subscription features are visible in schema before daemon
implementation code is inspected.

*Root input/output headers may list exported variant names directly.* When a
bare header entry resolves to a namespace declaration, the variant carries the
same-named payload type. Root inline declarations are inserted into the
exported namespace before the root enum is assembled, and direct PascalCase
field declarations inside root inline struct payloads are also exported for
later inline payloads and namespace declarations. Duplicate declarations are
errors.

*Authored schema is its own typed value before semantic schema.*
`SchemaSource` reads `.schema` documents into source-language data (imports,
input/output enums, namespace declarations) and writes canonical `.schema` text
projections back out. This source codec is separate from raw NOTA parsing and
is the source archive boundary through `SchemaSourceArtifact`.

*Asschema is removed.* The stack does not preserve the old compatibility
projection, `.asschema` text, `.asschema.rkyv` binary, `AsschemaArtifact`, or
old Asschema-facing APIs. The target pipeline is: authored `.schema`
deserializes into Rust datatypes that fully define the schema, that typed value
is rkyv-serializable, and Rust code is lowered from that typed value. `Schema`
is the semantic schema-in-Rust value used by lowerers, emitters, upgrades, and
symbol-path queries; it is not a text artifact and has no artifact/store
wrapper.

*The structured macro-node mechanism belongs at the NOTA layer.* nota-next
owns structural macro-node codec machinery; schema-next owns schema positions
and handlers. Built-in schema macros load through a serialized macro-library
artifact, with hand-authored source kept as a freshness-checked bootstrap
source.

*The macro library reads through typed structural macro nodes; hand parsing
above the raw parser is a violation.* Per Spirit v0n6 (Clarification):
[Everything reading NOTA-shaped structure above the raw structural parser
must go through typed structural macro nodes; surviving hand-parsing sites
such as the schema-next macro library are design violations to fix, not
acceptable code. If structural macro nodes cannot express a needed shape,
that signals the NOTA design was not implemented properly and must be
surfaced to the psyche rather than worked around.]

*The schema is content-addressable; its hash is its identity.* Per Spirit wrjl
(Decision): [The schema-layout description is fully content-addressable — its
hash is its identity. Any edit to the schema changes the address, which is the
version.] Content identity is computed on the semantic schema-in-Rust value:
`Schema::content_hash` is blake3 over the canonical rkyv bytes of `Schema`,
and `Schema::family_closure(name).content_hash()` is blake3 over the canonical
rkyv bytes of a `FamilyClosure` — the named declaration plus everything
transitively reachable from it, sorted canonically by name. Formatting-only
`.schema` source edits (whitespace, comments) do not move either address; the
hash is the version identity the version-control layer consumes, landing
beside the hand-authored `SchemaIdentity` version string, not replacing it.

*One cryptographic basis: blake3.* Per Spirit x0ja (Constraint): [One
consistent cryptographic basis spans the entire version-control and backup
system: blake3 for all content addressing ... No component diverges in hash
function.] All schema content addressing is blake3; the whole-schema and
family-closure hash kinds are domain-separated through distinct blake3
`derive_key` contexts so the two can never collide.
