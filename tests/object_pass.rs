use schema::{
    ModuleName, ObjectDelimiter, ObjectPathSegment, ObjectPosition, SchemaObjectPass,
    identifier_vector,
};

fn object_pass_fixture() -> &'static str {
    "
{ Magnitude (ImportAll ./magnitude.schema) }
[
  (State (Statement Declaration))
  (Record (Entry))
]
{
  Topic [String]
  Topics [(Vec Topic)]
  Entry [Topics Kind Description Magnitude]
  Kind (Decision Principle Correction Clarification Constraint)
  Observation (State (Records RecordQuery) Topics)
}
"
}

#[test]
fn object_pass_preserves_file_namespace_prefix_and_root_delimiters() {
    let pass = SchemaObjectPass::parse_text(
        ModuleName::new("spirit_signal").unwrap(),
        object_pass_fixture(),
    )
    .expect("object pass parses");

    assert_eq!(pass.namespace_prefix().as_str(), "spirit_signal");
    let delimiters = pass
        .roots()
        .iter()
        .map(|root| root.delimiter())
        .collect::<Vec<_>>();

    assert_eq!(
        delimiters,
        vec![
            ObjectDelimiter::CurlyBraces,
            ObjectDelimiter::SquareBrackets,
            ObjectDelimiter::CurlyBraces
        ]
    );
}

#[test]
fn object_pass_preserves_namespace_map_order_and_value_delimiters() {
    let pass = SchemaObjectPass::parse_text(
        ModuleName::new("spirit_signal").unwrap(),
        object_pass_fixture(),
    )
    .expect("object pass parses");
    let namespace_root = pass
        .namespace_roots()
        .nth(1)
        .expect("second map is namespace");
    let entries = namespace_root
        .namespace_entries()
        .expect("namespace entries");

    assert_eq!(
        entries
            .iter()
            .map(|entry| (entry.name(), entry.delimiter()))
            .collect::<Vec<_>>(),
        vec![
            ("Topic", ObjectDelimiter::SquareBrackets),
            ("Topics", ObjectDelimiter::SquareBrackets),
            ("Entry", ObjectDelimiter::SquareBrackets),
            ("Kind", ObjectDelimiter::Parentheses),
            ("Observation", ObjectDelimiter::Parentheses),
        ]
    );
}

#[test]
fn object_pass_exposes_identifier_vectors_before_schema_semantics() {
    let pass = SchemaObjectPass::parse_text(
        ModuleName::new("spirit_signal").unwrap(),
        object_pass_fixture(),
    )
    .expect("object pass parses");
    let namespace_root = pass.namespace_roots().nth(1).expect("namespace");
    let entries = namespace_root
        .namespace_entries()
        .expect("namespace entries");
    let entry = entries
        .iter()
        .find(|entry| entry.name() == "Entry")
        .expect("Entry declaration");

    assert_eq!(
        entry.identifier_vector(),
        Some(vec!["Topics", "Kind", "Description", "Magnitude"])
    );

    let value = nota_codec::parse_str("[State Record Observe]").expect("sequence parses");
    assert_eq!(
        identifier_vector(&value),
        Some(vec!["State", "Record", "Observe"])
    );
}

#[test]
fn object_pass_recursively_records_delimited_object_paths() {
    let pass = SchemaObjectPass::parse_text(
        ModuleName::new("spirit_signal").unwrap(),
        object_pass_fixture(),
    )
    .expect("object pass parses");

    let state_root = pass
        .all_objects()
        .into_iter()
        .find(|node| {
            node.delimiter() == ObjectDelimiter::Parentheses
                && node.position() == ObjectPosition::SequenceItem
                && node.head() == Some("State")
        })
        .expect("State header root object");

    assert_eq!(
        state_root.path().segments(),
        &[
            ObjectPathSegment::Root(1),
            ObjectPathSegment::SequenceItem(0)
        ]
    );
}

#[test]
fn object_pass_derives_prefix_from_schema_file_path() {
    let pass = SchemaObjectPass::read_path("tests/fixtures/schema-e2e/spirit-v0-1-1.schema")
        .expect("fixture reads");

    assert_eq!(pass.namespace_prefix().as_str(), "spirit_v0_1_1");
    assert_eq!(
        pass.roots().first().map(|root| root.delimiter()),
        Some(ObjectDelimiter::CurlyBraces)
    );
}
