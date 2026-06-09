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

fn lower(source: &str) -> schema_next::Schema {
    SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("collections:lib", "0.1.0"))
        .expect("schema lowers")
}

fn roots(namespace: &str) -> String {
    format!("[] [] {{ {namespace} }}")
}

fn struct_fields<'schema>(
    schema: &'schema schema_next::Schema,
    name: &str,
) -> &'schema [schema_next::FieldDeclaration] {
    match schema.type_named(name).expect("type present") {
        TypeDeclaration::Struct(declaration) => &declaration.fields,
        TypeDeclaration::Newtype(_) | TypeDeclaration::Enum(_) => {
            panic!("{name} should be a struct")
        }
    }
}

fn single_reference<'schema>(
    schema: &'schema schema_next::Schema,
    name: &str,
) -> &'schema TypeReference {
    match schema.type_named(name).expect("type present") {
        TypeDeclaration::Newtype(declaration) => &declaration.reference,
        TypeDeclaration::Struct(_) | TypeDeclaration::Enum(_) => {
            panic!("{name} should be a single-reference declaration")
        }
    }
}

#[test]
fn vec_field_lowers_to_vector_reference() {
    let schema = lower(&roots("Service String Cluster (Vec Service)"));
    assert_eq!(
        single_reference(&schema, "Cluster"),
        &TypeReference::Vector(Box::new(TypeReference::new("Service")))
    );
}

#[test]
fn scalar_field_names_lower_to_reserved_references() {
    let schema = lower(&roots(
        "Entry { string String integer Integer boolean Boolean path Path }",
    ));
    let fields = struct_fields(&schema, "Entry");
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
    let schema = lower(&roots(
        "Query { optionalInteger (Optional Integer) stringVector (Vec String) booleanByString (Map (String Boolean)) optionalPath (Optional Path) }",
    ));
    let fields = struct_fields(&schema, "Query");
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
            "[] [] { String { integer Integer } }",
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
    let schema = lower(&roots(
        "NodeName String NodeProposal String Cluster (Map (NodeName NodeProposal))",
    ));
    assert_eq!(
        single_reference(&schema, "Cluster"),
        &TypeReference::Map(
            Box::new(TypeReference::new("NodeName")),
            Box::new(TypeReference::new("NodeProposal")),
        )
    );
}

#[test]
fn option_field_lowers_to_optional_reference() {
    let schema = lower(&roots("Cache String Cluster (Optional Cache)"));
    assert_eq!(
        single_reference(&schema, "Cluster"),
        &TypeReference::Optional(Box::new(TypeReference::new("Cache")))
    );
}

#[test]
fn square_bracket_field_is_not_vec_type_syntax() {
    let error = SchemaEngine::default()
        .lower_source(
            "[] [] { Service { string String } Cluster { service [Service] } }",
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
            "[] [] { NodeName { string String } NodeProposal { string String } Cluster { nodes {NodeName NodeProposal} } }",
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
    let schema = lower(&roots(
        "Trust { string String } Service { string String } Cluster { trust Trust serviceVector (Vec Service) optionalTrust (Optional Trust) }",
    ));
    let fields = struct_fields(&schema, "Cluster");
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
    let schema = lower(&roots(
        "Leaf String Key String Nest (Map (Key (Vec (Optional Leaf))))",
    ));
    assert_eq!(
        single_reference(&schema, "Nest"),
        &TypeReference::Map(
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
    let schema = lower(
        "[] [(Projected (Map (NodeName NodeConfig)))] { NodeName { string String } NodeConfig { string String } }",
    );
    let payload = schema.output().variants[0]
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
            "[] [] { Leaf { string String } Bad { hashSet (HashSet (Vec Leaf)) } }",
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
            "[] [] { Leaf { string String } Bad { map (Map (Leaf)) } }",
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
