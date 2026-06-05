use std::fs;

use schema_next::{
    SchemaEngine, SchemaError, SchemaIdentity, SchemaSourceArtifact, TypeDeclaration,
};

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
fn schema_source_lowers_to_same_schema_as_direct_source() {
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
        "schema source object should be a semantics-preserving stage before schema"
    );
}

#[test]
fn schema_source_reference_fields_lower_to_canonical_field_names() {
    let source = "{}\n[]\n[]\n{\n  Topic String\n  RecordIdentifier Integer\n  Entry { recordIdentifier RecordIdentifier byTopic (Map (Topic RecordIdentifier)) }\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
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
    let source =
        "{}\n[]\n[]\n{\n  Correction { Topic * }\n  Topic String\n  Kind [Decision Correction]\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
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
fn root_header_bare_names_resolve_to_exported_namespace_payloads() {
    let source = "{}\n[Lookup Count]\n[Found Counted]\n{\n  Lookup RecordIdentifier\n  Count Query\n  Found Entry\n  Counted Integer\n  RecordIdentifier Integer\n  Query { Topic * }\n  Topic String\n  Entry { Topic * }\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
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
    let Some(TypeDeclaration::Alias(lookup)) = schema.type_named("Lookup") else {
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
fn schema_source_artifact_round_trips_through_binary_archive() {
    let source = "{}\n[(Lookup { RecordIdentifier * }) (Count { Query * })]\n[]\n{\n  RecordIdentifier Integer\n  Query { Topic * }\n  Topic String\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");
    let bytes = artifact
        .to_binary_bytes()
        .expect("schema source artifact archives");
    let recovered =
        SchemaSourceArtifact::from_binary_bytes(&bytes).expect("schema source artifact restores");

    assert_eq!(artifact, recovered);
    assert_eq!(recovered.to_schema_text(), source);
}

#[test]
fn source_enum_variants_are_typed_structural_macro_nodes() {
    let source = "{}\n[Reserved (Record Entry) (Inline { Topic * })]\n[]\n{\n  Entry { Topic * }\n  Topic String\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("schema source decodes");

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
    let source = "{}\n[(Record Entry Extra)]\n[]\n{}";
    let error = SchemaSourceArtifact::from_schema_text(source)
        .expect_err("three-object variant signature is not a supported structural case");

    let SchemaError::UnsupportedMacroNodeStructure {
        position,
        expected,
        found,
    } = error
    else {
        panic!("expected structural macro-node error, got {error:?}");
    };

    assert_eq!(position, "EnumVariants");
    assert_eq!(found, "parenthesis");
    assert!(
        expected.iter().any(|case| case.contains("unit variant")),
        "diagnostic names the unit structural case"
    );
    assert!(
        expected.iter().any(|case| case.contains("data variant")),
        "diagnostic names the data structural case"
    );
}
