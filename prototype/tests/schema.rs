//! Schema-level tests — three-part shape parsing, namespace
//! assembly, codec emission.

use schema_derived_nota_prototype::{
    AssembledSchema, EmittedCodec, Library, MacroEngine, MacroShape, ThreePartSchema, TypeBody,
};

const NOTA_SCHEMA: &str = include_str!("../schemas/nota.schema");
const COORDINATE_SCHEMA: &str = include_str!("../schemas/coordinate.schema");

#[test]
fn nota_schema_reads_into_three_part_view() {
    let three_part = ThreePartSchema::read(NOTA_SCHEMA).expect("nota.schema reads");
    assert!(
        three_part.specifying.is_empty(),
        "no imports at the foundation"
    );
    assert!(three_part.input_header.is_empty(), "no operations");
    assert!(three_part.input_extras.is_empty(), "no extras");
    assert!(
        !three_part.namespace.is_empty(),
        "namespace must have content"
    );
    assert!(three_part.output.is_empty(), "no replies");
    assert!(!three_part.has_input());
    assert!(!three_part.has_output());
}

#[test]
fn nota_schema_namespace_declares_expected_types() {
    let schema = AssembledSchema::read(NOTA_SCHEMA).expect("nota.schema assembles");
    // Spot-check the foundational declarations.
    assert!(schema.lookup("Delimiter").is_some());
    assert!(schema.lookup("IdentifierClass").is_some());
    assert!(schema.lookup("TokenKind").is_some());
    assert!(schema.lookup("Token").is_some());
    assert!(schema.lookup("Node").is_some());
    assert!(schema.lookup("StringForm").is_some());
}

#[test]
fn nota_schema_delimiter_is_an_enum_with_six_variants() {
    let schema = AssembledSchema::read(NOTA_SCHEMA).expect("nota.schema assembles");
    let entry = schema.lookup("Delimiter").expect("Delimiter declared");
    match &entry.body {
        TypeBody::Enum { variants } => {
            let names: Vec<_> = variants.iter().map(|v| v.name.as_str()).collect();
            assert_eq!(
                names,
                vec![
                    "RecordOpen",
                    "RecordClose",
                    "VectorOpen",
                    "VectorClose",
                    "MapOpen",
                    "MapClose"
                ]
            );
        }
        other => panic!("expected Enum, got {other:?}"),
    }
}

#[test]
fn coordinate_schema_has_three_input_operations() {
    let schema = AssembledSchema::read(COORDINATE_SCHEMA).expect("coordinate.schema assembles");
    assert_eq!(schema.input_operations.len(), 3);
    let names: Vec<_> = schema
        .input_operations
        .iter()
        .map(|op| op.name.as_str())
        .collect();
    assert_eq!(names, vec!["Move", "Rotate", "Read"]);
}

#[test]
fn coordinate_schema_has_one_output_operation() {
    let schema = AssembledSchema::read(COORDINATE_SCHEMA).expect("coordinate.schema assembles");
    assert_eq!(schema.output_operations.len(), 1);
    assert_eq!(schema.output_operations[0].name, "Replied");
}

#[test]
fn emit_codec_from_nota_schema_classifies_kinds() {
    let schema = AssembledSchema::read(NOTA_SCHEMA).expect("nota.schema assembles");
    let codec = EmittedCodec::emit(&schema);
    assert!(codec.declares("Delimiter"));
    assert!(codec.declares("Node"));
    assert!(codec.enum_names.contains(&"Delimiter".to_string()));
    assert!(codec.is_known_variant("Delimiter", "RecordOpen"));
    assert!(codec.is_known_variant("Delimiter", "MapClose"));
    assert!(!codec.is_known_variant("Delimiter", "DoesNotExist"));
}

#[test]
fn emit_codec_bare_eligibility_matches_nota_rules() {
    let schema = AssembledSchema::read(NOTA_SCHEMA).expect("nota.schema assembles");
    let codec = EmittedCodec::emit(&schema);
    // Eligible
    assert!(codec.is_bare_eligible("nota-codec"));
    assert!(codec.is_bare_eligible("camelCase"));
    assert!(codec.is_bare_eligible("foo"));
    // Ineligible
    assert!(!codec.is_bare_eligible(""));
    assert!(!codec.is_bare_eligible("Pascal"));
    assert!(!codec.is_bare_eligible("None"));
    assert!(!codec.is_bare_eligible("with space"));
    assert!(!codec.is_bare_eligible("3leading"));
}

#[test]
fn emit_codec_block_form_detection() {
    let schema = AssembledSchema::read(NOTA_SCHEMA).expect("nota.schema assembles");
    let codec = EmittedCodec::emit(&schema);
    assert!(codec.needs_block_form("multi\nline"));
    assert!(!codec.needs_block_form("single line"));
}

#[test]
fn macro_classify_single_identifier_map() {
    let mut kernel = schema_derived_nota_prototype::Kernel::new("{ universalUnknown }");
    let node = kernel.parse_single().expect("parses");
    let engine = MacroEngine::new();
    match engine.classify(&node) {
        MacroShape::SingleIdentifierMap { name } => assert_eq!(name, "universalUnknown"),
        other => panic!("expected SingleIdentifierMap, got {other:?}"),
    }
}

#[test]
fn macro_classify_key_value_map() {
    let mut kernel = schema_derived_nota_prototype::Kernel::new("{ host localhost port 8080 }");
    let node = kernel.parse_single().expect("parses");
    let engine = MacroEngine::new();
    match engine.classify(&node) {
        MacroShape::KeyValueMap { entries } => {
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].0, "host");
            assert_eq!(entries[1].0, "port");
        }
        other => panic!("expected KeyValueMap, got {other:?}"),
    }
}

#[test]
fn macro_classify_named_record() {
    let mut kernel = schema_derived_nota_prototype::Kernel::new("(Some 42)");
    let node = kernel.parse_single().expect("parses");
    let engine = MacroEngine::new();
    match engine.classify(&node) {
        MacroShape::NamedRecord { name, fields } => {
            assert_eq!(name, "Some");
            assert_eq!(fields.len(), 1);
        }
        other => panic!("expected NamedRecord, got {other:?}"),
    }
}

#[test]
fn library_loads_core_implicitly_then_per_component() {
    let mut library = Library::with_core(NOTA_SCHEMA).expect("core loads");
    assert!(library.core().lookup("Delimiter").is_some());
    library
        .load("coordinate", COORDINATE_SCHEMA)
        .expect("coordinate loads");
    assert!(library.get("coordinate").is_some());
    assert_eq!(library.loaded_names(), vec!["coordinate"]);
}

#[test]
fn library_resolve_falls_through_to_core() {
    let mut library = Library::with_core(NOTA_SCHEMA).expect("core loads");
    library
        .load("coordinate", COORDINATE_SCHEMA)
        .expect("coordinate loads");
    // `Coordinate` is declared in the coordinate schema.
    assert!(library.resolve("coordinate", "Coordinate").is_some());
    // `TokenKind` is declared only in core (nota.schema); fallthrough must find it.
    assert!(library.resolve("coordinate", "TokenKind").is_some());
    // `Nonexistent` is in neither.
    assert!(library.resolve("coordinate", "Nonexistent").is_none());
}

#[test]
fn library_rejects_double_load() {
    let mut library = Library::with_core(NOTA_SCHEMA).expect("core loads");
    library
        .load("coordinate", COORDINATE_SCHEMA)
        .expect("first load");
    let err = library.load("coordinate", COORDINATE_SCHEMA);
    assert!(err.is_err(), "second load must fail");
}
