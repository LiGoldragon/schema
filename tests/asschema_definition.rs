use std::path::Path;

use nota_next::Document;
use schema_next::{ImportResolver, SchemaEngine, SchemaIdentity, TypeDeclaration, TypeReference};

#[test]
fn asschema_data_model_is_built_from_real_schema_fixture() {
    let source = include_str!("fixtures/big-schemas/spirit-reactive-large.schema");
    Document::parse(source).expect("schema fixture is legal NOTA");

    let asschema = SchemaEngine::default()
        .lower_source(
            source,
            SchemaIdentity::new("example:spirit-reactive-large", "0.1.0"),
        )
        .expect("schema lowers into typed Asschema data");

    assert_eq!(
        asschema.identity().component().as_str(),
        "example:spirit-reactive-large"
    );
    assert_eq!(asschema.identity().version(), "0.1.0");

    let TypeDeclaration::Struct(record_set) = asschema
        .type_named("RecordSet")
        .expect("RecordSet declaration")
    else {
        panic!("RecordSet must be a struct declaration");
    };
    let records = record_set
        .fields
        .iter()
        .find(|field| field.name.as_str() == "records")
        .expect("records field");
    assert_eq!(
        records.reference,
        TypeReference::Vector(Box::new(TypeReference::new("Entry"))),
        "schema Vec call lowers into typed Vector data, not rendered ASSchema text",
    );

    let by_topic = record_set
        .fields
        .iter()
        .find(|field| field.name.as_str() == "by_topic")
        .expect("by_topic field");
    assert_eq!(
        by_topic.reference,
        TypeReference::Map(
            Box::new(TypeReference::new("Topic")),
            Box::new(TypeReference::new("RecordIdentifier")),
        ),
        "schema KeyValue call lowers into typed Map data, not rendered ASSchema text",
    );
}

#[test]
fn asschema_import_data_is_built_from_real_schema_fixture() {
    let source = include_str!("fixtures/big-schemas/imported-mail-consumer.schema");
    Document::parse(source).expect("schema fixture is legal NOTA");

    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("marker-core")
        .join("schema");
    let resolver = ImportResolver::new().with_dependency("marker-core", schema_dir, "0.1.0");
    let mut context = schema_next::MacroContext::default();
    let asschema = SchemaEngine::default()
        .lower_source_with_resolver(
            source,
            SchemaIdentity::new("example:imported-mail-consumer", "0.1.0"),
            &mut context,
            &resolver,
        )
        .expect("schema with imports lowers");

    assert_eq!(asschema.imports().len(), 2);
    assert_eq!(asschema.resolved_imports().len(), 2);
    assert_eq!(
        asschema.resolved_imports()[0].source().rust_path(),
        "marker_core::schema::mail::DatabaseMarker"
    );
}
