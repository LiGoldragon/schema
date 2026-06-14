# schema-next

`schema-next` is the replacement schema engine for the schema-derived stack.
It consumes `nota-next` blocks, runs position-aware schema macros, and lowers
to an ordered semantic schema-in-Rust value.

The active file surface is `.schema`. A `.schema` file must parse as legal
NOTA before schema-specific reading. Core schema files can be inspected at the
raw layer as a known root struct named from the file stem, containing one
native brace key/value map of datatype names to raw NOTA datatype objects.
`Schema` is the in-memory semantic schema-in-Rust endpoint produced by
lowering. It stores the known `Input` and `Output` enum declarations as direct
fields plus ordered datatype declarations. They are not serialized as a vector
of root wrappers: vectors remain homogeneous, and the root product shape
carries the heterogeneous positions. Each namespace declaration is
visibility-tagged as `(Public Name Value)` or `(Private Name Value)`;
top-level authored declarations become public and inline PascalCase
declarations become private module-local types.

Asschema is retired. There is no `.asschema` text artifact, no `.asschema.rkyv`
binary, no `AsschemaArtifact`, and no semantic-schema store. The pipeline is
`.schema` -> schema-in-Rust (`SchemaSource` source nouns own resolution) ->
`Schema` -> Rust: authored `.schema` deserializes into the typed source value,
the semantic `Schema` value is rkyv-serializable, and Rust is lowered from that
typed value. The text/binary source artifact lives at `SchemaSourceArtifact`.
The core substrate keeps checked-in data artifacts:
`schemas/builtin-macros.macro-library` is the serialized macro table consumed
by the default registry, freshness-checked against `schemas/builtin-macros.schema`.

The low-level syntax layer preserves the NOTA/schema split: square brackets are
raw vector structure and enum bodies, not `Vec` type syntax. Composite type
references are typed Schema objects such as `(Vector Topic)`,
`(Map Topic RecordIdentifier)`, and `(Optional Topic)`. Authored namespace
braces are strict key/value maps: `Topic String`, `Topics (Vector Topic)`,
`Entry { topic Topic Topics * }`, and `Kind [Decision Correction]`. Struct
declarations lower to the semantic key/value map form: field name -> type
reference. Newtypes lower as one contained type reference, not as a one-entry
field map. The default parser accepts the strict surface only; older
declaration forms that repeat their own name are not on the production lowering
path.

Declarative schema macros have a typed data surface. The built-in macro source
is read as `MacroLibrary { source_entries: Vec<MacroLibrarySourceEntry> }`,
where `(SchemaMacro ...)` is the
`MacroLibrarySourceEntry::SchemaMacro(SchemaMacro)` variant. The checked-in
macro-library artifact is the same `MacroLibrary` value: it round-trips
through NOTA, archives itself directly through rkyv, and rebuilds executable
macro handlers. The remaining self-hosting step is to generate that
macro-table type from `schemas/core.schema` instead of using the hand-written
Rust noun.

Rust code emission is not here. It lives in `schema-rust-next`.

Crates expose schemas through a standard `schema/` directory. The current
entrypoint is `schema/lib.schema`; sibling files such as `schema/signal.schema`
are module schemas. Schema-qualified names use a single colon separator
(`crate:module:Type`) so the schema text mirrors Rust crate/module structure
without using Rust's `::` token.
