# Intent

`schema-next` is the new schema implementation.

Psyche intent:

*The schema-derived stack uses separate repositories for nota-next,
schema-next, and schema-rust-next rather than one combined integration
repository.*

*Macros lower schema positions into assembled schema. The lowering logic must
know where the node appeared; the same delimiter shape can mean different
things at different macro positions.*

This repository owns the schema macro engine and the ordered assembled schema
data model. It does not emit Rust source code.

Current implementation target:

- Macros are registered in `MacroRegistry`, including nested struct-field and
  enum-variant macros.
- The engine lowers root schema sections through the registry, not through
  hard-coded macro fields.
- A macro consumes a typed `MacroObject` at a `MacroPosition`; namespace
  declarations are pairs, while root sections are blocks.
- The context records which macros ran. Tests use that trace as the witness
  that Spirit schema lowering is really macro-dispatched.
