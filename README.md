# schema-next

`schema-next` is the replacement schema engine for the schema-derived stack.
It consumes `nota-next` blocks, runs position-aware schema macros, and emits
an ordered macro-free assembled schema.

The active file surface is `.schema`. A `.schema` file must parse as legal
NOTA before schema-specific reading. Core schema files can be inspected at the
raw layer as a known root struct named from the file stem, containing one
native brace key/value map of datatype names to raw NOTA datatype objects.
`Asschema` is the in-memory macro-free endpoint produced by lowering; the old
checked-in `.asschema` vector-record fixture syntax is obsolete.

Rust code emission is not here. It lives in `schema-rust-next`.

Crates expose schemas through a standard `schema/` directory. The current
entrypoint is `schema/lib.schema`; sibling files such as `schema/signal.schema`
are module schemas. Schema-qualified names use a single colon separator
(`crate:module:Type`) so the schema text mirrors Rust crate/module structure
without using Rust's `::` token.
