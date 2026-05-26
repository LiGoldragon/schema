# Architecture

`schema-next` turns NOTA structure into assembled schema.

## Pipeline

1. `nota-next::Document` parses source into blocks.
2. `SchemaEngine` validates the root shape.
3. Position-aware macros lower root surfaces and namespace declarations.
4. `Asschema` is emitted as the ordered macro-free endpoint.

## Constraints

- `MacroPosition` is passed into both `matches` and `lower`.
- `Asschema` stores declarations in `Vec` order; lookup maps are derived.
- The root schema is positional. Current MVP shape:
  - field 1: imports/exports map `{ }`
  - field 2: root surfaces `[ ]`
  - field 3: namespace map `{ }`
- Parentheses define enums and variants.
- Square brackets define structs and their fields.
