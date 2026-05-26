use schema_next::{
    MacroContext, MacroOutput, MacroPosition, Name, SchemaEngine, SchemaIdentity, SchemaMacro,
    TypeDeclaration,
};

#[test]
fn lowers_spirit_schema_into_ordered_asschema() {
    let source = include_str!("../schemas/spirit-min.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit", "0.1.0"))
        .expect("schema lowers");

    assert_eq!(asschema.imports.len(), 0);
    assert_eq!(asschema.surfaces[0].name.as_str(), "Input");
    assert_eq!(asschema.surfaces[0].variants[0].name.as_str(), "Record");
    assert_eq!(
        asschema.surfaces[0].variants[0]
            .payload
            .as_ref()
            .expect("payload")
            .name
            .as_str(),
        "Entry"
    );
    assert_eq!(
        asschema
            .namespace
            .iter()
            .map(|declaration| declaration.name().as_str())
            .collect::<Vec<_>>(),
        vec![
            "Topic",
            "Topics",
            "Description",
            "RecordIdentifier",
            "Entry",
            "Query",
            "RecordSet",
            "Kind",
            "Magnitude",
        ]
    );
}

#[test]
fn square_brackets_lower_to_structs_and_parentheses_lower_to_enums() {
    let source = "{} [] { Entry [Topic Kind] Kind (Decision Constraint) }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("schema lowers");

    assert!(matches!(asschema.namespace[0], TypeDeclaration::Struct(_)));
    assert!(matches!(asschema.namespace[1], TypeDeclaration::Enum(_)));
}

#[test]
fn macro_lowering_receives_macro_position() {
    struct ProbeMacro;

    impl SchemaMacro for ProbeMacro {
        fn name(&self) -> &'static str {
            "Probe"
        }

        fn matches(&self, _object: &nota_next::Block, position: MacroPosition) -> bool {
            position == MacroPosition::Surface
        }

        fn lower(
            &self,
            _object: &nota_next::Block,
            position: MacroPosition,
            context: &mut MacroContext,
        ) -> Result<MacroOutput, schema_next::SchemaError> {
            context.remember_position(position);
            Ok(MacroOutput::References(Vec::new()))
        }
    }

    let document = nota_next::Document::parse("(Input)").expect("nota parses");
    let mut context = MacroContext::default();
    let object = document.root_object_at(0).expect("root object");
    let probe = ProbeMacro;

    assert!(probe.matches(object, MacroPosition::Surface));
    probe
        .lower(object, MacroPosition::Surface, &mut context)
        .expect("probe lower");
    assert_eq!(context.positions_seen(), &[MacroPosition::Surface]);
}

#[test]
fn field_names_are_derived_from_type_names() {
    let source = "{} [] { Entry [RecordIdentifier Description] }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("schema lowers");
    let TypeDeclaration::Struct(entry) = &asschema.namespace[0] else {
        panic!("entry should be a struct");
    };

    assert_eq!(entry.fields[0].name, Name::new("record_identifier"));
    assert_eq!(entry.fields[1].name, Name::new("description"));
}
