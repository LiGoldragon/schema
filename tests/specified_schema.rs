use schema_next::{
    ImportResolver, SchemaEngine, SchemaIdentity, SourceDeclaration, SourceDeclarations,
    SpecifiedDeclarationBody, SpecifiedPayloadBody, SpecifiedPayloadShape, SpecifiedSchema,
    TypeDeclaration, TypeReference,
};

fn specified_fixture() -> SpecifiedSchema {
    SchemaEngine::default()
        .lower_specified_source(
            include_str!("fixtures/specified-ir.schema"),
            SchemaIdentity::new("schema:specified-ir", "0.1.0"),
        )
        .expect("specified IR fixture lowers")
}

#[test]
fn specified_schema_makes_root_variants_and_payload_shapes_explicit() {
    let specified = specified_fixture();
    let input = specified.input().as_enum().expect("input root is an enum");
    let record = input
        .variant_named("Record")
        .expect("Record variant exists");
    let payload = record.payload().expect("Record has a payload");

    assert_eq!(payload.reference(), &TypeReference::new("Record"));
    assert_eq!(
        payload.immediate_body(),
        Some(&SpecifiedPayloadBody::Newtype(TypeReference::new(
            "RecordRequest"
        ))),
        "the immediate payload body preserves the Record newtype boundary"
    );

    let shape = payload.shape(&specified);
    let fields = shape
        .as_struct()
        .expect("Record payload derives transparent newtypes to the struct shape");
    let field_pairs = fields
        .iter()
        .map(|field| {
            (
                field.name().as_str(),
                field.reference().plain_name().map(|name| name.as_str()),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        field_pairs,
        vec![
            ("record_entry", Some("Entry")),
            ("record_reason", Some("Justification")),
        ]
    );
}

#[test]
fn specified_schema_keeps_namespace_declarations_as_explicit_data() {
    let specified = specified_fixture();
    let entry = specified
        .declaration_named("Entry")
        .expect("Entry declaration exists");
    let SpecifiedDeclarationBody::Struct(fields) = entry.body() else {
        panic!("Entry should be a struct declaration");
    };

    let field_pairs = fields
        .iter()
        .map(|field| (field.name().as_str(), field.reference()))
        .collect::<Vec<_>>();
    assert_eq!(
        field_pairs,
        vec![
            (
                "domains",
                &TypeReference::Vector(Box::new(TypeReference::new("Domain")))
            ),
            ("kind", &TypeReference::new("Kind")),
            ("description", &TypeReference::new("Description")),
            ("certainty", &TypeReference::new("Certainty")),
            ("importance", &TypeReference::new("Importance")),
            ("privacy", &TypeReference::new("Privacy")),
            ("referents", &TypeReference::new("Referents")),
        ],
        "the explicit field-prefix syntax lowers to field names plus references"
    );

    let kind = specified
        .declaration_named("Kind")
        .expect("Kind declaration exists");
    let variants = kind.body().as_enum().expect("Kind is an enum");
    let variant_names = variants
        .iter()
        .map(|variant| variant.name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        variant_names,
        vec![
            "Decision",
            "Principle",
            "Correction",
            "Clarification",
            "Constraint"
        ],
        "enum alternatives are explicit once, on the enum declaration"
    );
}

#[test]
fn specified_schema_summarizes_enum_payloads_without_recursive_expansion() {
    let specified = specified_fixture();
    let output = specified
        .output()
        .as_enum()
        .expect("output root is an enum");
    let accepted = output
        .variant_named("RecordAccepted")
        .expect("RecordAccepted variant exists");
    let payload = accepted.payload().expect("RecordAccepted has a payload");

    assert_eq!(payload.reference(), &TypeReference::new("RecordAccepted"));
    assert_eq!(
        payload.immediate_body(),
        Some(&SpecifiedPayloadBody::Newtype(TypeReference::new(
            "RecordIdentifier"
        )))
    );
    assert_eq!(
        payload.shape(&specified),
        SpecifiedPayloadShape::Scalar(TypeReference::String),
        "transparent newtype chains derive the scalar leaf without duplicating declaration nodes"
    );

    let certainty = specified
        .declaration_named("Certainty")
        .expect("Certainty declaration exists");
    let SpecifiedDeclarationBody::Newtype(reference) = certainty.body() else {
        panic!("Certainty should be a newtype declaration");
    };
    assert_eq!(reference, &TypeReference::new("Magnitude"));
}

#[test]
fn specified_schema_is_a_rkyv_serializable_data_value() {
    let specified = specified_fixture();
    let bytes = specified
        .to_binary_bytes()
        .expect("specified schema encodes to rkyv bytes");
    let recovered = SpecifiedSchema::from_binary_bytes(&bytes)
        .expect("specified schema decodes from rkyv bytes");

    assert_eq!(
        recovered, specified,
        "the fully specified schema value should round-trip as binary data"
    );
}

#[test]
fn specified_payload_shape_is_derived_not_stored_on_payload() {
    let specified = specified_fixture();
    let output = specified
        .output()
        .as_enum()
        .expect("output root is an enum");
    let accepted = output
        .variant_named("RecordAccepted")
        .expect("RecordAccepted variant exists");
    let payload = accepted.payload().expect("RecordAccepted has a payload");

    assert_eq!(payload.reference(), &TypeReference::new("RecordAccepted"));
    assert_eq!(
        payload.immediate_body(),
        Some(&SpecifiedPayloadBody::Newtype(TypeReference::new(
            "RecordIdentifier"
        ))),
        "the canonical payload stores the immediate role boundary"
    );

    let bytes = specified
        .to_binary_bytes()
        .expect("specified schema encodes to rkyv bytes");
    let recovered = SpecifiedSchema::from_binary_bytes(&bytes)
        .expect("specified schema decodes from rkyv bytes");
    let recovered_payload = recovered
        .output()
        .as_enum()
        .expect("output root is an enum")
        .variant_named("RecordAccepted")
        .expect("RecordAccepted variant exists")
        .payload()
        .expect("RecordAccepted has a payload");

    assert_eq!(
        recovered_payload.immediate_body(),
        Some(&SpecifiedPayloadBody::Newtype(TypeReference::new(
            "RecordIdentifier"
        ))),
        "rkyv carries the immediate role boundary"
    );
    assert_eq!(
        recovered_payload.shape(&recovered),
        SpecifiedPayloadShape::Scalar(TypeReference::String),
        "terminal shape is recomputed from the recovered schema, not archived on the payload"
    );
}

#[test]
fn specified_schema_content_hash_excludes_derived_payload_shape_cache() {
    let specified = specified_fixture();
    let output = specified
        .output()
        .as_enum()
        .expect("output root is an enum");
    let accepted = output
        .variant_named("RecordAccepted")
        .expect("RecordAccepted variant exists");
    let payload = accepted.payload().expect("RecordAccepted has a payload");

    assert_eq!(
        payload.shape(&specified),
        SpecifiedPayloadShape::Scalar(TypeReference::String),
        "the test exercises the derived terminal shape before hashing"
    );

    assert_eq!(
        specified
            .content_hash()
            .expect("specified schema hashes")
            .to_hex(),
        "b1b8b5aad9a636ebf66c9f24999531560f4a291df93c2d38a24ae204fb57d9ab",
        "the golden hash is over the canonical specified-schema bytes; adding a stored derived shape cache changes this value"
    );
}

#[test]
fn specified_schema_projects_back_to_the_schema_declaration_codec() {
    let specified = specified_fixture();
    let entry = specified
        .declaration_named("Entry")
        .expect("Entry declaration exists");
    let value = entry.body().to_source_declaration_value();
    let declaration = SourceDeclaration::new(entry.name().clone(), Some(value));

    assert_eq!(
        declaration.to_schema_text(),
        "(Entry { domains.(Vector Domain) Kind Description Certainty Importance Privacy Referents })",
        "SpecifiedSchema should project to typed schema declarations without a hand printer"
    );

    let input = specified.input().as_enum().expect("input root is an enum");
    let record = input
        .variant_named("Record")
        .expect("Record variant exists");
    let value = record
        .payload()
        .expect("Record payload exists")
        .to_help_source_declaration_value(&specified);
    let declaration = SourceDeclaration::new(record.name().clone(), Some(value));
    assert_eq!(
        declaration.to_schema_text(),
        "(Record { record_entry.Entry record_reason.Justification })",
        "root Help projection should use the explicit payload shape from SpecifiedSchema"
    );
}

#[test]
fn specified_schema_projects_self_tagged_variants_through_schema_codec() {
    let specified = SchemaEngine::default()
        .lower_specified_source(
            "[]\n[Domain]\n{\n  Domain [(Health)]\n  Health [Body Mind]\n}",
            SchemaIdentity::new("schema:self-tag", "0.1.0"),
        )
        .expect("self-tag fixture lowers");
    let domain = specified
        .declaration_named("Domain")
        .expect("Domain declaration exists");
    let value = domain.body().to_source_declaration_value();
    let declaration = SourceDeclaration::new(domain.name().clone(), Some(value));
    let text = declaration.to_schema_text();

    assert_eq!(text, "(Domain [(Health)])");

    let declarations =
        SourceDeclarations::from_schema_text(&text).expect("projected self-tag decodes");
    let declaration = declarations
        .declarations()
        .first()
        .expect("one projected declaration");
    let Some(schema_next::SourceDeclarationValue::Enum(body)) = declaration.value() else {
        panic!("projected Domain should decode as an enum declaration");
    };
    assert!(
        matches!(
            body.variants().first(),
            Some(schema_next::SourceVariantSignature::SelfTagged(_))
        ),
        "the declaration codec preserves the payload-bearing self-tagged form"
    );

    let lowered = SchemaEngine::default()
        .lower_schema_source(
            &schema_next::SchemaSource::from_schema_text(
                "[]\n[]\n{\n  Domain [(Health)]\n  Health [Body Mind]\n}",
            )
            .expect("reconstructed source decodes"),
            SchemaIdentity::new("schema:self-tag-round-trip", "0.1.0"),
        )
        .expect("reconstructed schema lowers");
    let Some(TypeDeclaration::Enum(domain)) = lowered.type_named("Domain") else {
        panic!("Domain should lower as an enum");
    };
    let variant = domain
        .variants
        .first()
        .expect("Domain has a Health variant after round trip");

    assert_eq!(variant.name.as_str(), "Health");
    assert_eq!(
        variant.payload.as_ref(),
        Some(&TypeReference::new("Health")),
        "self-tagged projection decodes as a payload-bearing variant"
    );
}

#[test]
fn specified_source_lowers_with_embedded_import_resolver() {
    let resolver = ImportResolver::new().with_module_source(
        "dependency-crate",
        "domain",
        "0.1.0",
        "{}\n[]\n[]\n{\n  ImportedThing String\n}",
    );
    let specified = SchemaEngine::default()
        .lower_specified_source_with_resolver(
            "{ ImportedThing dependency-crate:domain:ImportedThing }\n[Use]\n[]\n{\n  Use ImportedThing\n}",
            SchemaIdentity::new("consumer", "0.1.0"),
            &resolver,
        )
        .expect("specified schema lowers with an embedded dependency module");

    assert_eq!(specified.resolved_imports().len(), 1);
    assert_eq!(
        specified.resolved_imports()[0].source().rust_path(),
        "dependency_crate::schema::domain::ImportedThing"
    );
}
