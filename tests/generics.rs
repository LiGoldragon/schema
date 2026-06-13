//! Generic application references — `(Foo A B …)` at a reference position.
//!
//! `TypeReference::Application { head, arguments }` is the broad
//! generic-application form, captured by nota-next's
//! `#[shape(pascal_head, body)]` structural-macro seam. These tests pin the
//! four behaviours the design requires:
//!
//! (a) a multi-arg user generic application lowers to `Application` and
//!     round-trips byte-stable through both the rkyv codec and the canonical
//!     NOTA codec;
//! (b) the built-in heads `(Vector X)`, `(Optional X)`, `(Map (K V))` still
//!     lower to their dedicated variants through the same dispatch, and a
//!     built-in head wins over the broad application form (dispatch ORDER);
//! (c) a dropped alias `(Vec X)` no longer lowers to the collection — it is
//!     an ordinary application head now;
//! (d) the closure walk over an imported generic head records that head's
//!     import.

use std::path::PathBuf;

use nota_next::{Document, NotaDecode, NotaEncode};
use schema_next::{
    ApplicationHead, ImportResolver, MacroContext, Name, SchemaEngine, SchemaIdentity,
    TypeDeclaration, TypeReference,
};

fn lower(namespace: &str) -> schema_next::Schema {
    SchemaEngine::default()
        .lower_source(
            &format!("[] [] {{ {namespace} }}"),
            SchemaIdentity::new("generics:lib", "0.1.0"),
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

// (a) multi-arg user generic application lowers to Application and round-trips.

#[test]
fn multi_argument_application_lowers_to_application() {
    let schema = lower("Alpha String Beta String Holder (Foo Alpha Beta)");
    assert_eq!(
        single_reference(&schema, "Holder"),
        &TypeReference::Application {
            head: ApplicationHead::Local(Name::new("Foo")),
            arguments: vec![TypeReference::new("Alpha"), TypeReference::new("Beta")],
        }
    );
}

#[test]
fn application_round_trips_byte_stable_through_rkyv() {
    let reference = TypeReference::Application {
        head: ApplicationHead::Local(Name::new("Foo")),
        arguments: vec![TypeReference::new("Alpha"), TypeReference::new("Beta")],
    };
    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&reference).expect("application archives as rkyv");
    let restored = rkyv::from_bytes::<TypeReference, rkyv::rancor::Error>(&bytes)
        .expect("application decodes from rkyv");
    assert_eq!(restored, reference);
    // Archiving the restored value yields identical bytes — byte-stable.
    let again =
        rkyv::to_bytes::<rkyv::rancor::Error>(&restored).expect("re-archive the restored value");
    assert_eq!(bytes.as_slice(), again.as_slice());
}

#[test]
fn application_round_trips_through_canonical_nota_codec() {
    let reference = TypeReference::Application {
        head: ApplicationHead::Local(Name::new("Foo")),
        arguments: vec![TypeReference::new("Alpha"), TypeReference::new("Beta")],
    };
    let text = reference.to_nota();
    let document = Document::parse(&text).expect("application NOTA parses");
    let decoded = TypeReference::from_nota_block(&document.root_objects()[0])
        .expect("application decodes from canonical NOTA");
    assert_eq!(decoded, reference);
    // The re-encode is byte-identical to the first projection.
    assert_eq!(decoded.to_nota(), text);
}

// (b) built-ins still lower through the seam, and a built-in head wins over
// the broad application form (dispatch ORDER).

#[test]
fn builtin_heads_still_lower_to_their_variants() {
    let schema = lower(
        "Key String Value String VectorField (Vector Value) OptionalField (Optional Value) MapField (Map (Key Value))",
    );
    assert_eq!(
        single_reference(&schema, "VectorField"),
        &TypeReference::Vector(Box::new(TypeReference::new("Value")))
    );
    assert_eq!(
        single_reference(&schema, "OptionalField"),
        &TypeReference::Optional(Box::new(TypeReference::new("Value")))
    );
    assert_eq!(
        single_reference(&schema, "MapField"),
        &TypeReference::Map(
            Box::new(TypeReference::new("Key")),
            Box::new(TypeReference::new("Value")),
        )
    );
}

#[test]
fn builtin_head_wins_over_broad_application_form() {
    // `(Vector Value)` matches the broad `(Foo A …)` shape too (Vector is a
    // PascalCase head), but the built-in fast path is dispatched first, so it
    // must NOT lower to an application named `Vector`.
    let schema = lower("Value String Field (Vector Value)");
    let reference = single_reference(&schema, "Field");
    assert!(
        matches!(reference, TypeReference::Vector(_)),
        "a built-in head must win over the application form, got {reference:?}",
    );
    assert!(
        !matches!(reference, TypeReference::Application { .. }),
        "the built-in head must not fall through to the application form",
    );
}

// (c) a dropped alias no longer lowers to the collection.

#[test]
fn dropped_vec_alias_no_longer_lowers_to_vector() {
    let schema = lower("Service String Cluster (Vec Service)");
    let reference = single_reference(&schema, "Cluster");
    assert!(
        !matches!(reference, TypeReference::Vector(_)),
        "the dropped `Vec` alias must not lower to a Vector",
    );
    assert_eq!(
        reference,
        &TypeReference::Application {
            head: ApplicationHead::Local(Name::new("Vec")),
            arguments: vec![TypeReference::new("Service")],
        }
    );
}

// (d) the closure walk over an imported generic head records the import.

#[test]
fn closure_over_imported_generic_head_records_the_import() {
    let resolver = ImportResolver::new().with_dependency(
        "marker-core",
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/marker-core/schema"),
        "0.1.0",
    );
    let consumer_source = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/import-generic-consumer/schema/lib.schema"),
    )
    .expect("read generic-head consumer schema");
    let schema = SchemaEngine::default()
        .lower_source_with_resolver(
            &consumer_source,
            SchemaIdentity::new("import-generic-consumer", "0.1.0"),
            &mut MacroContext::default(),
            &resolver,
        )
        .expect("consumer schema lowers");

    // The `Output` root reaches `(Marked (DatabaseMarker Topic))` — the
    // imported `DatabaseMarker` is the application head, so its import must be
    // pulled into the closure.
    let closure = schema.family_closure("Output").expect("output closure");
    let imports = closure
        .imports()
        .iter()
        .map(|import| import.local_name.as_str().to_owned())
        .collect::<Vec<_>>();
    assert!(
        imports.contains(&"DatabaseMarker".to_owned()),
        "the imported generic head is recorded in the closure, got {imports:?}",
    );
}
