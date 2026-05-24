# schema — architecture

## Role

`schema` hosts the typed model for Persona's NOTA schema language. The
crate represents a resolved schema document, validates declaration and
variant references, and derives layout metadata that tells macro code which
fields stay in the fixed root and which fields move into ordered boxes.

## Boundaries

**Owns:**
- Schema document types: `Document`, `Declaration`, `Variant`,
  `Payload`, `TypeExpression`, `Primitive`, `Container`, and `Engine`.
- Validation of declaration uniqueness, variant uniqueness, and named type
  references.
- Conservative root-versus-box layout planning for data-carrying variants.
- Cross-schema reference placeholders (`Reference`) after path or symbolic
  refs are resolved by a reader.

**Does not own:**
- NOTA text parsing. A reader will lower NOTA source into this model.
- `signal_channel!` code emission. That stays in the macro crate.
- Short-header frame bytes. Those stay in `signal-frame`.
- Version projection execution. That stays in `version-projection` and the
  macro-emitted impls.
- Daemon runtime, storage opening, and handover orchestration.

## Code map

```text
src/
├── lib.rs        # public exports
├── document.rs   # Document + validation
├── declaration.rs # declarations, variants, payloads, refs
├── expression.rs # primitive/container/named type expressions
├── layout.rs     # fixed-root versus ordered-box planning
├── name.rs       # schema identifier validation
└── error.rs      # typed error enum

tests/
└── document.rs   # validation + layout behavior
```

## Invariants

Schema records are positional. Data-carrying variant fields are ordered
`TypeExpression` values; the model does not carry field labels.

Names are PascalCase identifiers because declarations and variants become
closed Rust enums or enum-like schema nodes.

Validation is intentionally loud. Unknown types, duplicate declarations,
and duplicate variants are errors before macro emission starts.

Layout planning is conservative. Built-in fixed-width primitives and unit
enums can stay in the root. Strings, bytes, containers, unresolved
cross-schema references, and recursively variable declarations move to
boxes until a later resolved schema proves otherwise.

## Status

Initial library scaffold. It is sufficient for macro work to depend on a
typed schema document and a deterministic root/box layout plan, but it is
not yet a parser, code generator, or runtime schema registry.
