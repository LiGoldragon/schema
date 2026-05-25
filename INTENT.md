# schema intent

`schema` exists because schema work now has its own repository. Its purpose
is to give the schema-language and macro work a typed substrate instead of
keeping schema knowledge scattered across design reports and macro internals.

Current intent, in priority order:

1. Represent the NOTA schema language as typed Rust data. Per spirit
   record 551, schema is the macro-language source of truth for component
   data, wire, storage, and upgrade behavior.
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
   descriptors, and version projection. Per record 552, schema diffs
   isolate as typed upgrade operations.
7. Use a multi-pass NOTA-first reader (record 549): one parser, multiple
   semantic passes. Macro application iterates to a fixed point (record
   569); the namespace is dependency-ordered (records 553, 570).
8. Newtypes (record 571) emit single-tuple structs over named inner
   types, exposing inner trait impls transparently while carrying their
   own.
9. Diff operations are Add / Remove / Modify (record 561), with Modify
   subdivided into ContainerEmbed / EnumWrap / Reorder / KeyChange.
10. Data-carrying variants take enum slots 0-6 first; unit variants come
    after (record 562). This makes adding a new unit variant a no-op
    upgrade. Enum-slot planning minimises database rewrites across
    compatible upgrades (record 557).
11. Enum space pre-allocates by inner-type semantics (record 563);
    Boolean fits in one bit, Option in two, micro-enums let SEMA fit
    smaller than rkyv-with-raw-enums by pre-breaking the encoding space
    by type.
12. Common workspace identifiers (record, forward, send, and similar per
    record 564) are stored as enum-encoded composite names; the namespace
    is composable across components and enables multilingual labels
    because the wire form is the discriminator, not the string.
13. Test the schema language end-to-end from actual `.schema` files using
    local relative imports before treating the Rust-only model as sufficient.
14. Stay library-shaped until the runtime schema registry/triad authority is
    explicitly settled.
15. Carry explicit provenance on schema and upgrade work (record 560) so
    the reader can trace each fragment back to its authoring context.

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
- Whether `nota-codec` lands a tree-parser + spans (currently Pass 1 of
  the multi-pass reader is built privately in `schema`); lean is to land
  in `nota-codec` so every NOTA-reading client benefits. Per
  `primary/reports/designer/334-v2-multi-pass-nota-first-schema-reader.md`.
