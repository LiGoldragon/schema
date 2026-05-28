//! Collection + Option type references.
//!
//! A struct field or enum-variant payload can now wrap its referenced
//! type in a collection or option. The surface forms are typed NOTA:
//! `(Vec T)`, `(Map (K V))`, and `(Optional T)`. They lower to
//! `TypeReference::Vector / Map / Optional`. Bare-symbol fields keep
//! the plain shape, so non-collection schemas stay byte-identical.

use schema_next::{SchemaEngine, SchemaIdentity, TypeDeclaration, TypeReference};

fn lower(source: &str) -> schema_next::Asschema {
    SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("collections:lib", "0.1.0"))
        .expect("schema lowers")
}

fn struct_fields<'asschema>(
    asschema: &'asschema schema_next::Asschema,
    name: &str,
) -> &'asschema [schema_next::FieldDeclaration] {
    match asschema.type_named(name).expect("type present") {
        TypeDeclaration::Struct(declaration) | TypeDeclaration::Newtype(declaration) => {
            &declaration.fields
        }
        TypeDeclaration::Enum(_) => panic!("{name} should be a struct"),
    }
}

#[test]
fn vec_field_lowers_to_vector_reference() {
    let asschema = lower("() () { Service [Text] Cluster [(Vec Service)] }");
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "service_vector");
    assert_eq!(
        fields[0].reference,
        TypeReference::Vector(Box::new(TypeReference::new("Service")))
    );
}

#[test]
fn key_value_field_lowers_to_map_reference() {
    let asschema = lower(
        "() () { NodeName [Text] NodeProposal [Text] Cluster [(Map (NodeName NodeProposal))] }",
    );
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "node_proposal_by_node_name");
    assert_eq!(
        fields[0].reference,
        TypeReference::Map(
            Box::new(TypeReference::new("NodeName")),
            Box::new(TypeReference::new("NodeProposal")),
        )
    );
}

#[test]
fn option_field_lowers_to_optional_reference() {
    let asschema = lower("() () { Cache [Text] Cluster [(Optional Cache)] }");
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "optional_cache");
    assert_eq!(
        fields[0].reference,
        TypeReference::Optional(Box::new(TypeReference::new("Cache")))
    );
}

#[test]
fn square_bracket_field_is_not_vec_type_syntax() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { Service [Text] Cluster [[Service]] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("raw square bracket is not a Vec reference");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "SquareBracket".to_owned(),
            argument_count: 1,
        }
    );
}

#[test]
fn brace_field_is_not_map_type_syntax() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { NodeName [Text] NodeProposal [Text] Cluster [{NodeName NodeProposal}] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("raw brace map is not a Map reference");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "Brace".to_owned(),
            argument_count: 2,
        }
    );
}

#[test]
fn collection_field_and_plain_field_coexist_in_one_struct() {
    let asschema = lower(
        "() () { Trust [Text] Service [Text] Cluster [Trust (Vec Service) (Optional Trust)] }",
    );
    let fields = struct_fields(&asschema, "Cluster");
    // Bare symbol stays the legacy plain field (name derived from type).
    assert_eq!(fields[0].name.as_str(), "trust");
    assert_eq!(fields[0].reference, TypeReference::new("Trust"));
    assert_eq!(fields[1].name.as_str(), "service_vector");
    assert!(matches!(fields[1].reference, TypeReference::Vector(_)));
    assert_eq!(fields[2].name.as_str(), "optional_trust");
    assert!(matches!(fields[2].reference, TypeReference::Optional(_)));
}

#[test]
fn nested_collections_lower_recursively() {
    // A map whose value is itself a vector of an optional leaf.
    let asschema =
        lower("() () { Leaf [Text] Key [Text] Nest [(Map (Key (Vec (Optional Leaf))))] }");
    let fields = struct_fields(&asschema, "Nest");
    assert_eq!(
        fields[0].reference,
        TypeReference::Map(
            Box::new(TypeReference::new("Key")),
            Box::new(TypeReference::Vector(Box::new(TypeReference::Optional(
                Box::new(TypeReference::new("Leaf"))
            )))),
        )
    );
}

#[test]
fn collection_payload_lowers_in_an_output_variant() {
    // Output variant carrying a map payload — the projection result
    // shape Horizon needs (Projected -> a map of node configs).
    let asschema =
        lower("() ((Projected (Map (NodeName NodeConfig)))) { NodeName [Text] NodeConfig [Text] }");
    let payload = asschema.output().variants[0]
        .payload
        .as_ref()
        .expect("projected payload");
    assert_eq!(
        payload,
        &TypeReference::Map(
            Box::new(TypeReference::new("NodeName")),
            Box::new(TypeReference::new("NodeConfig")),
        )
    );
}

#[test]
fn unknown_collection_head_is_rejected() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { Leaf [Text] Bad [(HashSet (Vec Leaf))] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("unknown collection head should fail");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "HashSet".to_owned(),
            argument_count: 2,
        }
    );
}

#[test]
fn map_with_wrong_argument_count_is_rejected() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { Leaf [Text] Bad [(Map (Leaf))] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("Map needs two arguments");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "Map".to_owned(),
            argument_count: 1,
        }
    );
}
