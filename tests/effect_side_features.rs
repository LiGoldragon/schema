//! Coverage for the three new feature variants
//! (`EffectTable`, `FanOutTargets`, `StorageDescriptor`) plus the
//! universal-Unknown injection pass per /346 §9.

use schema::{
    DeclarationBody, FanOutOutputDeclaration, Feature, LoadedSchema, Name, Payload, Primitive,
    Schema, TypeExpression, UniversalUnknownMacro,
};
use std::path::Path;

fn fixture(path: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("effect-side")
        .join(path)
}

#[test]
fn shape_parser_recognises_effect_table_and_fan_out_features() {
    let text = std::fs::read_to_string(fixture("recorder.schema")).expect("fixture exists");
    let schema = Schema::parse_str(&text).expect("schema parses through shape_parser");
    let features = schema.features();
    assert!(matches!(
        features.iter().find(|f| matches!(f, Feature::EffectTable(_))),
        Some(_)
    ));
    assert!(matches!(
        features.iter().find(|f| matches!(f, Feature::FanOutTargets(_))),
        Some(_)
    ));
}

#[test]
fn streaming_parser_recognises_effect_table_and_fan_out_features() {
    let text = std::fs::read_to_string(fixture("recorder.schema")).expect("fixture exists");
    let schema = Schema::parse_str_with_streaming_decoder(&text)
        .expect("schema parses through streaming decoder");
    let features = schema.features();
    assert!(matches!(
        features.iter().find(|f| matches!(f, Feature::EffectTable(_))),
        Some(_)
    ));
    assert!(matches!(
        features.iter().find(|f| matches!(f, Feature::FanOutTargets(_))),
        Some(_)
    ));
}

#[test]
fn shape_parser_recognises_storage_descriptor() {
    let text = std::fs::read_to_string(fixture("storage.schema")).expect("fixture exists");
    let schema = Schema::parse_str(&text).expect("schema parses");
    let features = schema.features();
    let storage = features
        .iter()
        .find_map(|feature| match feature {
            Feature::StorageDescriptor(descriptor) => Some(descriptor),
            _ => None,
        })
        .expect("StorageDescriptor feature present");
    let entries = storage.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].logical_name().as_str(), "Records");
    assert_eq!(entries[0].table_type().as_str(), "RecordsTable");
}

#[test]
fn effect_table_parses_action_to_effect_pairs() {
    let text = std::fs::read_to_string(fixture("recorder.schema")).expect("fixture exists");
    let schema = Schema::parse_str(&text).expect("schema parses");
    let effect_table = schema
        .features()
        .iter()
        .find_map(|feature| match feature {
            Feature::EffectTable(table) => Some(table),
            _ => None,
        })
        .expect("EffectTable feature present");
    let entries = effect_table.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].action().as_str(), "RecordEntry");
    assert_eq!(entries[0].effect().as_str(), "RecordWriteEffect");
}

#[test]
fn fan_out_targets_parses_three_output_kinds() {
    let text = std::fs::read_to_string(fixture("recorder.schema")).expect("fixture exists");
    let schema = Schema::parse_str(&text).expect("schema parses");
    let fan_out = schema
        .features()
        .iter()
        .find_map(|feature| match feature {
            Feature::FanOutTargets(targets) => Some(targets),
            _ => None,
        })
        .expect("FanOutTargets feature present");
    let row = fan_out
        .entries()
        .iter()
        .find(|entry| entry.effect().as_str() == "RecordWriteEffect")
        .expect("RecordWriteEffect row present");
    let outputs = row.outputs();
    assert_eq!(outputs.len(), 2);
    // First output: actor-call form (Store SpiritStorage InsertStampedEntry)
    match &outputs[0] {
        FanOutOutputDeclaration::Actor {
            method_tag,
            actor_type,
            actor_method,
        } => {
            assert_eq!(method_tag.as_str(), "Store");
            assert_eq!(actor_type.as_str(), "SpiritStorage");
            assert_eq!(actor_method.as_str(), "InsertStampedEntry");
        }
        other => panic!("expected Actor output, got {other:?}"),
    }
    // Second output: wire reply form (Reply RecordAccepted)
    match &outputs[1] {
        FanOutOutputDeclaration::Reply { variant } => {
            assert_eq!(variant.as_str(), "RecordAccepted");
        }
        other => panic!("expected Reply output, got {other:?}"),
    }
}

