//! Collection + Option type references.
//!
//! A struct field or enum-variant payload can now wrap its referenced
//! type in a collection or option. The surface forms are Schema
//! type-reference objects:
//! `(Vec T)`, `(Map (K V))`, and `(Optional T)`. They lower to
//! `TypeReference::Vector / Map / Optional`. Bare-symbol fields keep
//! the declared-name shape, while reserved scalar names lower to
//! scalar references instead of pretending to be user namespace types.

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
    let asschema = lower(
        "() () { Service {| Service string String |} Cluster {| Cluster serviceVector (Vec Service) |} }",
    );
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "service_vector");
    assert_eq!(
        fields[0].reference,
        TypeReference::Vector(Box::new(TypeReference::new("Service")))
    );
}

#[test]
fn scalar_field_names_lower_to_reserved_references() {
    let asschema = lower(
        "() () { Entry {| Entry string String integer Integer boolean Boolean path Path |} }",
    );
    let fields = struct_fields(&asschema, "Entry");
    assert_eq!(fields[0].name.as_str(), "string");
    assert_eq!(fields[0].reference, TypeReference::String);
    assert_eq!(fields[1].name.as_str(), "integer");
    assert_eq!(fields[1].reference, TypeReference::Integer);
    assert_eq!(fields[2].name.as_str(), "boolean");
    assert_eq!(fields[2].reference, TypeReference::Boolean);
    assert_eq!(fields[3].name.as_str(), "path");
    assert_eq!(fields[3].reference, TypeReference::Path);
}

#[test]
fn scalar_references_nest_inside_collections() {
    let asschema = lower(
        "() () { Query {| Query optionalInteger (Optional Integer) stringVector (Vec String) booleanByString (Map (String Boolean)) optionalPath (Optional Path) |} }",
    );
    let fields = struct_fields(&asschema, "Query");
    assert_eq!(
        fields[0].reference,
        TypeReference::Optional(Box::new(TypeReference::Integer))
    );
    assert_eq!(
        fields[1].reference,
        TypeReference::Vector(Box::new(TypeReference::String))
    );
    assert_eq!(
        fields[2].reference,
        TypeReference::Map(
            Box::new(TypeReference::String),
            Box::new(TypeReference::Boolean)
        )
    );
    assert_eq!(
        fields[3].reference,
        TypeReference::Optional(Box::new(TypeReference::Path))
    );
}

#[test]
fn scalar_names_are_reserved_at_namespace_declaration_position() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { String [Integer] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("reserved scalar names cannot be user-declared schema types");
    assert_eq!(
        error,
        schema_next::SchemaError::ReservedScalarTypeName {
            name: "String".to_owned(),
        }
    );
}

#[test]
fn key_value_field_lowers_to_map_reference() {
    let asschema = lower(
        "() () { NodeName {| NodeName string String |} NodeProposal {| NodeProposal string String |} Cluster {| Cluster nodeProposalByNodeName (Map (NodeName NodeProposal)) |} }",
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
    let asschema = lower(
        "() () { Cache {| Cache string String |} Cluster {| Cluster optionalCache (Optional Cache) |} }",
    );
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
            "() () { Service {| Service string String |} Cluster {| Cluster service [Service] |} }",
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
            "() () { NodeName {| NodeName string String |} NodeProposal {| NodeProposal string String |} Cluster {| Cluster nodes {NodeName NodeProposal} |} }",
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
        "() () { Trust {| Trust string String |} Service {| Service string String |} Cluster {| Cluster trust Trust serviceVector (Vec Service) optionalTrust (Optional Trust) |} }",
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
    let asschema = lower(
        "() () { Leaf {| Leaf string String |} Key {| Key string String |} Nest {| Nest leafByKey (Map (Key (Vec (Optional Leaf)))) |} }",
    );
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
    let asschema = lower(
        "() ((Projected (Map (NodeName NodeConfig)))) { NodeName {| NodeName string String |} NodeConfig {| NodeConfig string String |} }",
    );
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
            "() () { Leaf {| Leaf string String |} Bad {| Bad hashSet (HashSet (Vec Leaf)) |} }",
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
            "() () { Leaf {| Leaf string String |} Bad {| Bad map (Map (Leaf)) |} }",
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
