use std::path::Path;

use schema_next::{
    Asschema, Declaration, EnumDeclaration, ImportResolver, MacroContext, SchemaEngine,
    SchemaIdentity, TypeDeclaration,
};

#[test]
fn big_spirit_example_lowers_to_checked_asschema_output() {
    assert_big_fixture(
        "spirit-reactive-large",
        include_str!("fixtures/big-schemas/spirit-reactive-large.schema"),
        None,
    );
}

#[test]
fn big_triad_example_lowers_to_checked_asschema_output() {
    assert_big_fixture(
        "triad-reactive-large",
        include_str!("fixtures/big-schemas/triad-reactive-large.schema"),
        None,
    );
}

#[test]
fn big_imported_consumer_example_resolves_cross_crate_imports() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("marker-core")
        .join("schema");
    let resolver = ImportResolver::new().with_dependency("marker-core", schema_dir, "0.1.0");
    assert_big_fixture(
        "imported-mail-consumer",
        include_str!("fixtures/big-schemas/imported-mail-consumer.schema"),
        Some(resolver),
    );
}

fn assert_big_fixture(name: &str, source: &str, resolver: Option<ImportResolver>) {
    let engine = SchemaEngine::default();
    let mut context = MacroContext::default();
    let identity = SchemaIdentity::new(format!("example:{name}"), "0.1.0");
    let asschema = match resolver {
        Some(resolver) => engine
            .lower_source_with_resolver(source, identity, &mut context, &resolver)
            .expect("big schema lowers with imports"),
        None => engine
            .lower_source_with_context(source, identity, &mut context)
            .expect("big schema lowers"),
    };
    assert_asschema_data_shape(name, &asschema);
}

fn assert_asschema_data_shape(name: &str, asschema: &Asschema) {
    assert_eq!(
        asschema.identity().component().as_str(),
        format!("example:{name}")
    );
    assert_eq!(asschema.identity().version(), "0.1.0");
    assert!(
        !asschema.input().variants.is_empty(),
        "{name} should lower typed input variants"
    );
    assert!(
        !asschema.output().variants.is_empty(),
        "{name} should lower typed output variants"
    );
    assert!(
        asschema.root_named("Input").is_some(),
        "{name} should expose Input as a direct root enum"
    );
    assert!(
        asschema.root_named("Output").is_some(),
        "{name} should expose Output as a direct root enum"
    );
    assert!(
        !asschema.namespace().is_empty(),
        "{name} should lower typed namespace declarations"
    );
    match name {
        "spirit-reactive-large" => {
            assert_has_type(asschema.namespace(), "Entry");
            assert_has_type(asschema.namespace(), "RecordSet");
            assert_has_variant(asschema.input(), "Record");
            assert_has_variant(asschema.output(), "Recorded");
        }
        "triad-reactive-large" => {
            assert_has_type(asschema.namespace(), "SignalRequest");
            assert_has_type(asschema.namespace(), "NexusRequest");
            assert_has_type(asschema.namespace(), "SemaRequest");
            assert_has_variant(asschema.input(), "SignalIn");
            assert_has_variant(asschema.output(), "SignalOut");
        }
        "imported-mail-consumer" => {
            assert!(!asschema.imports().is_empty());
            assert!(!asschema.resolved_imports().is_empty());
            assert_has_variant(asschema.output(), "Marked");
        }
        _ => panic!("unhandled big fixture {name}"),
    }
}

fn assert_has_type(declarations: &[Declaration], name: &str) {
    let found = declarations
        .iter()
        .any(|declaration| match declaration.value() {
            TypeDeclaration::Alias(declaration) => declaration.name.as_str() == name,
            TypeDeclaration::Struct(declaration) => declaration.name.as_str() == name,
            TypeDeclaration::Newtype(declaration) => declaration.name.as_str() == name,
            TypeDeclaration::Enum(declaration) => declaration.name.as_str() == name,
        });
    assert!(found, "missing namespace type {name}");
}

fn assert_has_variant(declaration: &EnumDeclaration, name: &str) {
    assert!(
        declaration
            .variants
            .iter()
            .any(|variant| variant.name.as_str() == name),
        "missing variant {name} on {}",
        declaration.name.as_str()
    );
}
