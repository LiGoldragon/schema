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

use nota_next::StructureShape;
use schema_next::{
    DeclarativeMacroLibrary, MacroContext, MacroPosition, Name, SchemaEngine, SchemaError,
    SchemaIdentity, TypeDeclaration,
};

/// Illustrates: a schema document is positional — exactly 4 root
/// objects (Imports, Input, Output, Namespace). Any other count is
/// a typed error, not silent truncation or zero-fill.
///
/// Intent record 805 (Maximum) names the four-position root.
#[test]
fn design_example_schema_document_has_exactly_four_root_objects() {
    let too_few = "{} (Input ()) (Output ())";
    let error = SchemaEngine::default()
        .lower_source(too_few, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("three root objects should fail");
    assert_eq!(
        error,
        SchemaError::ExpectedRootObjectCount {
            expected: 4,
            found: 3,
        }
    );

    let too_many = "{} (Input ()) (Output ()) {} {}";
    let error = SchemaEngine::default()
        .lower_source(too_many, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("five root objects should fail");
    assert_eq!(
        error,
        SchemaError::ExpectedRootObjectCount {
            expected: 4,
            found: 5,
        }
    );
}

/// Illustrates: a namespace brace is a key/value MAP — odd positions
/// are PascalCase keys (type names), even positions are type bodies.
/// Per intent record 894 (Maximum): brace IS key/value at the NOTA
/// layer; the schema namespace at position 3 uses pair-style, not
/// named-object form.
///
/// This is the positive complement of
/// `brace_namespace_rejects_parenthesized_named_objects` in
/// `lowering.rs` — that test PROVES the rejection; this test PROVES
/// the pair-style positive path.
#[test]
fn design_example_namespace_brace_is_pair_style_key_value_map() {
    let source = "{} (Input ()) (Output ()) { Topic [Text] Kind (Decision Constraint) }";
    let asschema = SchemaEngine::default()
        .lower_source(source, SchemaIdentity::new("example", "0.1.0"))
        .expect("pair-style namespace lowers");

    let names: Vec<&str> = asschema
        .namespace()
        .iter()
        .map(|declaration| declaration.name().as_str())
        .collect();
    assert_eq!(names, vec!["Topic", "Kind"]);

    let TypeDeclaration::Newtype(topic) = &asschema.namespace()[0] else {
        panic!("Topic [Text] should lower as a newtype (single-field struct)");
    };
    assert_eq!(topic.fields.len(), 1);
    let TypeDeclaration::Enum(kind) = &asschema.namespace()[1] else {
        panic!("Kind (Decision Constraint) should lower as an enum");
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
    // matches one declaration and observe the recorded binding names.
    let source = "{} (Input ()) (Output ()) { Entry [Topic Description] }";
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

/// Illustrates: the default `SchemaEngine` registers two macro
/// layers — Rust-hand-coded for the four ROOT positions
/// (RootImports, RootInput, RootOutput, RootNamespace) plus
/// declarative-from-`builtin-macros.schema` for the four INNER
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
            "SchemaEnumDefinitionBrace",
            "SchemaStructFields",
            "SchemaEnumVariants",
        ],
        "five declarative macros loaded from builtin-macros.schema (paren + brace enum forms)",
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
            MacroPosition::NamespaceDeclaration,
            MacroPosition::StructFields,
            MacroPosition::EnumVariants,
        ],
        "declarative macros target the three INNER positions",
    );

    // The four ROOT positions are not in the declarative set — they
    // are hand-coded Rust macros registered in
    // `MacroRegistry::with_schema_defaults`. Observed indirectly:
    // when the default engine processes a schema, all four ROOT
    // macro names appear in the applied trace.
    let source = "{} (Input ()) (Output ()) {}";
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
    for root_macro in ["RootImports", "RootInput", "RootOutput", "RootNamespace"] {
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
    let source = "{} (Input ((Record Entry))) (Output (Accepted)) { Entry [Description] }";
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
            (StructureShape::Document, 4),
            (StructureShape::Brace, 0),
            (StructureShape::Parenthesis, 2),
            (StructureShape::Atom, 0),
            (StructureShape::Parenthesis, 1),
            (StructureShape::Parenthesis, 2),
            (StructureShape::Atom, 0),
            (StructureShape::Parenthesis, 1),
        ],
    );
    assert_ne!(header.packed_word(), 0, "header packs into a u64 word");
}

/// Illustrates: brace-enum sugar is still a macro dispatch, not a
/// special ad-hoc parser branch. It fires only at enum-variant
/// positions with an even pair shape; malformed brace bodies produce
/// the typed brace-pair error.
#[test]
fn design_example_brace_macro_dispatch_depends_on_position_and_pair_shape() {
    let source = "{} (Input {Record Entry Observe Query}) (Output ()) {}";
    let mut context = MacroContext::default();
    let asschema = SchemaEngine::default()
        .lower_source_with_context(
            source,
            SchemaIdentity::new("example", "0.1.0"),
            &mut context,
        )
        .expect("brace enum sugar lowers in root input position");

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
    assert_eq!(
        variants,
        vec![("Record", Some("Entry")), ("Observe", Some("Query"))],
    );
    assert!(
        context
            .macros_applied()
            .iter()
            .any(|name| name == "BraceEnumVariants"),
        "brace enum sugar is witnessed through macro context",
    );

    let malformed = "{} (Input {Record Entry Observe}) (Output ()) {}";
    let error = SchemaEngine::default()
        .lower_source(malformed, SchemaIdentity::new("example", "0.1.0"))
        .expect_err("odd brace count is not a valid enum-pair macro input");
    assert_eq!(error, SchemaError::ExpectedEvenBraceEnumPairs { found: 3 });
}

/// Illustrates: root enum payloads can be written in direct variant
/// form or in nested enum-body form. Both lower to the same assembled
/// schema, which keeps the authored shorthand separate from the
/// macro-free endpoint.
#[test]
fn design_example_root_enum_accepts_direct_and_nested_variant_shapes() {
    let direct = "{} (Input (Record Entry) Drop) (Output ()) {}";
    let nested = "{} (Input ((Record Entry) Drop)) (Output ()) {}";

    let direct_schema = SchemaEngine::default()
        .lower_source(direct, SchemaIdentity::new("example", "0.1.0"))
        .expect("direct variants lower");
    let nested_schema = SchemaEngine::default()
        .lower_source(nested, SchemaIdentity::new("example", "0.1.0"))
        .expect("nested enum body lowers");

    assert_eq!(direct_schema.input(), nested_schema.input());
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
        {}
        (Input {Record Entry Observe Query})
        (Output {RecordAccepted RecordIdentifier RecordsObserved RecordSet})
        {
          NexusAction {Record Entry Observe Query}
          NexusResult {Accepted RecordIdentifier Observed RecordSet}
          SemaCommand {Record Entry Observe Query}
          SemaResponse {Recorded RecordIdentifier Observed RecordSet}
          Topic [Text]
          RecordIdentifier [Integer]
          Entry [Topic]
          Query [Topic]
          RecordSet [Entry]
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
    for plane_type in ["NexusAction", "NexusResult", "SemaCommand", "SemaResponse"] {
        assert!(
            names.contains(&plane_type),
            "{plane_type} is declared as schema data, not a hidden runtime enum",
        );
    }
}
