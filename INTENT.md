# schema intent

`schema` exists because schema work now has its own repository. Its purpose
is to give the schema-language and macro work a typed substrate instead of
keeping schema knowledge scattered across design reports and macro internals.

Current intent, in priority order:

1. Represent the NOTA schema language as typed Rust data.
2. Keep the top-level `.schema` file as the six fixed positional fields:
   imports, ordinary header, owner header, sema header, namespace, features.
3. Use NOTA's curly-brace map form as the name-value substrate for schema
   namespaces.
4. Keep the schema language positional: type first, then fields in
   declaration order.
5. Support the current Spirit MVP shape: uniform `(Root [SubVariant ...])`
   route headers, namespace-defined route bodies, cross-schema imports,
   root-plus-ordered-box layout, and feature-carried upgrade annotations.
6. Lower authored schemas into `AssembledSchema`, the explicit machine
   object consumed by short-header generation, code emission, storage
   descriptors, and version projection.
7. Stay library-shaped until the runtime schema registry/triad authority is
   explicitly settled.

Open intent needing later settlement:

- Whether this repository also owns the eventual `nota-box` crate surface or
  whether `nota-codec` keeps the wire container and `schema` only owns
  metadata.
- Whether a future schema daemon triad is required before schema metadata is
  queried at runtime.
- Whether schema imports begin as path references only, Cargo symbolic
  references, or both.
- Whether upgrade grammar grows beyond `Migrate`, `RenamedFrom`, `Drop`,
  `Custom`, and `Untranslatable` before the Spirit pilot needs it.
