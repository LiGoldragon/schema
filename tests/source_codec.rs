use std::fs;

use schema_next::{
    RelationDeclaration, SchemaEngine, SchemaError, SchemaIdentity, SchemaSourceArtifact,
    TypeDeclaration,
};

fn source_codec_fixture(name: &str) -> String {
    fs::read_to_string(format!("tests/fixtures/source-codec/{name}.schema"))
        .unwrap_or_else(|error| panic!("read source-codec schema fixture {name}: {error}"))
        .trim_end()
        .to_owned()
}

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
        "{}\n[Record Observe]\n[RecordAccepted RecordsObserved]\n{\n  Record Entry\n  Observe Query\n  RecordAccepted RecordIdentifier\n  RecordsObserved RecordSet\n  Topic { string String }\n  Topics { values (Vec Topic) }\n  Description { string String }\n  RecordIdentifier { integer Integer }\n  Entry { topics Topics kind Kind description Description magnitude Magnitude }\n  Query { topic Topic kind Kind }\n  RecordSet { entries (Vec Entry) }\n  Kind [Decision Principle Correction Clarification Constraint]\n  Magnitude [Minimum VeryLow Low Medium High VeryHigh Maximum]\n}",
        canonical,
        "source codec should write one canonical schema source surface"
    );
}

#[test]
fn schema_source_lowers_through_engine_schema_source_endpoint() {
    let source = fs::read_to_string("tests/fixtures/spirit-crate/schema/lib.schema")
        .expect("read schema source fixture");
    let identity = SchemaIdentity::new("spirit-next:lib", "0.1.0");
    let engine = SchemaEngine::default();
    let source_artifact =
        SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let through_endpoint = engine
        .lower_schema_source(source_artifact.source(), identity.clone())
        .expect("schema source endpoint lowers");
    let through_object = source_artifact
        .source()
        .lower(&engine, identity)
        .expect("schema source object lowers");

    assert_eq!(
        through_endpoint, through_object,
        "schema source object and engine endpoint should lower the same typed schema"
    );
}

#[test]
fn schema_source_reference_fields_lower_to_canonical_field_names() {
    let source = source_codec_fixture("reference-fields");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");
    let Some(TypeDeclaration::Struct(entry)) = schema.type_named("Entry") else {
        panic!("Entry should lower to a struct");
    };

    let field_names = entry
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        field_names,
        vec!["record_identifier", "by_topic"],
        "schema-source lowering must preserve canonical derived field names"
    );
}

#[test]
fn namespace_enum_bare_variants_do_not_resolve_to_same_named_payloads() {
    let source = source_codec_fixture("namespace-enum-bare-variants");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");
    let Some(TypeDeclaration::Enum(kind)) = schema.type_named("Kind") else {
        panic!("Kind should lower to an enum");
    };

    let variants = kind
        .variants
        .iter()
        .map(|variant| (variant.name.as_str(), variant.payload.as_ref()))
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        vec![("Decision", None), ("Correction", None)],
        "bare namespace enum variants stay unit variants even when same-named schema types exist"
    );
}

#[test]
fn namespace_inline_enum_variant_declarations_are_public_payload_types() {
    let source = source_codec_fixture("namespace-inline-enum-variants");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert_eq!(
        schema
            .namespace()
            .iter()
            .map(|declaration| (declaration.name().as_str(), declaration.visibility()))
            .collect::<Vec<_>>(),
        vec![
            ("Craft", schema_next::Visibility::Public),
            ("Information", schema_next::Visibility::Public),
            ("Domain", schema_next::Visibility::Public),
            ("Entry", schema_next::Visibility::Public),
        ],
        "inline enum variants exposed through a public namespace enum must be public payload types"
    );
    let Some(TypeDeclaration::Enum(domain)) = schema.type_named("Domain") else {
        panic!("Domain should lower to an enum");
    };
    assert_eq!(
        domain
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.name.as_str(),
                    variant
                        .payload
                        .as_ref()
                        .and_then(schema_next::TypeReference::plain_name)
                        .map(schema_next::Name::as_str),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Craft", Some("Craft")),
            ("Information", Some("Information"))
        ]
    );
}

#[test]
fn root_header_bare_names_resolve_to_exported_namespace_payloads() {
    let source = source_codec_fixture("root-header-bare-names");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    let input = schema.input();
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
        schema.type_named("Lookup").is_some(),
        "root header should resolve through the exported namespace object"
    );
    let Some(TypeDeclaration::Newtype(lookup)) = schema.type_named("Lookup") else {
        panic!("bare namespace binding should lower to a newtype");
    };
    assert_eq!(
        lookup.reference.plain_name().map(schema_next::Name::as_str),
        Some("RecordIdentifier")
    );
}

