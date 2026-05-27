use schema_next::{
    DeclarativeMacroLibrary, MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry,
    Name, SchemaEngine, SchemaIdentity, SchemaMacro, TypeDeclaration,
};

#[test]
fn lowers_spirit_schema_into_ordered_asschema() {
    let source = include_str!("../schemas/spirit-min.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit", "0.1.0"))
        .expect("schema lowers");

    assert_eq!(asschema.imports().len(), 0);
    assert_eq!(asschema.input().name.as_str(), "Input");
    assert_eq!(asschema.input().variants[0].name.as_str(), "Record");
    assert_eq!(
        asschema.input().variants[0]
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
    let source = "{} (Input ()) (Output ()) { (Entry [Topic Kind]) (Kind (Decision Constraint)) }";
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
fn root_schema_describes_the_schema_root_type() {
    let source = include_str!("../schemas/root.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema", "0.1.0"))
        .expect("root schema lowers");

    assert_eq!(asschema.input().name.as_str(), "Input");
    assert_eq!(asschema.output().name.as_str(), "Output");

    let TypeDeclaration::Struct(schema) = asschema
        .type_named("Schema")
        .expect("schema type declaration")
    else {
        panic!("Schema should be a struct");
    };

    assert_eq!(
        schema
            .fields
            .iter()
            .map(|field| field.reference.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Imports", "Input", "Output", "Namespace"]
    );

    let TypeDeclaration::Enum(type_declaration) = asschema
        .type_named("TypeDeclaration")
        .expect("type declaration enum")
    else {
        panic!("TypeDeclaration should be an enum");
    };
    assert_eq!(
        type_declaration
            .variants
            .iter()
            .map(|variant| (
                variant.name.as_str(),
                variant
                    .payload
                    .as_ref()
                    .map(|payload| payload.name.as_str())
            ))
            .collect::<Vec<_>>(),
        vec![
            ("Struct", Some("StructDeclaration")),
            ("Enum", Some("EnumDeclaration")),
            ("Newtype", Some("StructDeclaration")),
        ]
    );
}

#[test]
fn core_schema_describes_default_builtin_macro_positions() {
    let source = include_str!("../schemas/core.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema-core", "0.1.0"))
        .expect("core schema lowers");

    let TypeDeclaration::Struct(core_schema) = asschema
        .type_named("CoreSchema")
        .expect("core schema declaration")
    else {
        panic!("CoreSchema should be a struct");
    };
    assert_eq!(
        core_schema
            .fields
            .iter()
            .map(|field| field.reference.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "BuiltinMacroPositions",
            "BuiltinMacroShapes",
            "BuiltinMacroOutputs",
            "BuiltinMacroDefinitions",
        ]
    );

    let TypeDeclaration::Enum(macro_position) = asschema
        .type_named("MacroPosition")
        .expect("macro position enum")
    else {
        panic!("MacroPosition should be an enum");
    };
    assert_eq!(
        macro_position
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "RootImports",
            "RootInput",
            "RootOutput",
            "RootNamespace",
            "NamespaceDeclaration",
            "StructFields",
            "EnumVariants",
        ]
    );
}

#[test]
fn builtin_macro_file_defines_visible_dollar_captures() {
    let library = DeclarativeMacroLibrary::builtin().expect("builtin macros parse");
    let names = library
        .definitions()
        .iter()
        .map(|definition| definition.name().as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec![
            "SchemaStructDefinition",
            "SchemaEnumDefinition",
            "SchemaStructFields",
            "SchemaEnumVariants",
        ]
    );

    let struct_definition = library
        .definitions()
        .iter()
        .find(|definition| definition.name().as_str() == "SchemaStructDefinition")
        .expect("struct macro definition");
    assert_eq!(struct_definition.capture_names(), vec!["$Name", "$*Fields"]);

    let enum_definition = library
        .definitions()
        .iter()
        .find(|definition| definition.name().as_str() == "SchemaEnumDefinition")
        .expect("enum macro definition");
    assert_eq!(enum_definition.capture_names(), vec!["$Name", "$*Variants"]);
}

#[test]
fn macro_lowering_receives_macro_position() {
    struct ProbeMacro;

    impl SchemaMacro for ProbeMacro {
        fn name(&self) -> &str {
            "Probe"
        }

        fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
            position == MacroPosition::RootInput && object.block().is_some()
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

    assert!(probe.matches(MacroObject::Block(object), MacroPosition::RootInput));
    probe
        .lower(
            MacroObject::Block(object),
            MacroPosition::RootInput,
            &mut context,
            &MacroRegistry::new(),
        )
        .expect("probe lower");
    assert_eq!(context.positions_seen(), &[MacroPosition::RootInput]);
    assert_eq!(
        context
            .macros_applied()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["Probe"]
    );
}

#[test]
fn field_names_are_derived_from_type_names() {
    let source = "{} (Input ()) (Output ()) { (Entry [RecordIdentifier Description]) }";
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
        context
            .macros_applied()
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec![
            "RootImports",
            "RootInput",
            "SchemaEnumVariants",
            "RootOutput",
            "SchemaEnumVariants",
            "RootNamespace",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaStructDefinition",
            "SchemaStructFields",
            "SchemaEnumDefinition",
            "SchemaEnumVariants",
            "SchemaEnumDefinition",
            "SchemaEnumVariants",
        ]
    );
    assert!(
        context
            .bindings_seen()
            .iter()
            .any(|binding| binding == "SchemaStructDefinition::Name")
    );
    assert!(
        context
            .bindings_seen()
            .iter()
            .any(|binding| binding == "SchemaStructDefinition::*Fields")
    );
    assert!(context.expanded_templates().iter().any(|template| {
        template
            == "SchemaStructDefinition -> (Type (Struct Entry [Topics Kind Description Magnitude]))"
    }));
    assert!(context.expanded_templates().iter().any(|template| {
        template == "SchemaEnumDefinition -> (Type (Enum Kind (Decision Principle Correction Clarification Constraint)))"
    }));
    assert_eq!(
        context.positions_seen(),
        &[
            MacroPosition::RootImports,
            MacroPosition::RootInput,
            MacroPosition::EnumVariants,
            MacroPosition::RootOutput,
            MacroPosition::EnumVariants,
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
        .lower_source(
            "{} (Input ()) (Output ()) {}",
            SchemaIdentity::new("example", "0.1.0"),
        )
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
    fn name(&self) -> &str {
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
