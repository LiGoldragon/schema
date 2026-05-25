use schema::{Schema, multi_pass};

const IMPORT_FREE_SCHEMA: &str = r#"
{}
[(State [Statement Declaration]) (Record [Entry])]
[]
[]
{
  State [(Statement) (Declaration)]
  Record [(Entry)]

  Topic (String)
  Kind [Decision Principle]
  Statement (Topic)
  Declaration (Topic Kind)
  Entry (Topic Kind)
  RecordAccepted (u64)
}
[(Reply RecordAccepted)]
"#;

#[test]
fn shape_parser_is_equivalent_to_streaming_decoder_for_spirit_fixture() {
    let text = include_str!("fixtures/schema-e2e/spirit-v0-1-1.schema");

    let shape_parser_schema = Schema::parse_str(text).expect("shape parser accepts fixture");
    let streaming_schema =
        Schema::parse_str_with_streaming_decoder(text).expect("streaming parser accepts fixture");

    assert_eq!(shape_parser_schema, streaming_schema);
}

#[test]
fn multi_pass_pipeline_matches_canonical_assembly_without_imports() {
    let canonical = Schema::parse_str(IMPORT_FREE_SCHEMA)
        .expect("schema parses")
        .assemble(&[])
        .expect("canonical assemble succeeds");

    let report = multi_pass::read_schema_with_report(IMPORT_FREE_SCHEMA)
        .expect("multi-pass macro pipeline succeeds");
    let assembled = report.assembled.expect("assembled output present");

    assert_eq!(assembled, canonical);
    assert_eq!(report.import_firings, 0);
    assert_eq!(report.header_firings, 2);
    assert_eq!(report.type_firings, 8);
    assert_eq!(report.feature_firings, 1);
}

#[test]
fn multi_pass_pipeline_rejects_non_uniform_header_shape() {
    let bad = r#"
{}
[(State Statement)]
[]
[]
{
  State [(Statement)]
  Statement (String)
}
[]
"#;

    let error = multi_pass::read_schema_six_position(bad).expect_err("bad header must fail");
    assert!(
        error
            .to_string()
            .contains("requires a `[...]` endpoint list")
    );
}
