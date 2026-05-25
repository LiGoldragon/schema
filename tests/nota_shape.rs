use nota_codec::{NotaValue, parse_sequence};

fn spirit_fixture() -> Vec<NotaValue> {
    parse_sequence(include_str!("fixtures/schema-e2e/spirit-v0-1-1.schema"))
        .expect("fixture parses as generic top-level NOTA values")
}

fn map_value<'a>(map: &'a [nota_codec::NotaMapEntry], key: &str) -> &'a NotaValue {
    map.iter()
        .find(|entry| entry.key() == key)
        .unwrap_or_else(|| panic!("missing map key {key}"))
        .value()
}

#[test]
fn first_pass_sees_six_schema_positions_and_local_import_shapes() {
    let values = spirit_fixture();

    assert_eq!(values.len(), 6);
    assert!(values[0].is_map());
    assert!(values[1].is_sequence());
    assert!(values[2].is_sequence());
    assert!(values[3].is_sequence());
    assert!(values[4].is_map());
    assert!(values[5].is_sequence());

    let imports = values[0].as_map().expect("imports map");
    assert_eq!(imports.len(), 3);
    assert!(map_value(imports, "Magnitude").has_data_shape("ImportAll", 1));
    assert!(map_value(imports, "SemaSet").has_data_shape("Import", 2));
    assert!(map_value(imports, "Shared").has_data_shape("Import", 2));

    let magnitude_import = map_value(imports, "Magnitude")
        .as_record()
        .expect("import directive record");
    assert_eq!(
        magnitude_import[1].identifier_text(),
        Some("./magnitude.schema")
    );
}

#[test]
fn first_pass_classifies_namespace_macro_candidate_shapes() {
    let values = spirit_fixture();
    let namespace = values[4].as_map().expect("namespace map");

    let route_enum = map_value(namespace, "State");
    assert!(route_enum.is_record());
    let variants = route_enum.as_record().expect("route body variants");
    assert_eq!(variants.len(), 2);
    assert!(variants[0].has_data_shape("Statement", 0));
    assert!(variants[1].has_data_shape("Declaration", 0));

    let newtype_candidate = map_value(namespace, "Topic");
    assert!(newtype_candidate.is_sequence());
    let newtype_items = newtype_candidate
        .as_sequence()
        .expect("newtype candidate sequence");
    assert_eq!(newtype_items.len(), 1);
    assert_eq!(newtype_items[0].identifier_text(), Some("String"));

    let record_candidate = map_value(namespace, "Entry");
    assert!(record_candidate.is_sequence());
    let record_fields = record_candidate.as_sequence().expect("record fields");
    assert_eq!(record_fields.len(), 8);
    assert_eq!(record_fields[0].identifier_text(), Some("Topic"));

    let container_field = map_value(namespace, "RecordsObserved")
        .as_sequence()
        .expect("record with vector field");
    assert_eq!(container_field.len(), 1);
    assert!(container_field[0].has_data_shape("Vec", 1));
}

#[test]
fn first_pass_classifies_upgrade_macro_shape() {
    let values = spirit_fixture();
    let features = values[5].as_sequence().expect("features vector");
    let upgrade = features
        .iter()
        .find(|value| value.is_tagged_record("Upgrade"))
        .expect("upgrade feature");

    assert!(upgrade.has_data_shape("Upgrade", 2));

    let items = upgrade.as_record().expect("upgrade record");
    assert!(items[1].has_data_shape("FromVersion", 1));
    assert_eq!(
        items[1].as_record().expect("from version record")[1].identifier_text(),
        Some("v0.1")
    );
    assert!(items[2].has_data_shape("Migrate", 1));
    assert_eq!(
        items[2].as_record().expect("migrate record")[1].identifier_text(),
        Some("Entry")
    );
}
