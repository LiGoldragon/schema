# schema-next

`schema-next` is the replacement schema engine for the schema-derived stack.
It consumes `nota-next` blocks, runs position-aware schema macros, and emits
an ordered macro-free assembled schema.

Rust code emission is not here. It lives in `schema-rust-next`.

Crates expose schemas through a standard `schema/` directory. The current
entrypoint is `schema/lib.schema`; sibling files such as `schema/signal.schema`
are module schemas. Schema-qualified names use a single colon separator
(`crate:module:Type`) so the schema text mirrors Rust crate/module structure
without using Rust's `::` token.
