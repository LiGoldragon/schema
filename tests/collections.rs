//! Collection + Option type references.
//!
//! A struct field or enum-variant payload can now wrap its referenced
//! type in a collection or option. The surface forms are explicit
//! macro invocations — `(@Vec (T))`, `(@KeyValue (K V))`, `(@Option
//! (T))` — and lower to the `TypeReference::Vector / Map / Optional`
//! variants. Collection fields are written as explicit pairs inside a
//! struct body: `(fieldName (@Vec (T)))`. Bare-symbol fields keep the
//! legacy plain shape, so non-collection schemas stay byte-identical.
//!
//! The map keyword is `KeyValue` (record 1045 dropped the redundant
//! `Map` suffix); the emitter still produces `BTreeMap<K, V>`.

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
    let asschema = lower("() () { Service [Text] Cluster [(services (@Vec (Service)))] }");
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "services");
    assert_eq!(
        fields[0].reference,
        TypeReference::Vector(Box::new(TypeReference::new("Service")))
    );
}

#[test]
fn key_value_field_lowers_to_map_reference() {
    let asschema = lower(
        "() () { NodeName [Text] NodeProposal [Text] Cluster [(nodes (@KeyValue (NodeName NodeProposal)))] }",
    );
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "nodes");
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
    let asschema = lower("() () { Cache [Text] Cluster [(cache (@Option (Cache)))] }");
    let fields = struct_fields(&asschema, "Cluster");
    assert_eq!(fields[0].name.as_str(), "cache");
    assert_eq!(
        fields[0].reference,
        TypeReference::Optional(Box::new(TypeReference::new("Cache")))
    );
}

#[test]
fn collection_field_and_plain_field_coexist_in_one_struct() {
    let asschema = lower(
        "() () { Trust [Text] Service [Text] Cluster [Trust (services (@Vec (Service))) (cache (@Option (Trust)))] }",
    );
    let fields = struct_fields(&asschema, "Cluster");
    // Bare symbol stays the legacy plain field (name derived from type).
    assert_eq!(fields[0].name.as_str(), "trust");
    assert_eq!(fields[0].reference, TypeReference::new("Trust"));
    // Explicit-pair collection fields carry their stated name + type.
    assert_eq!(fields[1].name.as_str(), "services");
    assert!(matches!(fields[1].reference, TypeReference::Vector(_)));
    assert_eq!(fields[2].name.as_str(), "cache");
    assert!(matches!(fields[2].reference, TypeReference::Optional(_)));
}

#[test]
fn nested_collections_lower_recursively() {
    // A map whose value is itself a vector of an optional leaf.
    let asschema = lower(
        "() () { Leaf [Text] Key [Text] Nest [(deep (@KeyValue (Key (@Vec ((@Option (Leaf)))))))] }",
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
        "() ((Projected (@KeyValue (NodeName NodeConfig)))) { NodeName [Text] NodeConfig [Text] }",
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
            "() () { Leaf [Text] Bad [(field (@HashSet (Leaf)))] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("unknown collection head should fail");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "HashSet".to_owned(),
            argument_count: 1,
        }
    );
}

#[test]
fn key_value_with_wrong_argument_count_is_rejected() {
    let error = SchemaEngine::default()
        .lower_source(
            "() () { Leaf [Text] Bad [(field (@KeyValue (Leaf)))] }",
            SchemaIdentity::new("collections:lib", "0.1.0"),
        )
        .expect_err("KeyValue needs two arguments");
    assert_eq!(
        error,
        schema_next::SchemaError::UnknownTypeReferenceForm {
            head: "KeyValue".to_owned(),
            argument_count: 1,
        }
    );
}
