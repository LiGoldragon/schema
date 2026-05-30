use schema_next::{
    DeclarativeMacroLibrary, MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry,
    Name, SchemaEngine, SchemaIdentity, SchemaMacro, SchemaPackage, TypeDeclaration, TypeReference,
    Visibility,
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
fn at_declarations_lower_to_structs_and_enums() {
    let source = "[] [] { Entry { topic Topic kind Kind } Kind [Decision Constraint] }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("schema lowers");

    assert!(matches!(
        asschema.namespace()[0].value(),
        TypeDeclaration::Struct(_)
    ));
    assert!(matches!(
        asschema.namespace()[1].value(),
        TypeDeclaration::Enum(_)
    ));
}

#[test]
fn simple_newtype_declarations_lower_to_single_contained_reference() {
    let source = "[] [] { Topic String Topics (Vec Topic) }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("newtype forms lower");

    let TypeDeclaration::Newtype(topic) = asschema.type_named("Topic").expect("topic type") else {
        panic!("Topic should be a newtype");
    };
    assert_eq!(topic.reference, TypeReference::String);

    let TypeDeclaration::Newtype(topics) = asschema.type_named("Topics").expect("topics type")
    else {
        panic!("Topics should be a newtype");
    };
    assert_eq!(
        topics.reference,
        TypeReference::Vector(Box::new(TypeReference::new("Topic")))
    );
}

#[test]
fn brace_namespace_rejects_parenthesized_named_objects() {
    let source = "Input@[] Output@[] { (Entry Entry@{ topic@Topic kind@Kind }) }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("brace namespaces contain self-named declarations only");

    assert!(matches!(
        error,
        schema_next::SchemaError::ExpectedDelimiter { .. }
            | schema_next::SchemaError::MacroDidNotMatch { .. }
            | schema_next::SchemaError::UnsupportedMacroNodeStructure { .. }
    ));
}

#[test]
fn brace_namespace_rejects_redundant_key_value_declarations() {
    let source = "Input@[] Output@[] { Entry Entry@{ topic@Topic kind@Kind } }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("namespace declarations must be key/value pairs without duplicated names");

    assert!(matches!(
        error,
        schema_next::SchemaError::ExpectedDelimiter { .. }
            | schema_next::SchemaError::UnsupportedMacroNodeStructure { .. }
    ));
}

#[test]
fn colon_qualified_names_lower_as_schema_names() {
    let source = "[Record@ schema:spirit:Entry] [] { schema:spirit:Topic String schema:spirit:Entry schema:spirit:Topic }";
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
    let TypeDeclaration::Newtype(topic) = asschema.namespace()[0].value() else {
        panic!("topic should be a newtype");
    };
    assert_eq!(topic.name.local_part(), "Topic");
    let TypeDeclaration::Newtype(entry) = asschema.namespace()[1].value() else {
        panic!("single-field entry should be a newtype");
    };
    assert_eq!(
        entry.reference,
        TypeReference::Plain(Name::new("schema:spirit:Topic"))
    );
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
            ("Newtype", Some("NewtypeDeclaration")),
        ]
    );

    let TypeDeclaration::Enum(declaration) = asschema
        .type_named("Declaration")
        .expect("declaration enum")
    else {
        panic!("Declaration should be an enum");
    };
    assert_eq!(
        declaration
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
            ("Public", Some("NamedTypeDeclaration")),
            ("Private", Some("NamedTypeDeclaration")),
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

    let TypeDeclaration::Newtype(macro_pattern) = asschema
        .type_named("MacroPattern")
        .expect("macro pattern newtype")
    else {
        panic!("MacroPattern should be a newtype");
    };
    assert_eq!(
        macro_pattern
            .reference
            .plain_name()
            .expect("macro pattern object reference")
            .as_str(),
        "MacroPatternObject"
    );

    let TypeDeclaration::Enum(macro_pattern_object) = asschema
        .type_named("MacroPatternObject")
        .expect("macro pattern object enum")
    else {
        panic!("MacroPatternObject should be an enum");
    };
    assert_eq!(
        macro_pattern_object
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.name.as_str(),
                    variant
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.plain_name())
                        .map(Name::as_str),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Capture", Some("MacroCaptureName")),
            ("RestCapture", Some("MacroCaptureName")),
            ("Atom", Some("MacroAtom")),
            ("Delimited", Some("MacroPatternDelimited")),
        ]
    );

    let TypeDeclaration::Enum(macro_template_object) = asschema
        .type_named("MacroTemplateObject")
        .expect("macro template object enum")
    else {
        panic!("MacroTemplateObject should be an enum");
    };
    assert_eq!(
        macro_template_object
            .variants
            .iter()
            .map(|variant| {
                (
                    variant.name.as_str(),
                    variant
                        .payload
                        .as_ref()
                        .and_then(|payload| payload.plain_name())
                        .map(Name::as_str),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            ("Capture", Some("MacroCaptureName")),
            ("RestCapture", Some("MacroCaptureName")),
            ("Atom", Some("MacroAtom")),
            ("Delimited", Some("MacroTemplateDelimited")),
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
    let source = "[] [] { Entry { recordIdentifier RecordIdentifier description Description } }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("schema lowers");
    let TypeDeclaration::Struct(entry) = asschema.namespace()[0].value() else {
        panic!("entry should be a struct");
    };

    assert_eq!(entry.fields[0].name, Name::new("record_identifier"));
    assert_eq!(entry.fields[1].name, Name::new("description"));
}

#[test]
fn default_engine_lowers_through_registered_structural_forms() {
    let source = include_str!("../schemas/spirit-min.schema");
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit", "0.1.0"))
        .expect("schema lowers through macros");

    let input = asschema.root_named("Input").expect("input root");
    assert_eq!(
        input
            .enum_declaration()
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Record", "Observe"]
    );

    let output = asschema.root_named("Output").expect("output root");
    assert_eq!(
        output
            .enum_declaration()
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["RecordAccepted", "RecordsObserved"]
    );

    let entry = asschema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == "Entry")
        .expect("entry declaration");
    let TypeDeclaration::Struct(entry) = entry.value() else {
        panic!("entry should lower as a struct");
    };
    assert_eq!(
        entry
            .fields
            .iter()
            .map(|field| (
                field.name.as_str(),
                field.reference.plain_name().map(Name::as_str)
            ))
            .collect::<Vec<_>>(),
        vec![
            ("topics", Some("Topics")),
            ("kind", Some("Kind")),
            ("description", Some("Description")),
            ("magnitude", Some("Magnitude")),
        ]
    );

    let kind = asschema
        .namespace()
        .iter()
        .find(|declaration| declaration.name().as_str() == "Kind")
        .expect("kind declaration");
    let TypeDeclaration::Enum(kind) = kind.value() else {
        panic!("kind should lower as an enum");
    };
    assert_eq!(
        kind.variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec![
            "Decision",
            "Principle",
            "Correction",
            "Clarification",
            "Constraint",
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
            "{} Input@[] Output@[] {}",
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

#[test]
fn brace_body_is_not_enum_sugar_inside_namespace() {
    let source = "Input@[] Output@[] { Routing {ToInbox Address ToOutbox Address} }";
    let error = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("brace values are maps, not enum sugar");

    assert!(matches!(
        error,
        schema_next::SchemaError::ExpectedSyntaxReferenceArity { .. }
    ));
}

#[test]
fn at_declaration_field_pairs_lower_through_default_engine() {
    let source = "[] [] { Entry { recordIdentifier RecordIdentifier description Description } }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("at declaration lowers");
    let TypeDeclaration::Struct(entry) = asschema.namespace()[0].value() else {
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
fn at_type_shorthand_derives_fields_and_data_variant_payloads_from_real_schema() {
    let source = include_str!("fixtures/big-schemas/derived-members.schema");
    let asschema = SchemaEngine::default()
        .lower_source(
            source,
            SchemaIdentity::new("example:derived-members", "0.1.0"),
        )
        .expect("derived member schema lowers");

    let TypeDeclaration::Struct(entry) = asschema.type_named("Entry").expect("entry type") else {
        panic!("entry should be a struct");
    };
    assert_eq!(
        entry
            .fields
            .iter()
            .map(|field| (
                field.name.as_str(),
                field.reference.plain_name().map(Name::as_str)
            ))
            .collect::<Vec<_>>(),
        vec![
            ("topics", Some("Topics")),
            ("kind", Some("Kind")),
            ("description", Some("Description")),
            ("magnitude", Some("Magnitude")),
        ]
    );

    let TypeDeclaration::Struct(query) = asschema.type_named("Query").expect("query type") else {
        panic!("query should remain a struct");
    };
    assert_eq!(query.fields[0].name.as_str(), "topics");
    assert_eq!(
        query.fields[1].reference,
        TypeReference::Optional(Box::new(TypeReference::Integer))
    );

    let TypeDeclaration::Enum(some_enum) = asschema.type_named("SomeEnum").expect("some enum type")
    else {
        panic!("SomeEnum should be an enum");
    };
    assert_eq!(
        some_enum
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        vec!["SomethingHoldingSomething", "SomethingElse", "SomeString"]
    );
    assert_eq!(
        some_enum.variants[0].payload,
        Some(TypeReference::Plain(Name::new("SomethingHoldingSomething")))
    );
    assert_eq!(some_enum.variants[1].payload, None);
    assert_eq!(some_enum.variants[2].payload, Some(TypeReference::String));

    let TypeDeclaration::Newtype(topic) = asschema.type_named("Topic").expect("topic type") else {
        panic!("Topic should be a newtype");
    };
    assert_eq!(topic.reference, TypeReference::String);
}

#[test]
fn inline_at_declaration_creates_ordered_namespace_type() {
    let source = "Input@[] Output@[] { Entry@{ Receipt@{ recordIdentifier@RecordIdentifier } later@Receipt } }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("inline declaration lowers");

    assert_eq!(
        asschema
            .namespace()
            .iter()
            .map(|declaration| (declaration.name().as_str(), declaration.visibility()))
            .collect::<Vec<_>>(),
        vec![
            ("Receipt", Visibility::Private),
            ("Entry", Visibility::Public),
        ]
    );

    let TypeDeclaration::Struct(entry) = asschema.namespace()[1].value() else {
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
fn root_enum_requires_named_input_and_output_declarations() {
    let asschema = SchemaEngine::default()
        .lower_source(
            "[Record@ Entry] [] {}",
            SchemaIdentity::new("example", "0.1.0"),
        )
        .expect("bare input root lowers because the root position names it");
    assert_eq!(asschema.input().name.as_str(), "Input");
    assert_eq!(asschema.input().variants[0].name.as_str(), "Record");

    let error = SchemaEngine::default()
        .lower_source(
            "Input@[] Reply@[Accepted@Receipt] {}",
            SchemaIdentity::new("example", "0.1.0"),
        )
        .expect_err("root output must be named Output");
    assert_eq!(
        error,
        schema_next::SchemaError::RootEnumNameMismatch {
            expected: "Output".to_owned(),
            found: "Reply".to_owned(),
        }
    );
}
