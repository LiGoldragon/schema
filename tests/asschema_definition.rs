use std::path::Path;

use nota_next::{Block, Delimiter, Document, NotaEncode};
use schema_next::{
    Asschema, AsschemaArtifact, AsschemaStore, ImportResolver, Name, SchemaEngine, SchemaIdentity,
    TypeDeclaration, TypeReference,
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
    let document = Document::parse(&nota).expect("emitted asschema is legal NOTA");
    let root_objects = document.root_objects();
    assert_eq!(
        root_objects.len(),
        6,
        "asschema text is a known-root document with six positional fields"
    );
    assert!(
        matches!(
            root_objects.get(3),
            Some(Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                ..
            })
        ),
        "input is a direct enum-body field, not a labeled Input variant: {nota}"
    );
    assert!(
        matches!(
            root_objects.get(4),
            Some(Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                ..
            })
        ),
        "output is a direct enum-body field, not a labeled Output variant: {nota}"
    );
    assert!(
        !nota.contains("(Input [") && !nota.contains("(Output ["),
        "root field names are supplied by Asschema, not serialized as variants: {nota}"
    );
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

#[test]
fn asschema_store_round_trips_rkyv_and_reexports_nota() {
    let source = include_str!("fixtures/big-schemas/spirit-reactive-large.schema");
    let asschema = SchemaEngine::default()
        .lower_source(
            source,
            SchemaIdentity::new("example:spirit-reactive-large", "0.1.0"),
        )
        .expect("schema lowers into typed Asschema data");
    let paths = AsschemaArtifactTestPaths::new("spirit-reactive-large-store");
    let store = AsschemaStore::open(paths.sema_path()).expect("open asschema sema store");

    assert!(
        store
            .is_empty()
            .expect("fresh asschema sema store is readable")
    );
    store
        .put_asschema(&asschema)
        .expect("persist asschema as rkyv bytes in sema store");
    assert_eq!(store.len().expect("stored asschema count"), 1);

    let recovered = store
        .get_asschema(asschema.identity())
        .expect("read asschema from sema store")
        .expect("stored asschema is present");
    assert_eq!(recovered, asschema);

    store
        .export_nota_file(asschema.identity(), paths.exported_nota_path())
        .expect("export stored asschema back to NOTA");
    let exported = std::fs::read_to_string(paths.exported_nota_path())
        .expect("read exported asschema nota file");
    assert_eq!(
        Asschema::from_nota_source(&exported).expect("exported asschema decodes"),
        asschema
    );
    assert_eq!(
        exported,
        asschema.to_nota(),
        "re-exported NOTA must come from the recovered typed Asschema object"
    );

    drop(store);
    paths.remove();
}

#[test]
fn core_asschema_artifact_is_checked_in_and_fresh() {
    let expected = SchemaEngine::default()
        .lower_source(
            include_str!("../schemas/core.schema"),
            SchemaIdentity::new("schema-next:core", "0.1.0"),
        )
        .expect("core schema lowers");
    let checked_in = AsschemaArtifact::from_nota_source(include_str!("../schemas/core.asschema"))
        .expect("checked-in core asschema decodes");

    assert_eq!(
        checked_in.asschema(),
        &expected,
        "schemas/core.asschema must be refreshed when core.schema or lowering semantics change"
    );
}

#[test]
fn core_asschema_artifact_round_trips_as_nota_and_rkyv() {
    let source = include_str!("../schemas/core.asschema");
    let document = Document::parse(source).expect("core asschema is legal NOTA");
    assert_eq!(
        document.root_objects().len(),
        6,
        "checked-in core.asschema must use known-root fields, not an outer record"
    );
    assert!(
        !source.contains("(Input [") && !source.contains("(Output ["),
        "checked-in core.asschema must not label input/output root positions"
    );

    let artifact =
        AsschemaArtifact::from_nota_source(source).expect("checked-in core asschema decodes");
    let reparsed =
        Asschema::from_nota_source(&artifact.to_nota_source()).expect("core asschema re-decodes");
    let bytes = artifact
        .to_binary_bytes()
        .expect("core asschema archives to rkyv");
    let binary =
        Asschema::from_binary_bytes(&bytes).expect("core asschema decodes from archived bytes");

    assert_eq!(&reparsed, artifact.asschema());
    assert_eq!(&binary, artifact.asschema());
    assert!(
        artifact
            .asschema()
            .type_named("SchemaMacro")
            .is_some_and(|declaration| matches!(declaration, TypeDeclaration::Struct(_))),
        "core.asschema must carry the macro-table noun as assembled schema data"
    );
}

struct AsschemaArtifactTestPaths {
    directory: std::path::PathBuf,
    nota_path: std::path::PathBuf,
    binary_path: std::path::PathBuf,
    sema_path: std::path::PathBuf,
    exported_nota_path: std::path::PathBuf,
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
            sema_path: directory.join("schemas.sema"),
            exported_nota_path: directory.join("exported.asschema"),
            directory,
        }
    }

    fn nota_path(&self) -> &std::path::Path {
        &self.nota_path
    }

    fn binary_path(&self) -> &std::path::Path {
        &self.binary_path
    }

    fn sema_path(&self) -> &std::path::Path {
        &self.sema_path
    }

    fn exported_nota_path(&self) -> &std::path::Path {
        &self.exported_nota_path
    }

    fn remove(&self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
