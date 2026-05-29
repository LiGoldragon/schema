use schema_next::{
    DeclarativeMacroLibrary, MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry,
    Name, SchemaEngine, SchemaIdentity, SchemaMacro, SchemaPackage, TypeDeclaration, TypeReference,
};

#[test]
fn lowers_spirit_schema_into_ordered_asschema() {
    let source = include_str!("../schemas/spirit-min.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit", "0.1.0"))
        .expect("schema lowers");

    assert_eq!(asschema.imports().len(), 0);
    assert_eq!(asschema.roots().len(), 2);
    assert_eq!(asschema.roots()[0].name().as_str(), "Input");
    assert_eq!(asschema.roots()[1].name().as_str(), "Output");
    assert_eq!(
        asschema
            .root_named("Input")
            .expect("input root")
            .enum_declaration()
            .variants[0]
            .name
            .as_str(),
        "Record"
    );
    assert_eq!(asschema.input().name.as_str(), "Input");
    assert_eq!(asschema.input().variants[0].name.as_str(), "Record");
    assert_eq!(
        asschema.input().variants[0]
            .payload
            .as_ref()
            .expect("payload")
            .plain_name()
            .expect("plain payload")
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
fn pipe_declarations_lower_to_structs_and_enums() {
    let source =
        "() () { Entry {| Entry topic Topic kind Kind |} Kind (| Kind Decision Constraint |) }";
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
fn brace_namespace_rejects_parenthesized_named_objects() {
    let source = "() () { (Entry {| Entry topic Topic kind Kind |}) }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("brace namespaces are key-value maps only");

    assert_eq!(
        error,
        schema_next::SchemaError::ExpectedEvenMapEntries { found: 1 }
    );
}

#[test]
fn brace_namespace_rejects_parenthesized_named_objects_even_when_count_is_even() {
    let source =
        "() () { (Entry {| Entry topic Topic kind Kind |}) (Kind (| Kind Decision Constraint |)) }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("brace namespace keys must be symbols");

    assert!(matches!(
        error,
        schema_next::SchemaError::ExpectedSymbol { .. }
    ));
}

#[test]
fn colon_qualified_names_lower_as_schema_names() {
    let source = "((Record schema:spirit:Entry)) () { schema:spirit:Topic {| schema:spirit:Topic string String |} schema:spirit:Entry {| schema:spirit:Entry topic schema:spirit:Topic |} }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("schema:spirit:lib", "0.1.0"))
        .expect("schema lowers");

    assert_eq!(
        asschema.input().variants[0]
            .payload
            .as_ref()
            .expect("record payload")
            .plain_name()
            .expect("plain payload")
            .as_str(),
        "schema:spirit:Entry"
    );
    assert_eq!(
        asschema.namespace()[1].name().namespace_segments(),
        vec!["schema", "spirit", "Entry"]
    );
    let TypeDeclaration::Newtype(topic) = &asschema.namespace()[0] else {
        panic!("topic should be a newtype");
    };
    assert_eq!(topic.name.local_part(), "Topic");
    let TypeDeclaration::Newtype(entry) = &asschema.namespace()[1] else {
        panic!("single-field entry should be a newtype");
    };
    assert_eq!(entry.fields[0].name, Name::new("topic"));
}

#[test]
fn package_loader_reads_schema_lib_entrypoint() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("spirit-crate");
    let package = SchemaPackage::new(root, "spirit-next", "0.1.0");
    let source = package.load_lib().expect("load lib.schema");
    let asschema = source
        .lower(&SchemaEngine::default())
        .expect("schema lowers");

    assert_eq!(source.path(), package.lib_schema_path());
    assert_eq!(asschema.identity().component().as_str(), "spirit-next:lib");
    assert!(asschema.type_named("Entry").is_some());
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
            .map(|field| field.reference.plain_name().expect("plain field").as_str())
            .collect::<Vec<_>>(),
        vec!["Input", "Output", "Namespace"]
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
                    .map(|payload| payload.plain_name().expect("plain payload").as_str())
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
            .map(|field| field.reference.plain_name().expect("plain field").as_str())
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
            "TypeReference",
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
    let source =
        "() () { Entry {| Entry recordIdentifier RecordIdentifier description Description |} }";
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
        template == "SchemaStructDefinition -> (Type (Struct Entry [topics Topics kind Kind description Description magnitude Magnitude]))"
    }));
    assert!(context.expanded_templates().iter().any(|template| {
        template == "SchemaEnumDefinition -> (Type (Enum Kind (Decision Principle Correction Clarification Constraint)))"
    }));
    assert_eq!(
        context.positions_seen(),
        &[
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
        .lower_source("{} () () {}", SchemaIdentity::new("example", "0.1.0"))
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

#[test]
fn brace_body_is_not_enum_sugar_inside_namespace() {
    let source = "() () { Routing {ToInbox Address ToOutbox Address} }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("brace values are maps, not enum sugar");

    assert!(matches!(
        error,
        schema_next::SchemaError::MacroDidNotMatch { .. }
    ));
}

#[test]
fn pipe_declaration_flat_field_pairs_lower_through_default_engine() {
    let source =
        "() () { Entry {| Entry recordIdentifier RecordIdentifier description Description |} }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("pipe declaration lowers");
    let TypeDeclaration::Struct(entry) = &asschema.namespace()[0] else {
        panic!("entry should be a struct");
    };

    assert_eq!(entry.fields[0].name, Name::new("record_identifier"));
    assert_eq!(
        entry.fields[0].reference,
        TypeReference::Plain(Name::new("RecordIdentifier"))
    );
    assert_eq!(entry.fields[1].name, Name::new("description"));
}

#[test]
fn inline_pipe_declaration_creates_ordered_namespace_type() {
    let source = "() () { Entry {| Entry receipt {| Receipt recordIdentifier RecordIdentifier |} later Receipt |} }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("inline declaration lowers");

    assert_eq!(
        asschema
            .namespace()
            .iter()
            .map(|declaration| declaration.name().as_str())
            .collect::<Vec<_>>(),
        vec!["Receipt", "Entry"]
    );

    let TypeDeclaration::Struct(entry) = &asschema.namespace()[1] else {
        panic!("entry should be a struct");
    };
    assert_eq!(entry.fields[0].name, Name::new("receipt"));
    assert_eq!(
        entry.fields[0].reference,
        TypeReference::Plain(Name::new("Receipt"))
    );
    assert_eq!(entry.fields[1].name, Name::new("later"));
    assert_eq!(
        entry.fields[1].reference,
        TypeReference::Plain(Name::new("Receipt"))
    );
}

#[test]
fn root_enum_rejects_labeled_input_and_output_positions() {
    let error = SchemaEngine::default()
        .lower_source(
            "(Input ((Record Entry))) () {}",
            SchemaIdentity::new("example", "0.1.0"),
        )
        .expect_err("root input label should be rejected");
    assert_eq!(
        error,
        schema_next::SchemaError::RootEnumLabelForbidden {
            label: "Input".to_owned()
        }
    );

    let error = SchemaEngine::default()
        .lower_source(
            "() (Output ((Accepted Receipt))) {}",
            SchemaIdentity::new("example", "0.1.0"),
        )
        .expect_err("root output label should be rejected");
    assert_eq!(
        error,
        schema_next::SchemaError::RootEnumLabelForbidden {
            label: "Output".to_owned()
        }
    );
}
