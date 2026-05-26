include!("fixtures/generated_nota_reader/expected.rs");

use nota_codec::{Decoder, NotaDecode};
use schema::{AssembledNotaSchema, DeclarationBody, ModuleName, NotaReaderRustEmitter, Payload};

fn prototype_schema_text() -> &'static str {
    "
{
  Topic [String]
  Topics [(Vec Topic)]
  Description [String]
  Entry [Topics Kind Description Magnitude]
  Kind (Decision Principle Correction Clarification Constraint)
  Magnitude (Minimum VeryLow Low Medium High VeryHigh Maximum)
  Observation (Topics (Records Entry))
}
"
}

fn assembled() -> AssembledNotaSchema {
    AssembledNotaSchema::from_namespace_text(
        ModuleName::new("spirit_intent").expect("module name"),
        prototype_schema_text(),
    )
    .expect("schema lowers")
}

#[test]
fn delimiter_first_schema_lowers_to_ordered_assembled_nota_schema() {
    let assembled = assembled();
    let names = assembled
        .types()
        .iter()
        .map(|schema_type| schema_type.name().as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "Topic",
            "Topics",
            "Description",
            "Entry",
            "Kind",
            "Magnitude",
            "Observation"
        ]
    );

    let entry = assembled
        .types()
        .iter()
        .find(|schema_type| schema_type.name().as_str() == "Entry")
        .expect("Entry type");
    let DeclarationBody::Record(fields) = entry.body() else {
        panic!("Entry should lower as a positional struct");
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.effective_name().to_string())
            .collect::<Vec<_>>(),
        vec!["topics", "kind", "description", "magnitude"]
    );

    let observation = assembled
        .types()
        .iter()
        .find(|schema_type| schema_type.name().as_str() == "Observation")
        .expect("Observation type");
    let DeclarationBody::Enum { variants } = observation.body() else {
        panic!("Observation should lower as enum");
    };
    assert_eq!(variants[0].name().as_str(), "Topics");
    assert!(matches!(variants[0].payload(), Payload::Unit));
    assert_eq!(variants[1].name().as_str(), "Records");
    assert!(matches!(variants[1].payload(), Payload::Type(_)));
}

#[test]
fn rust_reader_emitter_matches_the_compiled_fixture() {
    let emitted = NotaReaderRustEmitter::emit_module(&assembled()).expect("emits reader module");

    assert_eq!(
        emitted,
        include_str!("fixtures/generated_nota_reader/expected.rs")
    );
    assert!(!emitted.contains("signal_channel"));
    assert!(!emitted.contains("legacy_signal_channel"));
    assert!(!emitted.contains("Feature"));
}

#[test]
fn reader_stack_runs_block_pass_before_object_lowering() {
    let result = AssembledNotaSchema::from_namespace_text(
        ModuleName::new("spirit_intent").expect("module name"),
        "{ Topic [String]\n",
    );
    let error = result.expect_err("unbalanced delimiter is caught by block pass");

    assert!(error.to_string().contains("schema object block pass"));
    assert!(error.to_string().contains("missing closing `}`"));
}

#[test]
fn reader_stack_uses_block_matcher_before_namespace_lowering() {
    let result = AssembledNotaSchema::from_namespace_text(
        ModuleName::new("spirit_intent").expect("module name"),
        "{ [Topic Name] [String] }",
    );
    let error = result.expect_err("non-symbol namespace key is rejected by block matcher");

    assert!(
        error
            .to_string()
            .contains("nota reader namespace block matcher")
    );
    assert!(error.to_string().contains("key must be a symbol candidate"));
}

#[test]
fn namespace_type_lowering_still_uses_object_value_after_block_match() {
    let assembled = AssembledNotaSchema::from_namespace_text(
        ModuleName::new("spirit_intent").expect("module name"),
        "{ Topic [String] Entry [Topic Kind] Kind (Decision Correction) }",
    )
    .expect("namespace lowers after block matching");

    let names = assembled
        .types()
        .iter()
        .map(|schema_type| schema_type.name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Topic", "Entry", "Kind"]);

    let kind = assembled
        .types()
        .iter()
        .find(|schema_type| schema_type.name().as_str() == "Kind")
        .expect("Kind type");
    let DeclarationBody::Enum { variants } = kind.body() else {
        panic!("Kind should remain object-value-lowered as enum");
    };
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name().as_str())
            .collect::<Vec<_>>(),
        vec!["Decision", "Correction"]
    );
}

#[test]
fn compiled_generated_reader_decodes_positional_record_values() {
    let mut decoder = Decoder::new("([schema nota] Decision [schema driven reader] High)");
    let entry = spirit_intent::Entry::decode(&mut decoder).expect("entry decodes");

    assert_eq!(
        entry,
        spirit_intent::Entry {
            topics: spirit_intent::Topics(vec![
                spirit_intent::Topic("schema".into()),
                spirit_intent::Topic("nota".into())
            ]),
            kind: spirit_intent::Kind::Decision,
            description: spirit_intent::Description("schema driven reader".into()),
            magnitude: spirit_intent::Magnitude::High,
        }
    );
}

#[test]
fn compiled_generated_reader_decodes_unit_and_data_carrying_enum_variants() {
    let mut unit_decoder = Decoder::new("Topics");
    let unit = spirit_intent::Observation::decode(&mut unit_decoder).expect("unit variant decodes");
    assert_eq!(unit, spirit_intent::Observation::Topics);

    let mut data_decoder = Decoder::new("(Records ([schema] Constraint [reader works] Maximum))");
    let data = spirit_intent::Observation::decode(&mut data_decoder).expect("data variant decodes");

    assert_eq!(
        data,
        spirit_intent::Observation::Records(spirit_intent::Entry {
            topics: spirit_intent::Topics(vec![spirit_intent::Topic("schema".into())]),
            kind: spirit_intent::Kind::Constraint,
            description: spirit_intent::Description("reader works".into()),
            magnitude: spirit_intent::Magnitude::Maximum,
        })
    );
}

#[test]
fn compiled_generated_reader_rejects_labeled_field_shape() {
    let mut decoder = Decoder::new("(Entry (topics [schema]) (kind Decision))");
    let error = spirit_intent::Entry::decode(&mut decoder).expect_err("labeled field shape errors");

    assert!(matches!(
        error,
        nota_codec::Error::LabeledFieldShape {
            type_name,
            expected_positional: 4,
            ..
        } if type_name == "Entry"
    ));
}
