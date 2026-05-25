use nota_codec::{NotaValue, parse_sequence};
use schema::{
    NamespaceValueShape, NodeDefinitionPoint, NodeDefinitionShape,
    multi_pass::read_schema_six_position,
};

fn parse_one(text: &str) -> NotaValue {
    let values = parse_sequence(text).expect("value parses");
    assert_eq!(values.len(), 1);
    values.into_iter().next().unwrap()
}

#[test]
fn namespace_shape_recognizer_splits_enum_record_newtype_and_alias() {
    let enum_value = parse_one("[Decision Principle]");
    let record_value = parse_one("(Topic Kind)");
    let newtype_value = parse_one("(String)");
    let alias_value = parse_one("Topic");

    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &enum_value).unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Enum)
    );
    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &record_value).unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Record)
    );
    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &newtype_value)
            .unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Newtype)
    );
    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &alias_value).unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Alias)
    );
}

#[test]
fn container_namespace_value_is_a_newtype_shape() {
    let value = parse_one("(Vec Topic)");

    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &value).unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Newtype)
    );
}

#[test]
fn lowercase_record_field_sugar_is_not_a_newtype_shape() {
    let value = parse_one("((state State))");

    assert_eq!(
        NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, &value).unwrap(),
        NodeDefinitionShape::NamespaceValue(NamespaceValueShape::Record)
    );
}

#[test]
fn lowercase_field_name_syntax_is_rejected() {
    let text = "
{}
[]
[]
[]
{
  State [Active Absent]
  StateObserved ((state State))
}
[]
";

    let error = read_schema_six_position(text).expect_err("field names are not NOTA");
    let message = format!("{error}");
    assert!(
        message.contains("positional") && message.contains("state"),
        "expected positional-field error, got: {message}"
    );
}

#[test]
fn node_shape_error_reports_definition_point_and_value_kind() {
    let value = parse_one("[Decision Principle]");

    let error =
        NodeDefinitionShape::recognize(NodeDefinitionPoint::HeaderRoot, &value).unwrap_err();
    let message = format!("{error}");

    assert!(
        message.contains("HeaderRoot"),
        "error should name the schema node point, got: {message}"
    );
    assert!(
        message.contains("Sequence"),
        "error should name the observed NOTA value kind, got: {message}"
    );
}

#[test]
fn multi_pass_pipeline_accepts_all_public_namespace_shapes() {
    let text = "
{}
[(Route [Record Alias Newtype ContainerNewtype Enum])]
[]
[]
{
  Route [(Record Record) (Alias Alias) (Newtype Newtype) (ContainerNewtype ContainerNewtype) (Enum Enum)]
  Record (Topic Kind)
  Alias Topic
  Newtype (String)
  ContainerNewtype (Vec Topic)
  Enum [Decision Principle]
  Topic (String)
  Kind [Decision Principle]
}
[]
";

    let assembled = read_schema_six_position(text).expect("pipeline accepts explicit shapes");
    assert_eq!(assembled.routes().len(), 5);
    assert_eq!(assembled.types().count(), 8);
}
