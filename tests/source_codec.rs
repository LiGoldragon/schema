use std::fs;

use schema_next::{SchemaEngine, SchemaIdentity, SchemaSourceArtifact};

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
