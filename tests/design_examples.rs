//! Design-illustrating tests for the schema-next stack.
//!
//! Each test illustrates ONE load-bearing design point with a short
//! fixture and a focused assertion. Test names start with
//! `design_example_` so a reader scanning the file knows which tests
//! are for design representation vs broader coverage.
//!
//! Companion to `tests/lowering.rs` (the broader test surface). When
//! a design report cites a test, the test in this file should be the
//! canonical example.

use nota_next::{Document, StructureShape};
use schema_next::{
    DeclarativeMacroLibrary, MacroContext, MacroDispatch, MacroPosition, MacroRegistry, Name,
    SchemaEngine, SchemaError, SchemaIdentity, SchemaNode, SchemaNodeData, SchemaNodeValue,
    TypeDeclaration, TypeReference,
};

/// Illustrates: a schema document is positional. The common no-import
/// form has exactly 3 root values (input enum body, output enum body,
/// namespace). A leading import map makes the 4-root form.
#[test]
fn design_example_schema_document_has_three_roots_or_four_with_imports() {
    let too_few = "[] []";
    let error = SchemaEngine::default()
        .lower_source(too_few, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("two root objects should fail");
    assert_eq!(
        error,
        SchemaError::ExpectedRootObjectCount {
            expected: "3 root values (input output namespace) or 4 with leading imports",
            found: 2,
        }
    );

    let too_many = "{} [] [] {} {}";
    let error = SchemaEngine::default()
        .lower_source(too_many, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("five root objects should fail");
    assert_eq!(
        error,
        SchemaError::ExpectedRootObjectCount {
            expected: "3 root values (input output namespace) or 4 with leading imports",
            found: 5,
        }
    );

    SchemaEngine::default()
        .lower_source("[] [] {}", SchemaIdentity::new("example", "0.1.0"))
        .expect("three-root no-import schema lowers");
    SchemaEngine::default()
        .lower_source("{} [] [] {}", SchemaIdentity::new("example", "0.1.0"))
        .expect("four-root import schema lowers");
}

/// Illustrates: the schema namespace is an honest brace key/value map.
/// Each declaration is two objects: the type name key and the definition
/// value. The declaration no longer repeats its name inside a self-named
/// `Name@Delimiter` object.
///
/// This is the positive complement of
/// `brace_namespace_rejects_parenthesized_named_objects` in
/// `lowering.rs` — that test PROVES the rejection; this test PROVES
/// the pair-style positive path.
#[test]
fn design_example_namespace_brace_contains_key_value_declarations() {
    let source = "[] [] { Topic String Kind [Decision Constraint] }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("key/value namespace lowers");

    let names: Vec<&str> = asschema
        .namespace()
        .iter()
        .map(|declaration| declaration.name().as_str())
        .collect();
    assert_eq!(names, vec!["Topic", "Kind"]);

    let TypeDeclaration::Newtype(topic) = asschema.namespace()[0].value() else {
        panic!("Topic should lower as a newtype (single-field struct)");
    };
    assert_eq!(topic.reference, TypeReference::String);
    let TypeDeclaration::Enum(kind) = asschema.namespace()[1].value() else {
        panic!("Kind should lower as an enum");
    };
    let variant_names: Vec<&str> = kind
        .variants
        .iter()
        .map(|variant| variant.name.as_str())
        .collect();
    assert_eq!(variant_names, vec!["Decision", "Constraint"]);
}

/// Illustrates: a declarative `SchemaMacro` declaration uses `$Name`
/// for single captures and `$*Name` for rest captures, AND those
/// names flow through to the macro context as `MacroName::Name` and
/// `MacroName::*Name` bindings when the macro fires.
///
/// Intent record 890 (Medium): macro bodies need an explicit binding
/// and reference mechanism for assigned symbols; a sigil such as
/// dollar sign is the candidate. This test pins the dollar-sigil
/// shape in working code.
#[test]
fn design_example_macro_captures_use_dollar_and_dollar_star_sigils() {
    let library = DeclarativeMacroLibrary::builtin().expect("builtin macros parse");

    let struct_definition = library
        .definitions()
        .iter()
        .find(|definition| definition.name().as_str() == "SchemaStructDefinition")
        .expect("struct macro definition");
    assert_eq!(struct_definition.capture_names(), vec!["$Name", "$*Fields"]);

    // The captures FIRE — feed a minimal schema where the struct macro
    // matches one legacy declaration and observe the recorded binding
    // names. The production namespace path now uses key/value pairs;
    // this test is specifically about the declarative macro library.
    let source = "Input@[] Output@[] { Entry@{ topic@Topic description@Description } }";
    let mut context = MacroContext::default();
    SchemaEngine::default()
        .lower_source_with_context(
            source,
            SchemaIdentity::new("example", "0.1.0"),
            &mut context,
        )
        .expect("schema lowers");

    let bindings = context.bindings_seen();
    assert!(
        bindings
            .iter()
            .any(|binding| binding == "SchemaStructDefinition::Name"),
        "single capture $Name binds as Name",
    );
    assert!(
        bindings
            .iter()
            .any(|binding| binding == "SchemaStructDefinition::*Fields"),
        "rest capture $*Fields binds as *Fields",
    );
}

/// Illustrates: a colon-qualified name like `schema:spirit:Entry`
/// decomposes into ordered segments by single-colon, and `local_part`
/// returns the final segment.
///
/// Intent records 895 + 902 (Maximum / High): namespace separator is
/// a SINGLE colon mirroring Rust crate:module:Type structure (not
/// Rust's double-colon).
///
/// Focused complement of `colon_qualified_names_lower_as_schema_names`
/// in `lowering.rs` — that test exercises colon names through a full
/// lowering; this one isolates the `Name` decomposition method
/// without parsing a schema.
#[test]
fn design_example_colon_qualified_name_decomposes_into_segments() {
    let qualified = Name::new("schema:spirit:Entry");

    assert_eq!(
        qualified.namespace_segments(),
        vec!["schema", "spirit", "Entry"]
    );
    assert_eq!(qualified.local_part(), "Entry");
    assert_eq!(qualified.field_name(), "entry");

    let bare = Name::new("Topic");
    assert_eq!(bare.namespace_segments(), vec!["Topic"]);
    assert_eq!(bare.local_part(), "Topic");
    assert_eq!(bare.field_name(), "topic");
}

/// Illustrates: the default `SchemaEngine` registers macro layers:
/// Rust-hand-coded for the root positions and type-reference tagged calls
/// (RootImports, RootInput, RootOutput, RootNamespace) plus
/// declarative-from-`builtin-macros.schema` for the inner structural
/// positions (NamespaceDeclaration / StructFields / EnumVariants —
/// the SchemaStructDefinition / SchemaEnumDefinition /
/// SchemaStructFields / SchemaEnumVariants library).
///
/// Intent record 864 (Maximum): real macro registry / macro-dispatch
/// design. This test asserts the layered shape from outside the
/// engine — no Spirit fixture needed.
#[test]
fn design_example_default_engine_has_two_macro_layers() {
    let library = DeclarativeMacroLibrary::builtin().expect("builtin macros parse");
    let declarative_names: Vec<&str> = library
        .definitions()
        .iter()
        .map(|definition| definition.name().as_str())
        .collect();
    assert_eq!(
        declarative_names,
        vec![
            "SchemaStructDefinition",
            "SchemaEnumDefinition",
            "SchemaStructFields",
            "SchemaEnumVariants",
        ],
        "declarative structural macros loaded from builtin-macros.schema",
    );

    let positions: Vec<MacroPosition> = library
        .definitions()
        .iter()
        .map(|definition| definition.position())
        .collect();
    assert_eq!(
        positions,
        vec![
            MacroPosition::NamespaceDeclaration,
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::EnumVariants,
        ],
        "declarative macros target the structural inner positions",
    );

    // The four ROOT positions are not in the declarative set — they
    // are hand-coded Rust macros registered in
    // `MacroRegistry::with_schema_defaults`. Observed indirectly:
    // when the default engine processes a schema, all four ROOT
    // macro names appear in the applied trace.
    let source = "[] [] {}";
    let mut context = MacroContext::default();
    SchemaEngine::default()
        .lower_source_with_context(
            source,
            SchemaIdentity::new("example", "0.1.0"),
            &mut context,
        )
        .expect("schema lowers");
    let applied: Vec<&str> = context
        .macros_applied()
        .iter()
        .map(String::as_str)
        .collect();
    for root_macro in ["RootInput", "RootOutput", "RootNamespace"] {
        assert!(
            applied.contains(&root_macro),
            "root macro {root_macro} fires on a minimal schema; applied = {applied:?}",
        );
    }
}

/// Illustrates: the schema engine consumes the NOTA first-pass
/// structure header. The header is recorded before semantic macro
/// lowering so macro dispatch can be tested against the same compact
/// first-two-level shape witness that will later feed signal-style
/// triage.
#[test]
fn design_example_schema_lowering_records_source_structure_header() {
    let source = "[Record@ Entry] [Accepted] { Entry { description Description } }";
    let mut context = MacroContext::default();
    SchemaEngine::default()
        .lower_source_with_context(
            source,
            SchemaIdentity::new("example", "0.1.0"),
            &mut context,
        )
        .expect("schema lowers");

    let header = context
        .structure_headers()
        .first()
        .expect("schema lowering records the source structure header");
    let observed: Vec<(StructureShape, u8)> = header
        .slots()
        .iter()
        .map(|slot| (slot.shape(), slot.child_count()))
        .collect();

    assert_eq!(
        observed,
        vec![
            (StructureShape::Document, 3),
            (StructureShape::SquareBracket, 2),
            (StructureShape::Atom, 0),
            (StructureShape::Atom, 0),
            (StructureShape::SquareBracket, 1),
            (StructureShape::Atom, 0),
            (StructureShape::Brace, 2),
            (StructureShape::Unknown, 15),
        ],
    );
    assert_ne!(header.packed_word(), 0, "header packs into a u64 word");
}

/// Illustrates: macro expectations live on node definitions. Structural
/// macros are expected at namespace/fields/variants positions; native
/// structure or tagged user macro invocations are expected at
/// type-reference positions.
#[test]
fn design_example_macro_node_definitions_separate_structural_from_tagged_invocation() {
    let registry = MacroRegistry::with_schema_defaults();
    let dispatches: Vec<(MacroPosition, MacroDispatch)> = registry
        .node_definitions()
        .iter()
        .map(|definition| (definition.position(), definition.dispatch()))
        .collect();
    assert_eq!(
        dispatches,
        vec![
            (MacroPosition::RootImports, MacroDispatch::RootPositional),
            (MacroPosition::RootInput, MacroDispatch::RootPositional),
            (MacroPosition::RootOutput, MacroDispatch::RootPositional),
            (MacroPosition::RootNamespace, MacroDispatch::RootPositional),
            (
                MacroPosition::NamespaceDeclaration,
                MacroDispatch::Structural
            ),
            (MacroPosition::StructFields, MacroDispatch::Structural),
            (MacroPosition::EnumVariants, MacroDispatch::Structural),
            (
                MacroPosition::TypeReference,
                MacroDispatch::StructuralOrTaggedInvocation
            ),
        ],
    );
}

/// Illustrates: a schema-node macro call is data. `(Normalize [Topic])`
/// parses as a tagged node named `Normalize` carrying a vector data payload
/// containing the symbol `Topic`. No sigil is needed because this is
/// read at a known schema-node position.
#[test]
fn design_example_schema_node_macro_call_is_tagged_data() {
    let document = Document::parse("(Normalize [Topic])").expect("nota parses");
    let node = SchemaNode::from_block(document.root_object_at(0).expect("macro node"))
        .expect("schema node parses");

    assert_eq!(node.tag().as_str(), "Normalize");
    assert_eq!(
        node.data(),
        &SchemaNodeData::Vector(vec![SchemaNodeValue::Symbol(Name::new("Topic"))])
    );
}

/// Illustrates: root enum payloads are authored directly inside the
/// known root enum body. Payload-carrying variants use `Variant@ Payload`;
/// unit variants use bare symbols.
#[test]
fn design_example_root_enum_uses_direct_variant_shapes() {
    let source = "[Record@ Entry Drop] [] {}";

    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("direct variants lower");

    let variants: Vec<(&str, Option<&str>)> = asschema
        .input()
        .variants
        .iter()
        .map(|variant| {
            (
                variant.name.as_str(),
                variant
                    .payload
                    .as_ref()
                    .map(|payload| payload.plain_name().expect("plain payload").as_str()),
            )
        })
        .collect();

    assert_eq!(variants, vec![("Record", Some("Entry")), ("Drop", None)]);
}

/// Illustrates: same-name payload variants are explicit data-carrying
/// variants. The old star suffix is gone from authored schema.
#[test]
fn design_example_same_name_payload_variant_uses_explicit_payload() {
    let source = "[Record@ Record] [Recorded@ Recorded] { Record { description Description } Recorded { recordIdentifier RecordIdentifier } }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("explicit same-name variants lower");

    assert_eq!(
        asschema.input().variants[0]
            .payload
            .as_ref()
            .expect("record payload")
            .plain_name()
            .expect("plain payload")
            .as_str(),
        "Record",
    );
    assert_eq!(
        asschema.output().variants[0]
            .payload
            .as_ref()
            .expect("recorded payload")
            .plain_name()
            .expect("plain payload")
            .as_str(),
        "Recorded",
    );
}

/// Illustrates: user-declared structural macros and tagged-invocation
/// macros are both real registry entries. Neither uses `@`: the node
/// position says whether the object is a structural definition or a
/// tagged macro call.
#[test]
fn design_example_user_declared_macros_extend_structural_and_named_slots() {
    let user_macros = DeclarativeMacroLibrary::from_source(
        "
        (SchemaMacro StringNewtype NamespaceDeclaration
          ($Name StringNewtype)
          (Type (Struct $Name [String])))
        (SchemaMacro Bag TypeReference
          (Bag $Type)
          (Reference (Vector $Type)))
        ",
    )
    .expect("user macro definitions parse");
    let mut registry = MacroRegistry::with_schema_defaults();
    for schema_macro in user_macros.into_macros() {
        registry.register_box(schema_macro);
    }
    let engine = SchemaEngine::with_registry(registry);
    let asschema = engine
        .lower_source(
            "Input@[] Output@[] { Topic@(StringNewtype) Topics@{ items@(Bag Topic) } }",
            SchemaIdentity::new("example", "0.1.0"),
        )
        .expect("schema lowers through user macros");

    let TypeDeclaration::Newtype(topic) = asschema.type_named("Topic").expect("topic type") else {
        panic!("StringNewtype macro creates a newtype");
    };
    assert_eq!(topic.reference, TypeReference::String);
    let TypeDeclaration::Newtype(topics) = asschema.type_named("Topics").expect("topics type")
    else {
        panic!("single-field Topics should be a newtype");
    };
    assert_eq!(
        topics.reference,
        TypeReference::Vector(Box::new(TypeReference::new("Topic"))),
    );
}

/// Illustrates: the same schema language names the three runtime
/// planes. Signal roots remain the schema's Input/Output, while
/// Nexus and SEMA vocabularies are ordinary schema objects in the
/// namespace until the plane-specific file split lands.
///
/// Intent records 964 and 965 rename the execution plane to Nexus
/// and classify Signal, Nexus, and SEMA as schema-driven planes.
#[test]
fn design_example_signal_nexus_and_sema_are_schema_declared_planes() {
    let source = "
        [Record@ Entry Observe@ Query]
        [RecordAccepted@ RecordIdentifier RecordsObserved@ RecordSet]
        {
          NexusInput [Signal@ Input Sema@ SemaOutput]
          NexusOutput [Sema@ SemaInput Signal@ Output]
          SemaInput [Record@ Entry Observe@ Query]
          SemaOutput [Recorded@ RecordIdentifier Observed@ RecordSet]
          Topic String
          RecordIdentifier Integer
          Entry { topic Topic }
          Query { topic Topic }
          RecordSet (Vec Entry)
        }
    ";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("spirit-next:lib", "0.1.0"))
        .expect("schema planes lower");

    assert_eq!(asschema.input().name.as_str(), "Input");
    assert_eq!(asschema.output().name.as_str(), "Output");

    let names: Vec<&str> = asschema
        .namespace()
        .iter()
        .map(|declaration| declaration.name().as_str())
        .collect();
    for plane_type in ["NexusInput", "NexusOutput", "SemaInput", "SemaOutput"] {
        assert!(
            names.contains(&plane_type),
            "{plane_type} is declared as schema data, not a hidden runtime enum",
        );
    }
}
