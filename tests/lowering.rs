use schema_next::{
    MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry, Name, SchemaEngine,
    SchemaIdentity, SchemaMacro, TypeDeclaration,
};

#[test]
fn lowers_spirit_schema_into_ordered_asschema() {
    let source = include_str!("../schemas/spirit-min.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit", "0.1.0"))
        .expect("schema lowers");

    assert_eq!(asschema.imports().len(), 0);
    assert_eq!(asschema.surfaces()[0].name.as_str(), "Input");
    assert_eq!(asschema.surfaces()[0].variants[0].name.as_str(), "Record");
    assert_eq!(
        asschema.surfaces()[0].variants[0]
            .payload
            .as_ref()
            .expect("payload")
            .name
            .as_str(),
        "Entry"
    );
    assert_eq!(
        asschema
            .namespace()
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

    assert!(matches!(
        asschema.namespace()[0],
        TypeDeclaration::Struct(_)
    ));
    assert!(matches!(asschema.namespace()[1], TypeDeclaration::Enum(_)));
}

#[test]
fn macro_lowering_receives_macro_position() {
    struct ProbeMacro;

    impl SchemaMacro for ProbeMacro {
        fn name(&self) -> &'static str {
            "Probe"
        }

        fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
            position == MacroPosition::Surface && object.block().is_some()
        }

        fn lower(
            &self,
            _object: MacroObject<'_>,
            position: MacroPosition,
            context: &mut MacroContext,
            _registry: &MacroRegistry,
        ) -> Result<MacroOutput, schema_next::SchemaError> {
            context.remember_macro(self.name());
            context.remember_position(position);
            Ok(MacroOutput::References(Vec::new()))
        }
    }

    let document = nota_next::Document::parse("(Input)").expect("nota parses");
    let mut context = MacroContext::default();
    let object = document.root_object_at(0).expect("root object");
    let probe = ProbeMacro;

    assert!(probe.matches(MacroObject::Block(object), MacroPosition::Surface));
    probe
        .lower(
            MacroObject::Block(object),
            MacroPosition::Surface,
            &mut context,
            &MacroRegistry::new(),
        )
        .expect("probe lower");
    assert_eq!(context.positions_seen(), &[MacroPosition::Surface]);
    assert_eq!(context.macros_applied(), &["Probe"]);
}

#[test]
fn field_names_are_derived_from_type_names() {
    let source = "{} [] { Entry [RecordIdentifier Description] }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("schema lowers");
    let TypeDeclaration::Struct(entry) = &asschema.namespace()[0] else {
        panic!("entry should be a struct");
    };

    assert_eq!(entry.fields[0].name, Name::new("record_identifier"));
    assert_eq!(entry.fields[1].name, Name::new("description"));
}

#[test]
fn default_engine_dispatches_through_registered_macros() {
    let source = include_str!("../schemas/spirit-min.schema");
    let mut context = MacroContext::default();

    SchemaEngine::default()
        .lower_source_with_context(source, SchemaIdentity::new("spirit", "0.1.0"), &mut context)
        .expect("schema lowers through macros");

    assert_eq!(
        context.macros_applied(),
        &[
            "RootImports",
            "RootSurfaces",
            "Surface",
            "Surface",
            "RootNamespace",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "StructFields",
            "TypeDeclaration",
            "EnumVariants",
            "TypeDeclaration",
            "EnumVariants",
        ]
    );
    assert_eq!(
        context.positions_seen(),
        &[
            MacroPosition::RootImports,
            MacroPosition::RootSurfaces,
            MacroPosition::Surface,
            MacroPosition::Surface,
            MacroPosition::RootNamespace,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::EnumVariants,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::EnumVariants,
        ]
    );
}

#[test]
fn schema_engine_can_be_built_from_a_macro_registry() {
    let mut registry = MacroRegistry::new();
    registry.register(RejectingRootImports);
    let engine = SchemaEngine::with_registry(registry);
    let error = engine
        .lower_source("{} [] {}", SchemaIdentity::new("example", "0.1.0"))
        .expect_err("custom registry should reject");

    assert_eq!(
        error,
        schema_next::SchemaError::ExpectedDelimiter {
            expected: "rejecting test macro"
        }
    );
}

struct RejectingRootImports;

impl SchemaMacro for RejectingRootImports {
    fn name(&self) -> &'static str {
        "RejectingRootImports"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::RootImports && object.block().is_some()
    }

    fn lower(
        &self,
        _object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, schema_next::SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        Err(schema_next::SchemaError::ExpectedDelimiter {
            expected: "rejecting test macro",
        })
    }
}
