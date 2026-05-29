# schema-next

`schema-next` is the replacement schema engine for the schema-derived stack.
It consumes `nota-next` blocks, runs position-aware schema macros, and emits
an ordered macro-free assembled schema.

The active file surface is `.schema`. A `.schema` file must parse as legal
NOTA before schema-specific reading. Core schema files can be inspected at the
raw layer as a known root struct named from the file stem, containing one
native brace key/value map of datatype names to raw NOTA datatype objects.
`Asschema` is the in-memory macro-free endpoint produced by lowering. It stores
ordered root declarations plus ordered datatype declarations. Each namespace
declaration is visibility-tagged as `(Public Name Value)` or
`(Private Name Value)` at the assembled-data level; top-level authored
declarations become public and inline PascalCase declarations become private
module-local types. Compatibility accessors still expose the current `Input`
and `Output` roots. Checked-in assembled-schema text fixtures are not part of
the active surface.

The low-level syntax layer preserves the NOTA/schema split: square brackets are
raw vector structure and schema struct field lists, not `Vec` type syntax.
Composite type references are typed NOTA objects such as `(Vec Topic)`,
`(Map (Topic RecordIdentifier))`, and `(Optional Topic)`. Authored datatype
declarations use name-first `@` forms: `Kind@[Decision Correction]` for enums
and `Entry@{ topic@Topic description@String }` for structs. Struct declarations
lower to the asschema key/value map form: field name -> type reference.

Rust code emission is not here. It lives in `schema-rust-next`.

Crates expose schemas through a standard `schema/` directory. The current
entrypoint is `schema/lib.schema`; sibling files such as `schema/signal.schema`
are module schemas. Schema-qualified names use a single colon separator
(`crate:module:Type`) so the schema text mirrors Rust crate/module structure
without using Rust's `::` token.
