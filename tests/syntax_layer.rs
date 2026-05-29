use std::fs;

use schema_next::{
    Name, SchemaError, SyntaxDeclaration, SyntaxReference, SyntaxSchema, SyntaxStructDeclaration,
};

#[test]
fn syntax_schema_reads_real_schema_file_through_raw_nota_first() {
    let schema = syntax_schema("tests/fixtures/syntax-layer/schema.schema");

    assert_eq!("Schema", schema.root_name().as_str());
    assert_eq!(11, schema.datatypes().len());

    let topic = struct_named(&schema, "Topic");
    assert_eq!("Topic", topic.name().as_str());
    assert_eq!(1, topic.fields().len());
    assert_eq!("text", topic.fields()[0].name().as_str());
    assert_eq!(&name_reference("Text"), topic.fields()[0].reference());
}

#[test]
fn typed_composite_objects_are_read_from_parenthesis_records() {
    let schema = syntax_schema("tests/fixtures/syntax-layer/schema.schema");
    let entry = struct_named(&schema, "Entry");

    assert_eq!("topics", entry.fields()[0].name().as_str());
    assert_eq!(&name_reference("Topics"), entry.fields()[0].reference());
    assert_eq!("related", entry.fields()[3].name().as_str());
    assert_eq!(&name_reference("TopicIndex"), entry.fields()[3].reference());
    assert_eq!("maybeTopic", entry.fields()[4].name().as_str());
    assert_eq!(
        &SyntaxReference::Optional(Box::new(name_reference("Topic"))),
        entry.fields()[4].reference()
    );

    let topics = struct_named(&schema, "Topics");
    assert_eq!(
        &SyntaxReference::Vector(Box::new(name_reference("Topic"))),
        topics.fields()[0].reference()
    );

    let topic_index = struct_named(&schema, "TopicIndex");
    assert_eq!(
        &SyntaxReference::Map(
            Box::new(name_reference("Topic")),
            Box::new(name_reference("Identifier"))
        ),
        topic_index.fields()[0].reference()
    );
}

#[test]
fn at_parenthesis_declaration_creates_unit_and_data_carrying_variants() {
    let schema = syntax_schema("tests/fixtures/syntax-layer/schema.schema");
    let input = enum_declaration(&schema, "SpiritInput");

    assert_eq!("SpiritInput", input.name().as_str());
    assert_eq!(3, input.variants().len());
    assert_eq!("Record", input.variants()[0].name().as_str());
    assert_eq!(
        Some(&name_reference("Entry")),
        input.variants()[0].payload()
    );
    assert_eq!("Observe", input.variants()[1].name().as_str());
    assert_eq!(
        Some(&name_reference("RecordQuery")),
        input.variants()[1].payload()
    );
    assert_eq!("Ping", input.variants()[2].name().as_str());
    assert_eq!(None, input.variants()[2].payload());
}

#[test]
fn at_brace_at_datatype_declaration_position_creates_struct_field_lists() {
    let schema = syntax_schema("tests/fixtures/syntax-layer/schema.schema");
    let text = struct_named(&schema, "Text");

    assert!(text.is_newtype());
    assert_eq!("string", text.fields()[0].name().as_str());
    assert_eq!(&name_reference("String"), text.fields()[0].reference());
}

#[test]
fn plain_square_bracket_datatype_declarations_are_rejected() {
    let source = "{ Text [String] }";
    let error =
        SyntaxSchema::from_path_and_source("tests/fixtures/syntax-layer/plain.schema", source)
            .unwrap_err();

    assert_eq!(
        SchemaError::ExpectedSyntaxDeclaration {
            found: "square-bracket vector".to_owned(),
        },
        error
    );
}

#[test]
fn at_declaration_name_must_match_namespace_key() {
    let source = fs::read_to_string("tests/fixtures/syntax-layer/name-mismatch.schema").unwrap();
    let error = SyntaxSchema::from_path_and_source(
        "tests/fixtures/syntax-layer/name-mismatch.schema",
        &source,
    )
    .unwrap_err();

    assert_eq!(
        SchemaError::RawDeclarationNameMismatch {
            key: "Entry".to_owned(),
            declared: "Other".to_owned()
        },
        error
    );
}

fn syntax_schema(path: &str) -> SyntaxSchema {
    let source = fs::read_to_string(path).unwrap();
    SyntaxSchema::from_path_and_source(path, &source).unwrap()
}

fn name_reference(name: &str) -> SyntaxReference {
    SyntaxReference::Plain(Name::new(name))
}

fn struct_named<'schema>(
    schema: &'schema SyntaxSchema,
    name: &str,
) -> &'schema SyntaxStructDeclaration {
    let declaration = schema.datatype_named(name).unwrap().declaration();
    let SyntaxDeclaration::Struct(declaration) = declaration else {
        panic!("{name} was not a struct declaration");
    };
    declaration
}

fn enum_declaration<'schema>(
    schema: &'schema SyntaxSchema,
    name: &str,
) -> &'schema schema_next::SyntaxEnumDeclaration {
    let declaration = schema.datatype_named(name).unwrap().declaration();
    let SyntaxDeclaration::Enum(declaration) = declaration else {
        panic!("{name} was not an enum declaration");
    };
    declaration
}
