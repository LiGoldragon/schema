# schema intent

`schema` exists because schema work now has its own repository. The first
purpose is to give the schema-language and macro work a typed substrate
instead of keeping schema knowledge scattered across design reports and
macro internals.

Current intent, in priority order:

1. Represent the NOTA schema language as typed Rust data.
2. Keep the schema language positional: type first, then fields in
   declaration order.
3. Support the current Spirit MVP shape: root-verb enums, data-carrying
   enum payloads, `(engine X)` annotations, cross-schema references, and
   root-plus-ordered-box layout.
4. Stay library-shaped until the runtime schema registry/triad authority is
   explicitly settled.

Open intent needing later settlement:

- Whether this repository also owns the eventual `nota-box` crate surface or
  whether `nota-codec` keeps the wire container and `schema` only owns
  metadata.
- Whether a future schema daemon triad is required before schema metadata is
  queried at runtime.
