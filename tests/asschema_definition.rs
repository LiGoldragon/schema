use std::path::Path;

use nota_next::{Document, NotaEncode};
use schema_next::{
    AsschemaArtifact, ImportResolver, Name, SchemaEngine, SchemaIdentity, TypeDeclaration,
    TypeReference,
};

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
        "schema native vector reference lowers into typed Vector data",
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
        "schema native key-value reference lowers into typed Map data",
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

#[test]
fn asschema_is_a_live_nota_and_rkyv_data_artifact() {
    let source = include_str!("fixtures/big-schemas/spirit-reactive-large.schema");
    let asschema = SchemaEngine::default()
        .lower_source(
            source,
            SchemaIdentity::new("example:spirit-reactive-large", "0.1.0"),
        )
        .expect("schema lowers into typed Asschema data");

    let nota = asschema.to_nota();
    Document::parse(&nota).expect("emitted asschema is legal NOTA");
    let from_nota = schema_next::Asschema::from_nota_source(&nota)
        .expect("asschema decodes from its NOTA form");
    assert_eq!(from_nota, asschema);

    let bytes = asschema
        .to_binary_bytes()
        .expect("asschema encodes as rkyv bytes");
    let from_binary =
        schema_next::Asschema::from_binary_bytes(&bytes).expect("asschema decodes from rkyv bytes");
    assert_eq!(from_binary, asschema);

    assert!(
        nota.contains("(Public Entry (Struct (Entry"),
        "the assembled artifact carries visibility, names, and type declarations as data: {nota}"
    );
    assert!(
        nota.contains("(Vector (Plain Entry))"),
        "schema Vec sugar must be gone; assembled schema carries Vector data: {nota}"
    );
}

#[test]
fn asschema_names_emit_symbol_safe_strings_as_bare_symbols() {
    assert_eq!(Name::new("Entry").to_nota(), "Entry");
    assert_eq!(
        Name::new("schema:spirit:Entry").to_nota(),
        "schema:spirit:Entry"
    );
    assert_eq!(
        Name::new("not a symbol").to_nota(),
        "[not a symbol]",
        "non-symbol names still fall back to NOTA string form"
    );
}

#[test]
fn asschema_artifact_reads_and_writes_real_nota_and_binary_files() {
    let source = include_str!("fixtures/big-schemas/spirit-reactive-large.schema");
    let asschema = SchemaEngine::default()
        .lower_source(
            source,
            SchemaIdentity::new("example:spirit-reactive-large", "0.1.0"),
        )
        .expect("schema lowers into typed Asschema data");
    let artifact = AsschemaArtifact::new(asschema.clone());
    let paths = AsschemaArtifactTestPaths::new("spirit-reactive-large");

    artifact
        .write_nota_file(paths.nota_path())
        .expect("write asschema nota artifact");
    artifact
        .write_binary_file(paths.binary_path())
        .expect("write asschema binary artifact");

    let from_nota =
        AsschemaArtifact::read_nota_file(paths.nota_path()).expect("read asschema nota artifact");
    let from_binary = AsschemaArtifact::read_binary_file(paths.binary_path())
        .expect("read asschema binary artifact");

    assert_eq!(from_nota.asschema(), &asschema);
    assert_eq!(from_binary.asschema(), &asschema);
    assert!(
        std::fs::read_to_string(paths.nota_path())
            .expect("read written asschema text")
            .contains("(Plain Entry)"),
        "real .asschema artifact must carry bare schema symbols"
    );

    paths.remove();
}

struct AsschemaArtifactTestPaths {
    directory: std::path::PathBuf,
    nota_path: std::path::PathBuf,
    binary_path: std::path::PathBuf,
}

impl AsschemaArtifactTestPaths {
    fn new(name: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "schema-next-asschema-artifact-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("create asschema artifact test directory");
        Self {
            nota_path: directory.join("lib.asschema"),
            binary_path: directory.join("lib.asschema.rkyv"),
            directory,
        }
    }

    fn nota_path(&self) -> &std::path::Path {
        &self.nota_path
    }

    fn binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    fn remove(&self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
