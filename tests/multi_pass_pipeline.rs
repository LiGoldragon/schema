//! End-to-end test for the multi-pass macro pipeline.
//!
//! Loads the LIVE Spirit `.schema` file, runs it through
//! `schema::multi_pass::read_schema_with_report`, and asserts:
//!
//! - Each builtin macro family fires the expected number of times
//!   (import / header / type / feature).
//! - The resulting `AssembledSchema` carries the expected number of
//!   imports / routes / types / features.
//! - The pipeline's `AssembledSchema` is byte-equivalent (via the
//!   canonical `Schema::parse_str(...).assemble(&[])` path) — the
//!   strongest claim we can make that the multi-pass dispatch ARRIVES
//!   at the same canonical lowered form the existing parser produces.
//!
//! Cross-references:
//! - `reports/second-designer/183-fully-schema-and-nota-mvp-2026-05-25.md`
//! - `reports/designer/334-v2-multi-pass-nota-first-schema-reader.md` §6
//!   (byte-equivalence is the load-bearing proof)

use schema::multi_pass::{PipelineReport, read_schema_six_position, read_schema_with_report};
use schema::{AssembledType, Feature, ImportResolution, Name, Schema};

const LIVE_SPIRIT_TEXT: &str = include_str!("fixtures/schema-e2e/live-spirit.schema");

#[test]
fn pipeline_lowers_live_spirit_schema_byte_equivalent_to_canonical_reader() {
    let multi_pass = read_schema_six_position(LIVE_SPIRIT_TEXT).expect("multi-pass reads ok");
    let canonical_schema = Schema::parse_str(LIVE_SPIRIT_TEXT).expect("canonical parser reads ok");

    // Give the canonical assembler the same single-name ImportAll
    // resolution that the multi-pass pipeline applies internally
    // (binding-name = imported-name when no sibling-schema resolver
    // is available — per `MacroPipeline::lower_imports` in
    // `src/multi_pass.rs`).
    let resolutions = vec![
        ImportResolution::new(
            Name::new("Magnitude").unwrap(),
            vec![Name::new("Magnitude").unwrap()],
        )
        .unwrap(),
    ];
    let canonical = canonical_schema
        .assemble(&resolutions)
        .expect("canonical assemble ok");

    assert_eq!(
        format!("{multi_pass:#?}"),
        format!("{canonical:#?}"),
        "multi-pass pipeline diverges from canonical Schema::parse_str + assemble"
    );
}

#[test]
fn pipeline_report_counts_match_live_spirit_schema_shape() {
    let report = read_schema_with_report(LIVE_SPIRIT_TEXT).expect("multi-pass reads ok");
    let PipelineReport {
        macro_index,
        import_firings,
        header_firings,
        type_firings,
        feature_firings,
        assembled,
    } = report;
    let assembled = assembled.expect("assembled present");

    // The live Spirit schema declares:
    //   {Magnitude (ImportAll ...)  SemaSet (Import ... [3 names])}
    //   [(State ...) (Record ...) (Observe ...) (Watch ...) (Unwatch ...)]
    //   []
    //   []
    //   { 34 local namespace entries }
    //   [(Reply ...) (Event ...) (Observable ...)]
    //
    // Imports fire once per binding (`Magnitude`, `SemaSet`).
    assert_eq!(macro_index.import_candidates, 2);
    assert_eq!(import_firings, 2, "expected two import macro firings");

    // Headers fire once per root: 5 ordinary roots, 0 owner, 0 sema.
    assert_eq!(macro_index.header_candidates, 5);
    assert_eq!(header_firings, 5, "expected five header macro firings");

    // Type firings = local namespace count + imported names.
    //   Local namespace entries (live-spirit.schema has):
    //     State Record Observe Watch Unwatch
    //     Kind ObservationMode Presence UnimplementedReason
    //     Topic Summary Context Quote StatementText FocusArea
    //     RecordIdentifier QuestionIdentifier QuestionText
    //     StateSubscriptionToken RecordSubscriptionToken
    //     Entry Statement RecordQuery RecordObservation
    //     RecordSubscription RecordSummary RecordProvenance
    //     TopicCount QuestionSummary
    //     Observation Subscription SubscriptionToken
    //     RecordAccepted StateObserved RecordsObserved
    //     RecordProvenancesObserved TopicsObserved QuestionsObserved
    //     SubscriptionOpened SubscriptionRetracted
    //     RequestUnimplemented SubscriptionSnapshot
    //     StateChanged RecordCaptured
    //     OperationKind OperationReceived EffectEmitted
    //   Imported names: SemaOperation SemaOutcome SemaObservation +
    //     ImportAll binding `Magnitude` registered as one imported
    //     name (per the canonical-reader behavior when assembled
    //     without explicit resolutions).
    //
    // The exact local count is read from the assembled output —
    // this insulates the test from minor schema edits.
    let local_types = assembled
        .types()
        .filter(|t| matches!(t, AssembledType::Local { .. }))
        .count();
    let imported_types = assembled
        .types()
        .filter(|t| matches!(t, AssembledType::Imported { .. }))
        .count();
    let expected_type_firings = local_types + imported_types;
    assert_eq!(macro_index.type_candidates, local_types);
    assert_eq!(
        type_firings, expected_type_firings,
        "type firings ({type_firings}) should match locals ({local_types}) + imports ({imported_types})"
    );

    // Features: live Spirit has Reply + Event + Observable.
    assert_eq!(macro_index.feature_candidates, 3);
    assert_eq!(feature_firings, 3, "expected three feature macro firings");

    // Sanity on the assembled output's overall shape.
    assert_eq!(assembled.imports().len(), 2);
    assert_eq!(assembled.routes().len(), 5);
    assert_eq!(assembled.features().len(), 3);
    assert!(
        local_types >= 30,
        "expected at least 30 local types in Spirit namespace, got {local_types}"
    );

    // The three feature variants must each be present.
    let mut has_reply = false;
    let mut has_event = false;
    let mut has_observable = false;
    for feature in assembled.features() {
        match feature {
            Feature::Reply(_) => has_reply = true,
            Feature::Event(_) => has_event = true,
            Feature::Observable(_) => has_observable = true,
            Feature::Upgrade(_) => {}
            Feature::EffectTable(_) | Feature::FanOutTargets(_) | Feature::StorageDescriptor(_) => {
            }
        }
    }
    assert!(has_reply, "Reply feature missing");
    assert!(has_event, "Event feature missing");
    assert!(has_observable, "Observable feature missing");
}

#[test]
fn pipeline_rejects_unknown_import_directive() {
    // Shape-logic dispatch in `ImportMacroRecognizer::recognize` must
    // reject `(Path ...)` (the retired form) with a clear error.
    let text = "
{
  Foo (Path ./missing.schema)
}
[]
[]
[]
{}
[]
";
    let error = read_schema_six_position(text).expect_err("retired import form must error");
    let message = format!("{error}");
    assert!(
        message.contains("unknown import directive"),
        "expected `unknown import directive` error, got: {message}"
    );
}

#[test]
fn pipeline_rejects_non_six_position_documents() {
    // Four top-level values instead of six.
    let text = "{} [] [] []";
    let error = read_schema_six_position(text).expect_err("4-position file must error");
    let message = format!("{error}");
    assert!(
        message.contains("six top-level values"),
        "expected `six top-level values` error, got: {message}"
    );
}
