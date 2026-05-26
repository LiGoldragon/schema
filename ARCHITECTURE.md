# Architecture

`schema-next` turns NOTA structure into assembled schema.

## Pipeline

1. `nota-next::Document` parses source into blocks.
2. `SchemaEngine` validates the root object count.
3. `MacroRegistry` dispatches position-aware macros for imports, root
   surfaces, namespace declarations, struct fields, and enum variants.
4. `Asschema` is emitted as the ordered macro-free endpoint.

## Constraints

- `MacroPosition` is passed into both `matches` and `lower`.
- `SchemaMacro` receives a `MacroObject` input (`Block` or namespace `Pair`)
  so each macro declares the object shape it consumes at its position.
- `MacroRegistry` is the engine dispatch path for schema sections and nested
  type-body lowering. Concrete macro fields on `SchemaEngine` or root macros
  are not the design.
- `MacroContext` records positions and applied macro names so tests can prove
  lowering used the macro path.
- `Asschema` stores declarations in `Vec` order; lookup maps are derived.
- The root schema is positional. Current MVP shape:
  - field 1: imports/exports map `{ }`
  - field 2: root surfaces `[ ]`
  - field 3: namespace map `{ }`
- Parentheses define enums and variants.
- Square brackets define structs and their fields.
