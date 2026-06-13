//! Generic application references — `(Foo A B …)` at a reference position —
//! and parameterized DECLARATION heads — `(Name Param …)` at a
//! declaration's type-name position.
//!
//! `TypeReference::Application { head, arguments }` is the broad
//! generic-application form, captured by nota-next's
//! `#[shape(pascal_head, body)]` structural-macro seam. The first block of
//! tests pins the application-form behaviours from the earlier slice:
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
//!
//! The second block pins the parameterized-declaration-head behaviours
//! (the head analogue of the application form): binders resolve inside the
//! body instead of failing the closure walk, an `Application` of a declared
//! parameterized head is arity-checked at lowering, and the declared head
//! is consulted before the broad application form.

use std::path::PathBuf;

use nota_next::{Document, NotaDecode, NotaEncode};
use schema_next::{
    ApplicationHead, ImportResolver, MacroContext, Name, SchemaEngine, SchemaError, SchemaIdentity,
    SchemaSourceArtifact, TypeDeclaration, TypeReference,
};

fn lower(namespace: &str) -> schema_next::Schema {
    try_lower(namespace).expect("schema lowers")
}

fn try_lower(namespace: &str) -> Result<schema_next::Schema, SchemaError> {
    SchemaEngine::default().lower_source(
        &format!("[] [] {{ {namespace} }}"),
        SchemaIdentity::new("generics:lib", "0.1.0"),
    )
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

// ----------------------------------------------------------------------
// Parameterized DECLARATION heads `(Name Param …)` — the head analogue of
// the application form. A declaration's type-name position becomes a
// parenthesized `(Name Param Param …)` head that introduces type-parameter
// binders; the binders resolve inside the body, and an `Application` of a
// declared parameterized head is arity-checked at lowering (decision O8).
// ----------------------------------------------------------------------

fn declaration_parameters<'schema>(
    schema: &'schema schema_next::Schema,
    name: &str,
) -> &'schema [Name] {
    schema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == name)
        .expect("declaration present")
        .parameters()
}

// (a) A parameterized declaration whose body uses its parameters as Plain
//     references lowers, and its family closure resolves the binders
//     instead of failing with FamilyReferenceNotFound.

#[test]
fn parameterized_declaration_resolves_its_parameters_as_binders() {
    let schema = lower("(Plane Input Output) { source Input target Output }");

    // The binders are recorded on the declaration, in order.
    assert_eq!(
        declaration_parameters(&schema, "Plane"),
        &[Name::new("Input"), Name::new("Output")],
    );

    // The closure walk reaches `Input` and `Output` as Plain field
    // references. Without binder scope this is a FamilyReferenceNotFound;
    // with it, the walk succeeds and pulls in no extra declarations — a
    // binder is a type-parameter, not a declared type.
    let closure = schema
        .family_closure("Plane")
        .expect("parameterized declaration closes over its binders, not undeclared names");
    let names = closure
        .declarations()
        .iter()
        .map(|declaration| declaration.name().as_str().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, ["Plane"]);
}

// (b) An Application supplying the WRONG argument count to a resolved
//     parameterized head is a typed arity error AT LOWERING — not a panic,
//     not a deferred emitter failure.

#[test]
fn application_with_wrong_argument_count_is_an_arity_error_at_lowering() {
    let error =
        try_lower("(Plane Input Output) { source Input target Output } Holder (Plane String)")
            .expect_err("one argument against a two-parameter head must fail at lowering");
    assert_eq!(
        error,
        SchemaError::GenericArityMismatch {
            head: "Plane".to_owned(),
            expected: 2,
            found: 1,
        },
    );
}

// (c) The correct argument count matching the declared arity lowers and
//     the application reference is present.

#[test]
fn application_with_correct_argument_count_lowers() {
    let schema =
        lower("(Plane Input Output) { source Input target Output } Holder (Plane String Integer)");
    assert_eq!(
        single_reference(&schema, "Holder"),
        &TypeReference::Application {
            head: ApplicationHead::Local(Name::new("Plane")),
            arguments: vec![TypeReference::String, TypeReference::Integer],
        },
    );
}

// (d) A declared parameterized head is consulted BEFORE the broad
//     Application form: applying `(Plane …)` resolves to the declared
//     `Plane` (so its arity binds), whereas an undeclared head fixes no
//     arity and any count is accepted as an unresolved generic application.

#[test]
fn declared_parameterized_head_wins_over_unresolved_application() {
    // The declared head's arity binds — a wrong count is rejected.
    assert_eq!(
        try_lower("(Plane Input Output) { source Input target Output } Holder (Plane String)")
            .expect_err("declared head is consulted, so its arity binds"),
        SchemaError::GenericArityMismatch {
            head: "Plane".to_owned(),
            expected: 2,
            found: 1,
        },
    );

    // An UNDECLARED head fixes no arity, so the same single-argument
    // application is an ordinary unresolved generic application — proving
    // the declared head, not the broad form, governed the case above.
    let schema = lower("Holder (Foo String)");
    assert_eq!(
        single_reference(&schema, "Holder"),
        &TypeReference::Application {
            head: ApplicationHead::Local(Name::new("Foo")),
            arguments: vec![TypeReference::String],
        },
    );
}

// The parameterized head survives the source-codec archive: the entry key
// projects back to `(Plane Input Output)` text and re-decodes to the same
// source object, and lowering through the source endpoint records the same
// binders as the macro-engine path (edit site 2).

#[test]
fn parameterized_head_round_trips_through_the_source_codec() {
    let source = "{}\n[]\n[]\n{\n  (Plane Input Output) { source Input target Output }\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("source decodes");
    let canonical = artifact.to_schema_text();
    assert!(
        canonical.contains("(Plane Input Output) { source Input target Output }"),
        "the parameterized head must project back to source text, got {canonical}",
    );
    let recovered =
        SchemaSourceArtifact::from_schema_text(&canonical).expect("canonical source decodes");
    assert_eq!(artifact, recovered, "the source archive round-trips");

    let schema = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("generics:lib", "0.1.0"),
        )
        .expect("source endpoint lowers the parameterized declaration");
    assert_eq!(
        declaration_parameters(&schema, "Plane"),
        &[Name::new("Input"), Name::new("Output")],
    );
}

// Arity validation is shared by both lowering paths: the source-codec
// endpoint rejects a wrong-arity application at lowering, exactly as the
// macro-engine path does.

#[test]
fn source_codec_path_also_validates_application_arity() {
    let source = "{}\n[]\n[]\n{\n  (Plane Input Output) { source Input target Output }\n  Holder (Plane String)\n}";
    let artifact = SchemaSourceArtifact::from_schema_text(source).expect("source decodes");
    let error = artifact
        .source()
        .lower(
            &SchemaEngine::default(),
            SchemaIdentity::new("generics:lib", "0.1.0"),
        )
        .expect_err("source-codec lowering must arity-check the application");
    assert_eq!(
        error,
        SchemaError::GenericArityMismatch {
            head: "Plane".to_owned(),
            expected: 2,
            found: 1,
        },
    );
}
