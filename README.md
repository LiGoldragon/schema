# schema-next

`schema-next` is the replacement schema engine for the schema-derived stack.
It consumes `nota-next` blocks, runs position-aware schema macros, and emits
an ordered macro-free assembled schema.

The assembled schema is the first target language. `.asschema` files are NOTA
data with one known root vector typed as `Asschema`; they contain no macro
markers, captures, or authored sugar. Authored `.schema` files may use sugar
while lowering, but the endpoint uses final variants such as `Struct`, `Enum`,
`Newtype`, `Plain`, `Vector`, `Optional`, `Map`, and `Carries`.

Rust code emission is not here. It lives in `schema-rust-next`.

Crates expose schemas through a standard `schema/` directory. The current
entrypoint is `schema/lib.schema`; sibling files such as `schema/signal.schema`
are module schemas. Schema-qualified names use a single colon separator
(`crate:module:Type`) so the schema text mirrors Rust crate/module structure
without using Rust's `::` token.
