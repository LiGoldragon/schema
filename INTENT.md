# INTENT — schema-next

`schema-next` is the schema macro engine and typed assembled schema data model for the schema-derived stack.

Load-bearing constraints:

*Macros are position-aware structural matching over NOTA objects.* The delimiter shape, contained object count, and qualification predicates select macro variants that lower objects into assembled schema. A macro invocation is data—a tagged/payload-carrying schema node at a macro position, represented as data before execution so tables can be serialized, deserialized, tested, and eventually pre-assembled.

*Schema files read as a known root struct.* The top-level schema shape supplies the root type name and field positions; inner variable vectors contain macro objects that expand by position into assembled schema.

*Reserved scalars belong in assembled schema, not Rust.* `String`, `Integer`, `Boolean`, and `Path` lower to scalar `TypeReference` variants; `Plain(Name)` is reserved for declared schema types and importable names. Scalar names cannot be re-declared.

*Cross-crate schema imports resolve through Cargo-exposed dependency directories.* A schema import source uses single-colon path syntax `crate:module:Type`; `ImportResolver` loads the dependency's schema module, confirms the name exists as either a namespace type or root enum, and records the resolved import.

*NOTA owns raw structure; Schema owns type-name vocabulary.* Square brackets are raw vector structure, not `Vec` type syntax. Schema type-reference objects include `(Vec T)`, `(Map (K V))`, and `(Optional T)`, lowering to `Vector`, `Map`, and `Optional` in assembled schema. Parentheses are composite-reference and macro-call form.

*Assembled schema namespace entries are visibility-tagged.* The canonical NOTA shape is `(Public Name Value)` or `(Private Name Value)`; top-level declarations lower to public, inline PascalCase declarations lower to private.

*Enum bodies are homogeneous vectors of variant-signature objects.* A unit variant is a bare PascalCase symbol; a data-carrying variant is `(Variant PayloadType)`. Bracket members remain one variant-signature object.

*Root input/output headers may list exported variant names directly.* When a bare header entry resolves to a namespace declaration, the variant carries the same-named payload type. Inline declarations are inserted into the exported namespace before the root enum is assembled.

*Authored schema is its own typed value before assembled schema.* `SchemaSource` reads `.schema` documents into source-language data (imports, input/output enums, namespace declarations) and writes canonical `.schema` text projections back out. This source codec is separate from raw NOTA parsing and separate from `Asschema` serialization.

*The structured macro-node mechanism belongs at the NOTA layer.* nota-next owns structural macro-node codec machinery; schema-next owns schema positions and handlers. Built-in schema macros load through a serialized macro-library artifact, with hand-authored source kept as a freshness-checked bootstrap source.

*Asschema is now a compatibility projection.* The target pipeline is: authored `.schema` deserializes into Rust datatypes that fully define the schema, that typed value is rkyv-serializable, and Rust code is lowered from that typed value. The current `Asschema` type, `.asschema` text, and `.asschema.rkyv` binary surfaces remain only until source-node types and Rust emission finish migration.

This repository owns the schema macro engine and the ordered assembled schema data model. It does not emit Rust source code.
