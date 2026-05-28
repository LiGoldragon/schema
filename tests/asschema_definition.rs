use schema_next::{Asschema, SchemaEngine, SchemaIdentity, TypeDeclaration, TypeReference};

#[test]
fn asschema_schema_is_final_macro_free_data() {
    let source = include_str!("../schemas/asschema.asschema");

    assert!(
        !source.contains('@'),
        ".asschema must not contain macro markers"
    );
    assert!(
        !source.contains("SchemaMacro"),
        ".asschema defines final data, not macro definitions"
    );
    assert!(
        !source.contains("$Name") && !source.contains("$*"),
        ".asschema must not contain macro captures"
    );

    let asschema = Asschema::from_nota(source).expect("assembled schema definition parses");

    assert_eq!(
        asschema.identity().component().as_str(),
        "schema-next:asschema"
    );
    assert_eq!(asschema.identity().version(), "0.1.0");
    assert!(asschema.imports().is_empty());
    assert!(asschema.resolved_imports().is_empty());

    let TypeDeclaration::Struct(root) = asschema.type_named("Asschema").expect("Asschema type")
    else {
        panic!("Asschema must be a struct declaration");
    };
    assert_eq!(
        root.fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "identity",
            "imports",
            "resolved_imports",
            "input",
            "output",
            "namespace"
        ]
    );

    let TypeDeclaration::Enum(type_reference) = asschema
        .type_named("TypeReference")
        .expect("TypeReference declaration")
    else {
        panic!("TypeReference must be an enum declaration");
    };
    assert_eq!(
        type_reference
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.name.as_str(),
                    variant
                        .payload
                        .as_ref()
                        .and_then(TypeReference::plain_name)
                        .map(|name| name.as_str()),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Plain", Some("Name")),
            ("Vector", Some("TypeReference")),
            ("Optional", Some("TypeReference")),
            ("Map", Some("TypeReferencePair")),
        ]
    );

    let TypeDeclaration::Struct(schema_node) = asschema
        .type_named("SchemaNode")
        .expect("SchemaNode declaration")
    else {
        panic!("SchemaNode must be a struct declaration");
    };
    assert_eq!(
        schema_node
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["tag", "data"],
        "macro calls are represented as tagged data nodes in assembled schema",
    );

    let TypeDeclaration::Enum(schema_node_data) = asschema
        .type_named("SchemaNodeData")
        .expect("SchemaNodeData declaration")
    else {
        panic!("SchemaNodeData must be an enum declaration");
    };
    assert_eq!(
        schema_node_data
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Unit", "Value", "Vector", "Map"],
    );
}

#[test]
fn lowered_asschema_uses_final_collection_variants_not_macro_sugar() {
    let source = "
        () ()
        {
          Topic [Text]
          Topics [(items (Vec [Topic]))]
          Query [(limit (Option [Integer]))]
          RecordSet [(byTopic (KeyValue [Topic Integer]))]
        }
    ";

    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example:collections", "0.1.0"))
        .expect("schema lowers");
    let rendered = asschema.to_nota();

    assert!(
        !rendered.contains('@'),
        "assembled schema must not contain macro markers"
    );
    assert!(rendered.contains("(Vector (Plain Topic))"));
    assert!(rendered.contains("(Optional (Plain Integer))"));
    assert!(rendered.contains("(Map [(Plain Topic) (Plain Integer)])"));
    assert!(
        !rendered.contains("(Map (Plain Topic) (Plain Integer))"),
        "Map is a final variant carrying one vector payload, not loose macro arguments"
    );
    assert_eq!(
        Asschema::from_nota(&rendered).expect("rendered asschema parses"),
        asschema
    );
}
