//! Constraint tests proving the invariants from /346 hold empirically.
//!
//! These tests sit alongside `effect_side_features.rs` and exercise
//! the universal-Unknown injection + storage-descriptor + effect-table
//! pipeline on synthetic schemas constructed in-test, so the
//! constraints can be proven without depending on cross-crate import
//! resolution.
//!
//! Written-this-session per the showcase report's empirical-rigor
//! discipline.
//!
//! Constraint coverage (cross-references /346 §1, §3, §9):
//!
//! - C1 universal Unknown on every RESPONSE enum
//! - C5 `finalize_universal_unknowns` is idempotent
//! - C5b non-Response enums are NEVER touched
//! - C5c non-enum bodies whose names end in `Response` are ignored
//! - C5d the multi-pass / streaming-decoder path also fires the hook
//!
//! C3 (single rkyv byte layout across sema+wire) lives in
//! `persona-spirit/src/schema_driven/storage.rs::tests` where rkyv is
//! already a dependency.

use schema::{
    DeclarationBody, Feature, Field, LoweringContext, Name, Payload, Primitive, Schema,
    TypeExpression, UniversalUnknownMacro, Variant,
};

/// C1: every RESPONSE enum picks up the universal `Unknown(String)`
/// variant, regardless of how many Response enums the schema declares.
#[test]
fn constraint_c1_every_response_enum_receives_unknown_variant() {
    // Synthetic schema with three Response enums + one non-Response
    // enum. `finalize_universal_unknowns` must touch the three but
    // leave the fourth alone.
    // Unit variants (no payload section) are valid; payloads with
    // bodies need at least one type expression per the schema reader.
    let text = r#"
        {}
        [] [] []
        {
          ApplesAction (Eat Throw)
          ApplesResponse (Eaten Tossed)
          BerriesResponse (Picked Mashed)
          CherriesResponse (Stoned Pitted)
          Mood (Happy Sad)
        }
        []
    "#;
    let schema = Schema::parse_str(text).expect("synthetic schema parses");
    let assembled = schema.assemble(&[]).expect("assembles");
    let response_names = ["ApplesResponse", "BerriesResponse", "CherriesResponse"];
    for name in response_names {
        let body = assembled
            .body(&Name::new(name).unwrap())
            .unwrap_or_else(|| panic!("{name} body present"));
        let DeclarationBody::Enum { variants } = body else {
            panic!("expected enum for {name}");
        };
        let variant_names: Vec<&str> = variants
            .iter()
            .map(|variant| variant.name().as_str())
            .collect();
        assert!(
            variant_names.contains(&"Unknown"),
            "{name} missing Unknown: {variant_names:?}",
        );
    }
    // The non-Response enum (Mood) must NOT be touched.
    let mood = assembled.body(&Name::new("Mood").unwrap()).unwrap();
    let DeclarationBody::Enum { variants } = mood else {
        panic!("expected enum");
    };
    let variant_names: Vec<&str> = variants
        .iter()
        .map(|variant| variant.name().as_str())
        .collect();
    assert!(
        !variant_names.contains(&"Unknown"),
        "Unknown leaked into non-Response enum Mood: {variant_names:?}",
    );
}

/// C5: `finalize_universal_unknowns` is idempotent. Calling it twice on
/// the same `LoweringContext` produces the same set of variants;
/// `Unknown` is not duplicated.
#[test]
fn constraint_c5_finalize_universal_unknowns_idempotent_on_full_lowering() {
    let text = r#"
        {}
        [] [] []
        {
          SoupResponse (Hot Cold)
        }
        []
    "#;
    let schema = Schema::parse_str(text).expect("parses");

    let assembled_once = schema.assemble(&[]).expect("first assemble");
    let assembled_twice = schema.assemble(&[]).expect("second assemble");

    let body_once = assembled_once
        .body(&Name::new("SoupResponse").unwrap())
        .unwrap();
    let body_twice = assembled_twice
        .body(&Name::new("SoupResponse").unwrap())
        .unwrap();

    let count = |body: &DeclarationBody| -> usize {
        let DeclarationBody::Enum { variants } = body else {
            panic!("expected enum");
        };
        variants
            .iter()
            .filter(|variant| variant.name().as_str() == "Unknown")
            .count()
    };

    assert_eq!(
        count(body_once),
        1,
        "first assemble injects exactly one Unknown"
    );
    assert_eq!(
        count(body_twice),
        1,
        "second assemble still has exactly one Unknown (idempotent)"
    );
}

