use std::path::PathBuf;

use schema::{Leg, LoadedSchema, Name, Projection, RouteBody, UpgradeAnnotation};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/schema-e2e")
        .join(name)
}

fn schema_name(value: &str) -> Name {
    Name::new(value).unwrap()
}

#[test]
fn reads_schema_file_with_local_imports_and_lowers_routes() {
    let loaded = LoadedSchema::read_path(fixture("spirit-v0-1-1.schema")).unwrap();
    let assembled = loaded.assembled();

    assert_eq!(assembled.imports().len(), 3);
    assert!(
        assembled
            .types()
            .any(|schema_type| schema_type.name() == &schema_name("Magnitude"))
    );
    assert!(
        assembled
            .types()
            .any(|schema_type| schema_type.name() == &schema_name("SemaOperation"))
    );
    assert!(
        assembled
            .types()
            .any(|schema_type| schema_type.name() == &schema_name("Source"))
    );
    assert!(
        assembled
            .types()
            .any(|schema_type| schema_type.name() == &schema_name("Stamp"))
    );

    assert_eq!(assembled.routes().len(), 4);

    let declaration = assembled
        .routes()
        .iter()
        .find(|route| {
            route.root().as_str() == "State" && route.endpoint().name().as_str() == "Declaration"
        })
        .unwrap();
    assert_eq!(declaration.leg(), Leg::Ordinary);
    assert_eq!(declaration.root_slot(), 0);
    assert_eq!(declaration.endpoint().slot(), 1);
    assert_eq!(
        declaration.body(),
        &RouteBody::Type(schema_name("Declaration"))
    );

    let observe_records = assembled
        .routes()
        .iter()
        .find(|route| {
            route.root().as_str() == "Observe" && route.endpoint().name().as_str() == "Records"
        })
        .unwrap();
    assert_eq!(observe_records.root_slot(), 2);
    assert_eq!(
        observe_records.body(),
        &RouteBody::Type(schema_name("RecordQuery"))
    );
}

#[test]
fn plans_upgrade_from_schema_files() {
    let previous = LoadedSchema::read_path(fixture("spirit-v0-1.schema")).unwrap();
    let current = LoadedSchema::read_path(fixture("spirit-v0-1-1.schema")).unwrap();

    let plan = current
        .assembled()
        .plan_upgrade_from(previous.assembled())
        .unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Annotated {
            name,
            annotation: UpgradeAnnotation::Migrate(migrated),
        } if name.as_str() == "Entry" && migrated.as_str() == "Entry"
    )));
}

#[test]
fn rejects_scalar_header_form() {
    let result = schema::Schema::parse_str("{} [(State Statement)] [] [] {} []");

    assert!(result.is_err());
}
