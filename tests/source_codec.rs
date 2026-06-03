use std::fs;

use schema_next::{SchemaEngine, SchemaIdentity, SchemaSourceArtifact, TypeDeclaration};

#[test]
fn schema_source_artifact_round_trips_module_source_text() {
    let source = fs::read_to_string("tests/fixtures/spirit-crate/schema/lib.schema")
        .expect("read schema source fixture");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let canonical = artifact.to_schema_text();
    let recovered = SchemaSourceArtifact::from_schema_text(&canonical)
        .expect("canonical schema source decodes");

    assert_eq!(
        artifact, recovered,
        "canonical schema source text should recover the same source object"
    );
    assert_eq!(
        "{}\n[(Record Entry) (Observe Query)]\n[(RecordAccepted RecordIdentifier) (RecordsObserved RecordSet)]\n{\n  Topic { string String }\n  Topics { values (Vec Topic) }\n  Description { string String }\n  RecordIdentifier { integer Integer }\n  Entry { topics Topics kind Kind description Description magnitude Magnitude }\n  Query { topic Topic kind Kind }\n  RecordSet { entries (Vec Entry) }\n  Kind [Decision Principle Correction Clarification Constraint]\n  Magnitude [Minimum VeryLow Low Medium High VeryHigh Maximum]\n}",
        canonical,
        "source codec should write one canonical schema source surface"
    );
}

#[test]
fn schema_source_lowers_to_same_asschema_as_direct_source() {
    let source = fs::read_to_string("tests/fixtures/spirit-crate/schema/lib.schema")
        .expect("read schema source fixture");
    let identity = SchemaIdentity::new("spirit-next:lib", "0.1.0");
    let engine = SchemaEngine::default();
    let direct = engine
        .lower_source(&source, identity.clone())
        .expect("direct source lowers");
    let source_artifact =
        SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let via_source = source_artifact
        .source()
        .lower(&engine, identity)
        .expect("schema source object lowers");

    assert_eq!(
        direct, via_source,
        "schema source object should be a semantics-preserving stage before asschema"
    );
}

#[test]
fn root_header_bare_names_resolve_to_exported_namespace_payloads() {
    let source = "{}\n[Lookup Count]\n[Found Counted]\n{\n  Lookup RecordIdentifier\n  Count Query\n  Found Entry\n  Counted Integer\n  RecordIdentifier Integer\n  Query { Topic * }\n  Topic String\n  Entry { Topic * }\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
    let asschema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    let input = asschema.input();
    assert_eq!(input.variants[0].name.as_str(), "Lookup");
    assert_eq!(
        input.variants[0]
            .payload
            .as_ref()
            .and_then(schema_next::TypeReference::plain_name)
            .map(schema_next::Name::as_str),
        Some("Lookup")
    );
    assert_eq!(input.variants[1].name.as_str(), "Count");
    assert_eq!(
        input.variants[1]
            .payload
            .as_ref()
            .and_then(schema_next::TypeReference::plain_name)
            .map(schema_next::Name::as_str),
        Some("Count")
    );
    assert!(
        asschema.type_named("Lookup").is_some(),
        "root header should resolve through the exported namespace object"
    );
    let Some(TypeDeclaration::Alias(lookup)) = asschema.type_named("Lookup") else {
        panic!("bare namespace binding should lower to an alias");
    };
    assert_eq!(
        lookup.reference.plain_name().map(schema_next::Name::as_str),
        Some("RecordIdentifier")
    );
}

#[test]
fn root_header_inline_declarations_are_exported_namespace_payloads() {
    let source = "{}\n[(Lookup { RecordIdentifier * }) (Count { Query * })]\n[]\n{\n  RecordIdentifier Integer\n  Query { Topic * }\n  Topic String\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
    let asschema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert!(
        asschema.type_named("Lookup").is_some(),
        "inline root declaration should enter the exported namespace"
    );
    assert!(
        asschema.type_named("Count").is_some(),
        "second inline root declaration should enter the exported namespace"
    );
    assert_eq!(
        asschema.input().variants[0]
            .payload
            .as_ref()
            .and_then(schema_next::TypeReference::plain_name)
            .map(schema_next::Name::as_str),
        Some("Lookup")
    );
    assert_eq!(
        asschema
            .namespace()
            .iter()
            .map(|declaration| (declaration.name().as_str(), declaration.visibility()))
            .collect::<Vec<_>>(),
        vec![
            ("RecordIdentifier", schema_next::Visibility::Public),
            ("Query", schema_next::Visibility::Public),
            ("Topic", schema_next::Visibility::Public),
            ("Lookup", schema_next::Visibility::Public),
            ("Count", schema_next::Visibility::Public),
        ]
    );
}