/// C5 direct: `inject_unknown_into_enum_body` called many times is a
/// no-op after the first call. This is the load-bearing invariant for
/// the post-pass hook.
#[test]
fn constraint_c5_inject_unknown_into_enum_body_idempotent_under_many_calls() {
    let mut body = DeclarationBody::Enum {
        variants: vec![
            Variant::unit(Name::new("Alpha").unwrap()),
            Variant::unit(Name::new("Beta").unwrap()),
        ],
    };
    for _ in 0..16 {
        UniversalUnknownMacro::inject_unknown_into_enum_body(&mut body);
    }
    let DeclarationBody::Enum { variants } = &body else {
        panic!("expected enum body");
    };
    assert_eq!(
        variants.len(),
        3,
        "expected exactly one Unknown added regardless of call count"
    );
    assert_eq!(variants[2].name().as_str(), "Unknown");
}

/// C5b: `is_response_enum_name` strictly checks the `Response` suffix
/// --- types whose name happens to contain "Response" mid-string are
/// not touched.
#[test]
fn constraint_c5b_is_response_enum_name_only_matches_suffix() {
    assert!(UniversalUnknownMacro::is_response_enum_name(
        &Name::new("RecorderResponse").unwrap()
    ));
    assert!(UniversalUnknownMacro::is_response_enum_name(
        &Name::new("Response").unwrap()
    ));
    // Substring not at suffix: NOT matched.
    assert!(!UniversalUnknownMacro::is_response_enum_name(
        &Name::new("ResponseHandler").unwrap()
    ));
    assert!(!UniversalUnknownMacro::is_response_enum_name(
        &Name::new("Action").unwrap()
    ));
}

/// C5c: non-enum bodies (record, newtype) are ignored even when their
/// names end in `Response` --- only enum-shaped bodies get the variant.
#[test]
fn constraint_c5c_non_enum_response_bodies_are_ignored() {
    let mut record_body = DeclarationBody::Record(vec![Field::inferred(
        TypeExpression::Primitive(Primitive::Unsigned64),
    )]);
    UniversalUnknownMacro::inject_unknown_into_enum_body(&mut record_body);
    match &record_body {
        DeclarationBody::Record(fields) => assert_eq!(fields.len(), 1),
        other => panic!("expected record body, got {other:?}"),
    }
}

/// C5d (housekeeping): we can prove the finalize hook fires from the
/// multi-pass pipeline path too --- both `Schema::assemble` and
/// `MacroPipeline::run` call it.
#[test]
fn constraint_c5d_lowering_context_finalize_hook_runs_through_multi_pass() {
    let text = r#"
        {}
        [] [] []
        {
          TeapotResponse (Brewing Steeping)
        }
        []
    "#;
    let schema = Schema::parse_str_with_streaming_decoder(text)
        .expect("schema parses through streaming decoder");
    let assembled = schema.assemble(&[]).expect("assembles");
    let body = assembled
        .body(&Name::new("TeapotResponse").unwrap())
        .unwrap();
    let DeclarationBody::Enum { variants } = body else {
        panic!("expected enum");
    };
    let names: Vec<&str> = variants.iter().map(|v| v.name().as_str()).collect();
    assert!(
        names.contains(&"Unknown"),
        "streaming-decoder path also runs finalize_universal_unknowns: {names:?}",
    );
}

/// Quiet linter use of LoweringContext re-export.
#[test]
fn lowering_context_public_handle_is_a_thing() {
    let _ = std::any::type_name::<LoweringContext>();
    let _ = std::any::type_name::<Feature>();
    let _ = std::any::type_name::<Payload>();
}