#[test]
fn universal_unknown_predicate_recognises_response_suffix() {
    let response = Name::new("RecorderResponse").unwrap();
    let other = Name::new("Operation").unwrap();
    assert!(UniversalUnknownMacro::is_response_enum_name(&response));
    assert!(!UniversalUnknownMacro::is_response_enum_name(&other));
}

#[test]
fn universal_unknown_injection_adds_string_carrier_variant_idempotently() {
    let name = Name::new("Sample").unwrap();
    let mut body = DeclarationBody::Enum {
        variants: vec![
            schema::Variant::unit(Name::new("Alpha").unwrap()),
            schema::Variant::unit(Name::new("Beta").unwrap()),
        ],
    };
    UniversalUnknownMacro::inject_unknown_into_enum_body(&mut body);
    UniversalUnknownMacro::inject_unknown_into_enum_body(&mut body); // idempotent

    let DeclarationBody::Enum { variants } = &body else {
        panic!("expected enum body");
    };
    assert_eq!(variants.len(), 3);
    let unknown = &variants[2];
    assert_eq!(unknown.name().as_str(), "Unknown");
    assert!(matches!(
        unknown.payload(),
        Payload::Type(TypeExpression::Primitive(Primitive::String))
    ));
    let _ = name;
}

#[test]
fn assembled_schema_carries_injected_unknown_on_response_enum() {
    let path = fixture("recorder.schema");
    let loaded = LoadedSchema::read_path(&path).expect("schema reads");
    let assembled = loaded.assembled();

    let response_body = assembled
        .body(&Name::new("RecorderResponse").unwrap())
        .expect("RecorderResponse local body present");
    let DeclarationBody::Enum { variants } = response_body else {
        panic!("expected RecorderResponse to be an enum body");
    };

    // Original schema declares RecordAccepted + StatusReturned;
    // finalize_universal_unknowns must inject Unknown(String) as the
    // third variant.
    let names: Vec<&str> = variants.iter().map(|v| v.name().as_str()).collect();
    assert!(names.contains(&"RecordAccepted"));
    assert!(names.contains(&"StatusReturned"));
    assert!(
        names.contains(&"Unknown"),
        "RecorderResponse missing universal Unknown variant: {names:?}"
    );

    let unknown = variants
        .iter()
        .find(|v| v.name().as_str() == "Unknown")
        .unwrap();
    assert!(
        matches!(
            unknown.payload(),
            Payload::Type(TypeExpression::Primitive(Primitive::String))
        ),
        "Unknown variant must carry a String payload"
    );
}

#[test]
fn assembled_schema_does_not_inject_unknown_on_non_response_enums() {
    let path = fixture("recorder.schema");
    let loaded = LoadedSchema::read_path(&path).expect("schema reads");
    let assembled = loaded.assembled();

    let action = assembled
        .body(&Name::new("RecorderAction").unwrap())
        .expect("RecorderAction body present");
    let DeclarationBody::Enum { variants } = action else {
        panic!("expected RecorderAction to be enum");
    };
    let names: Vec<&str> = variants.iter().map(|v| v.name().as_str()).collect();
    assert!(
        !names.contains(&"Unknown"),
        "Unknown leaked into ACTION enum: {names:?}"
    );
}

#[test]
fn loaded_schema_carries_storage_descriptor_feature() {
    let path = fixture("storage.schema");
    let loaded = LoadedSchema::read_path(&path).expect("schema reads");
    let assembled = loaded.assembled();
    let descriptor = assembled
        .features()
        .iter()
        .find_map(|feature| match feature {
            Feature::StorageDescriptor(descriptor) => Some(descriptor),
            _ => None,
        })
        .expect("StorageDescriptor present in assembled schema");
    assert_eq!(descriptor.entries().len(), 1);
}
