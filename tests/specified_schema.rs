use schema_next::{
    SchemaEngine, SchemaIdentity, SpecifiedDeclarationBody, SpecifiedPayloadBody,
    SpecifiedPayloadShape, SpecifiedSchema, TypeReference,
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

    let fields = payload
        .shape()
        .as_struct()
        .expect("Record payload follows transparent newtypes to the struct shape");
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
        payload.shape(),
        &SpecifiedPayloadShape::Scalar(TypeReference::String),
        "transparent newtype chains reach the scalar leaf without duplicating declaration nodes"
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