#[test]
fn root_header_inline_declarations_are_exported_namespace_payloads() {
    let source = source_codec_fixture("root-inline-payloads");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert!(
        schema.type_named("Lookup").is_some(),
        "inline root declaration should enter the exported namespace"
    );
    assert!(
        schema.type_named("Count").is_some(),
        "second inline root declaration should enter the exported namespace"
    );
    assert_eq!(
        schema.input().variants[0]
            .payload
            .as_ref()
            .and_then(schema_next::TypeReference::plain_name)
            .map(schema_next::Name::as_str),
        Some("Lookup")
    );
    assert_eq!(
        schema
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

#[test]
fn root_payload_field_declarations_are_exported_namespace_types() {
    let source = source_codec_fixture("root-payload-field-declarations");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert_eq!(
        schema
            .namespace()
            .iter()
            .map(|declaration| (declaration.name().as_str(), declaration.visibility()))
            .collect::<Vec<_>>(),
        vec![
            ("Topic", schema_next::Visibility::Public),
            ("Description", schema_next::Visibility::Public),
            ("Record", schema_next::Visibility::Public),
        ]
    );
    let Some(TypeDeclaration::Newtype(topic)) = schema.type_named("Topic") else {
        panic!("Topic should lower to a public newtype");
    };
    assert_eq!(topic.reference, schema_next::TypeReference::String);
    let Some(TypeDeclaration::Struct(record)) = schema.type_named("Record") else {
        panic!("Record should lower to a public struct");
    };
    assert_eq!(
        record
            .fields
            .iter()
            .map(|field| {
                (
                    field.name.as_str(),
                    field.reference.plain_name().map(schema_next::Name::as_str),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("topic", Some("Topic")),
            ("description", Some("Description"))
        ]
    );
}

#[test]
fn later_inline_payloads_resolve_root_payload_field_declarations() {
    let source = source_codec_fixture("later-inline-payloads");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert_eq!(
        schema
            .namespace()
            .iter()
            .map(|declaration| (declaration.name().as_str(), declaration.visibility()))
            .collect::<Vec<_>>(),
        vec![
            ("Topic", schema_next::Visibility::Public),
            ("Description", schema_next::Visibility::Public),
            ("Record", schema_next::Visibility::Public),
            ("ByTopic", schema_next::Visibility::Private),
            ("ByDescription", schema_next::Visibility::Private),
            ("Select", schema_next::Visibility::Public),
        ]
    );
    let Some(TypeDeclaration::Newtype(by_topic)) = schema.type_named("ByTopic") else {
        panic!("ByTopic should lower to a private newtype helper");
    };
    assert_eq!(
        by_topic
            .reference
            .plain_name()
            .map(schema_next::Name::as_str),
        Some("Topic")
    );
}

#[test]
fn trailing_namespace_can_reference_root_payload_field_declarations() {
    let source = source_codec_fixture("trailing-namespace-reference");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    let Some(TypeDeclaration::Newtype(wrapper)) = schema.type_named("Wrapper") else {
        panic!("Wrapper should lower to a public newtype");
    };
    assert_eq!(
        wrapper
            .reference
            .plain_name()
            .map(schema_next::Name::as_str),
        Some("Topic")
    );
}

#[test]
fn duplicate_inline_and_namespace_declarations_are_errors() {
    let source = source_codec_fixture("duplicate-inline-and-namespace");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let error = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect_err("duplicate Topic declaration should fail");

    assert!(matches!(
        error,
        SchemaError::DuplicateSourceDeclaration { name } if name == "Topic"
    ));
}

#[test]
fn duplicate_inline_declarations_are_errors() {
    let source = source_codec_fixture("duplicate-inline-fields");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let error = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect_err("duplicate inline Topic declaration should fail");

    assert!(matches!(
        error,
        SchemaError::DuplicateSourceDeclaration { name } if name == "Topic"
    ));
}

#[test]
fn schema_source_artifact_round_trips_through_binary_archive() {
    let source = source_codec_fixture("root-inline-payloads");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");
    let bytes = artifact
        .to_binary_bytes()
        .expect("schema source artifact archives");
    let recovered =
        SchemaSourceArtifact::from_binary_bytes(&bytes).expect("schema source artifact restores");

    assert_eq!(artifact, recovered);
    assert_eq!(recovered.to_schema_text(), source);
}

#[test]
fn schema_source_lowers_relation_declarations() {
    let source = source_codec_fixture("relations");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");

    assert_eq!(
        artifact.to_schema_text(),
        source,
        "relation declarations should round-trip through canonical schema source"
    );

    let bytes = artifact
        .to_binary_bytes()
        .expect("schema source artifact archives");
    let recovered =
        SchemaSourceArtifact::from_binary_bytes(&bytes).expect("schema source artifact restores");
    assert_eq!(artifact, recovered);

    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:domain", "0.1.0"),
        )
        .expect("schema source lowers");

    assert_eq!(schema.relations().len(), 2);
    let RelationDeclaration::Equivalence(values) = &schema.relations()[0];
    let paths = values
        .iter()
        .map(|value| {
            value
                .path()
                .iter()
                .map(schema_next::Name::as_str)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths,
        vec![
            vec!["Technology", "Hardware", "Networking"],
            vec!["Technology", "Software", "Distributed", "Networking"]
        ],
        "equivalence values lower as schema-name paths"
    );
}

#[test]
fn schema_source_lowers_stream_declarations_and_variant_relations() {
    let source = source_codec_fixture("stream-relations");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");

    assert_eq!(
        artifact.to_schema_text(),
        source,
        "stream declarations and stream variant relations encode as schema source"
    );

    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");

    assert_eq!(schema.streams().len(), 1);
    let stream = &schema.streams()[0];
    assert_eq!(stream.name.as_str(), "RecordStream");
    assert_eq!(
        stream.token.plain_name().map(schema_next::Name::as_str),
        Some("SubscriptionToken")
    );
    assert_eq!(
        stream.opened.plain_name().map(schema_next::Name::as_str),
        Some("SubscriptionReceipt")
    );
    assert_eq!(
        stream.event.plain_name().map(schema_next::Name::as_str),
        Some("RuntimeEvent")
    );
    assert_eq!(
        stream.close.plain_name().map(schema_next::Name::as_str),
        Some("SubscriptionToken")
    );
    assert!(
        schema.type_named("RecordStream").is_none(),
        "stream declarations are schema metadata, not namespace data types"
    );

    let watch_relation = schema.input().variants[0]
        .stream_relation
        .as_ref()
        .expect("Watch opens a stream");
    assert!(matches!(
        watch_relation,
        schema_next::StreamRelation::Opens(name) if name.as_str() == "RecordStream"
    ));

    let Some(TypeDeclaration::Enum(runtime_event)) = schema.type_named("RuntimeEvent") else {
        panic!("RuntimeEvent should lower to an enum");
    };
    let event_relation = runtime_event.variants[0]
        .stream_relation
        .as_ref()
        .expect("RecordChanged belongs to a stream");
    assert!(matches!(
        event_relation,
        schema_next::StreamRelation::Belongs(name) if name.as_str() == "RecordStream"
    ));
}

#[test]
fn source_enum_variants_are_typed_structural_macro_nodes() {
    let source = source_codec_fixture("structural-variant-nodes");
    let artifact = SchemaSourceArtifact::from_schema_text(&source).expect("schema source decodes");

    assert_eq!(
        artifact.to_schema_text(),
        source,
        "structural enum variant nodes encode back to the same schema source surface"
    );

    let input_variants = artifact.source().input().body().variants();
    assert_eq!(input_variants[0].name().as_str(), "Reserved");
    assert_eq!(input_variants[0].payload(), None);
    assert_eq!(input_variants[1].name().as_str(), "Record");
    assert_eq!(
        input_variants[1].payload(),
        Some(&schema_next::SourceReference::Plain(
            schema_next::Name::new("Entry")
        ))
    );
    assert_eq!(input_variants[2].name().as_str(), "Inline");
    assert_eq!(
        input_variants[2].payload(),
        None,
        "inline declaration payload is not a reference at the source layer"
    );

    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("example:lib", "0.1.0"),
        )
        .expect("schema source lowers");
    let variants = schema
        .input()
        .variants
        .iter()
        .map(|variant| {
            (
                variant.name.as_str(),
                variant
                    .payload
                    .as_ref()
                    .and_then(schema_next::TypeReference::plain_name)
                    .map(schema_next::Name::as_str),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        variants,
        vec![
            ("Reserved", None),
            ("Record", Some("Entry")),
            ("Inline", Some("Inline")),
        ],
        "lowering happens after structural variant selection"
    );
    assert!(
        schema.type_named("Inline").is_some(),
        "inline structural payload is exported as the variant's same-named type"
    );
}

#[test]
fn source_enum_variant_reports_structural_macro_expected_shapes() {
    let source = source_codec_fixture("unsupported-three-object-variant");
    let error = SchemaSourceArtifact::from_schema_text(&source)
        .expect_err("three-object variant signature is not a supported structural case");

    let SchemaError::UnsupportedMacroNodeStructure {
        position,
        expected,
        found,
    } = error
    else {
        panic!("expected structural macro-node error, got {error:?}");
    };

    assert_eq!(position, "SourceVariantSignature");
    assert_eq!(found, "parenthesis");
    assert!(
        expected.iter().any(|case| case.contains("Unit")),
        "diagnostic names the unit structural case"
    );
    assert!(
        expected.iter().any(|case| case.contains("Data")),
        "diagnostic names the data structural case"
    );
    assert!(
        expected.iter().any(|case| case.contains("Streaming")),
        "diagnostic names the streaming structural case"
    );
}
