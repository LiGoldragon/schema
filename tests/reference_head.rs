//! The canonical reference-grammar head-set lives in exactly one place,
//! `ReferenceHead`. Every reference-lowering site classifies its head through
//! `ReferenceHead::classify` instead of hand-copying the alias match, so the
//! sites cannot drift apart. These tests pin the classifier's recognised heads
//! and aliases, then confirm the alias spellings lower correctly through the
//! public lowering API.

use schema_next::{ReferenceHead, SchemaEngine, SchemaIdentity, TypeDeclaration, TypeReference};

fn lower(namespace: &str) -> schema_next::Schema {
    SchemaEngine::default()
        .lower_source(
            &format!("[] [] {{ {namespace} }}"),
            SchemaIdentity::new("reference-head:lib", "0.1.0"),
        )
        .expect("schema lowers")
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
fn classify_recognises_every_canonical_head_and_alias() {
    assert_eq!(ReferenceHead::classify("Vec"), Some(ReferenceHead::Vector));
    assert_eq!(
        ReferenceHead::classify("Vector"),
        Some(ReferenceHead::Vector)
    );
    assert_eq!(
        ReferenceHead::classify("Optional"),
        Some(ReferenceHead::Optional)
    );
    assert_eq!(
        ReferenceHead::classify("Option"),
        Some(ReferenceHead::Optional)
    );
    assert_eq!(
        ReferenceHead::classify("ScopeOf"),
        Some(ReferenceHead::ScopeOf)
    );
    assert_eq!(
        ReferenceHead::classify("Scope"),
        Some(ReferenceHead::ScopeOf)
    );
    assert_eq!(ReferenceHead::classify("Map"), Some(ReferenceHead::Map));
    assert_eq!(
        ReferenceHead::classify("KeyValue"),
        Some(ReferenceHead::Map)
    );
    assert_eq!(ReferenceHead::classify("Bytes"), Some(ReferenceHead::Bytes));
}

#[test]
fn classify_rejects_non_grammar_heads() {
    assert_eq!(ReferenceHead::classify("HashSet"), None);
    assert_eq!(ReferenceHead::classify("Plain"), None);
    assert_eq!(ReferenceHead::classify("FixedBytes"), None);
    assert_eq!(ReferenceHead::classify(""), None);
}

#[test]
fn vector_alias_lowers_through_the_lowering_path() {
    let schema = lower("Service String Cluster (Vector Service)");
    assert_eq!(
        single_reference(&schema, "Cluster"),
        &TypeReference::Vector(Box::new(TypeReference::new("Service")))
    );
}

#[test]
fn option_alias_lowers_through_the_lowering_path() {
    let schema = lower("Cache String Hold (Option Cache)");
    assert_eq!(
        single_reference(&schema, "Hold"),
        &TypeReference::Optional(Box::new(TypeReference::new("Cache")))
    );
}

#[test]
fn scope_alias_lowers_through_the_lowering_path() {
    let schema = lower("Leaf String Reach (Scope Leaf)");
    assert_eq!(
        single_reference(&schema, "Reach"),
        &TypeReference::ScopeOf(Box::new(TypeReference::new("Leaf")))
    );
}

#[test]
fn scope_of_canonical_lowers_through_the_lowering_path() {
    let schema = lower("Leaf String Reach (ScopeOf Leaf)");
    assert_eq!(
        single_reference(&schema, "Reach"),
        &TypeReference::ScopeOf(Box::new(TypeReference::new("Leaf")))
    );
}

#[test]
fn key_value_alias_lowers_through_the_lowering_path() {
    let schema = lower("Key String Value String Store (KeyValue (Key Value))");
    assert_eq!(
        single_reference(&schema, "Store"),
        &TypeReference::Map(
            Box::new(TypeReference::new("Key")),
            Box::new(TypeReference::new("Value")),
        )
    );
}
