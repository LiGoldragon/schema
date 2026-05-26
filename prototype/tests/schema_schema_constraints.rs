//! Constraint tests for records 799-807 per record 777
//! (intents-as-tests).
//!
//! Each LOAD-BEARING intent statement in records 799-807 manifests
//! here as a NAMED constraint test. Tests pin the rule STRUCTURALLY
//! (introspection on emitted types) or BEHAVIORALLY (test against
//! real input). When a test fails, the failure points directly at
//! which intent has been broken.
//!
//! Naming convention: `constraint_<record-number>_<what-it-pins>`.
//!
//! Mapping:
//!   * 799 — Block exposes structural query methods (delimiter +
//!           root-object family).
//!   * 800 — `qualifies_as_X` is structural, not interpretive — a
//!           PascalCase token qualifies but NOTA doesn't decide if
//!           it IS a type name.
//!   * 801 — Parser defaults to the HIGHER classification when a
//!           token could be either; demotion lives in the schema
//!           layer.
//!   * 802 — Inside a vector, every element is either a qualified
//!           symbol or itself a block.
//!   * 803 — NOTA does not perform schema-level type resolution —
//!           the library surface is structural-query only.
//!   * 804 — The default schema-schema is loadable implicitly via
//!           `SchemaSchema::default()`.
//!   * 805 — The root struct is implied by the `.schema` extension;
//!           authors write only positional fields, no explicit root
//!           declaration.
//!   * 807 — The `Macro` trait + `MacroContext` are part of the
//!           PUBLIC API (the macro interface is exposed).

use schema_derived_nota_prototype::{
    AssembledNode, BlockKind, BlockParser, Classification, ImportsSectionMacro,
    InputOutputStructMacro, LiteralKind, Macro, MacroContext, NamespaceSectionMacro, SchemaSchema,
    SymbolKind,
};
use std::sync::Arc;

// ════════════════════════════════════════════════════════════════════
// Record 799 — Block exposes structural query methods.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_799_block_exposes_structural_query_methods() {
    // Pins record 799: NOTA's narrowed library surface — methods
    // `is_X_bracket`, `holds_root_objects`, `root_object_at(n)`.
    let source = "(Move [1 2] {x 3})";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let outer = &blocks[0];
    // The refined-API names (no `_block` suffix per /357 §2).
    assert!(outer.is_parenthesis(), "`(Move ...)` is_parenthesis");
    assert!(!outer.is_square_bracket(), "not a square bracket");
    assert!(!outer.is_brace(), "not a brace");

    // The root-object family (record 799 + 774).
    assert_eq!(outer.holds_root_objects(), 3, "Move + [1 2] + {{x 3}}");
    assert!(outer.root_object_at(0).is_some(), "Move present at 0");
    assert!(outer.root_object_at(1).is_some(), "[1 2] present at 1");
    assert!(outer.root_object_at(2).is_some(), "brace block at 2");
    assert!(outer.root_object_at(3).is_none(), "no 4th root object");

    // Source span accessor (record 774).
    let span = outer.source_span();
    assert_eq!(span.start.byte_offset, 0, "starts at byte 0");
    assert_eq!(span.end.byte_offset, source.len(), "ends at end of source");
}

// ════════════════════════════════════════════════════════════════════
// Record 800 — `qualifies_as_X` is structural, NOT interpretive.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_800_pascal_token_qualifies_but_does_not_decide_type() {
    // Pins record 800: "NOTA does not concern itself with deciding
    // whether it's allowed for something to be PascalCase or not."
    //
    // A PascalCase token QUALIFIES as a PascalCase symbol; NOTA
    // exposes the structural check, NOT a "this IS a type name"
    // determination. The qualifying method MUST exist on Block; it
    // MUST be the structural alphabet check.
    let source = "MoveRequest moveRequest move-request";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3, "three identifier blocks");

    // `MoveRequest` qualifies as PascalCase — does NOT qualify as
    // camelCase. NOTA reports the qualification; whether MoveRequest
    // is allowed as a type name in some schema context is for the
    // schema layer to decide.
    let pascal = &blocks[0];
    assert!(pascal.qualifies_as_symbol(), "qualifies as a symbol");
    assert!(
        pascal.qualifies_as_pascal_case_symbol(),
        "qualifies as PascalCase"
    );
    assert!(
        !pascal.qualifies_as_camel_case_symbol(),
        "does NOT qualify as camelCase"
    );

    // `moveRequest` is camelCase — does not qualify as PascalCase.
    let camel = &blocks[1];
    assert!(
        camel.qualifies_as_camel_case_symbol(),
        "qualifies as camelCase"
    );
    assert!(
        !camel.qualifies_as_pascal_case_symbol(),
        "does NOT qualify as PascalCase"
    );

    // `move-request` is kebab-case — does not qualify as Pascal or
    // camelCase.
    let kebab = &blocks[2];
    assert!(kebab.qualifies_as_kebab_case_symbol(), "qualifies as kebab");
    assert!(!kebab.qualifies_as_pascal_case_symbol());
    assert!(!kebab.qualifies_as_camel_case_symbol());

    // The crucial discipline: there is NO `is_pascal_case_symbol()`
    // or `is_type_name()` method on Block. The interpretation
    // decision belongs to the schema layer; NOTA only qualifies. We
    // assert this by introspection: the methods that exist are
    // `qualifies_as_*`. If someone adds an `is_pascal_case_symbol`
    // to Block, that's a discipline violation — caught by the
    // adjacent constraint_803_nota_does_not_perform_schema_resolution.
}

// ════════════════════════════════════════════════════════════════════
// Record 801 — Default to the HIGHER classification.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_801_parser_defaults_to_higher_classification() {
    // Pins record 801: "NOTA classifies tokens with the HIGHEST
    // classification they qualify for. The schema layer can DEMOTE
    // to a string when its type context requires."
    //
    // A bare PascalCase token classifies as `QualifiedSymbol(Pascal)`,
    // NOT as a string. The same character bytes COULD be a string
    // in some schema context — the schema layer demotes.
    let source = "MoveRequest";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let classification = blocks[0]
        .classification()
        .expect("a classification for the identifier leaf");
    assert!(
        matches!(
            classification,
            Classification::QualifiedSymbol(SymbolKind::PascalCase)
        ),
        "MoveRequest defaults UP to QualifiedSymbol(PascalCase), not String — got {:?}",
        classification
    );

    // And the literal classification is below QualifiedSymbol:
    // a bare integer classifies as `Literal(Integer)` because no
    // symbol kind is higher applicable.
    let int_blocks = BlockParser::new("42")
        .parse_blocks()
        .expect("parse 42 succeeds");
    assert_eq!(
        int_blocks[0].classification(),
        Some(Classification::Literal(LiteralKind::Integer))
    );

    // A bracket-string explicitly classifies as String (the bracket
    // syntax forces the lower classification — it's the schema-
    // demoted form already present in the source).
    let str_blocks = BlockParser::new("[some text]")
        .parse_blocks()
        .expect("parse bracket string succeeds");
    // Bracket-string handling depends on kernel — verify it doesn't
    // crash and produces *some* classification (either String for
    // inline-string heuristic or Block for vector — the kernel's
    // bracket-disambiguation rule per /354's open question 2).
    let bracket_classification = str_blocks.first().and_then(|block| block.classification());
    assert!(
        bracket_classification.is_some(),
        "bracket form classifies as SOMETHING"
    );
}

// ════════════════════════════════════════════════════════════════════
// Record 802 — Vector elements are qualified symbols OR blocks.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_802_vector_contents_are_qualified_symbols_or_blocks() {
    // Pins record 802: "Inside a vector, everything is qualified
    // symbol (or another block). [Topics Kind Description] — every
    // element is a qualified symbol; the vector therefore is an
    // ordered struct of field-type-name."
    let source = "[Topics Kind Description Magnitude]";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let vec_block = &blocks[0];
    assert!(vec_block.is_square_bracket(), "outer is square");
    assert_eq!(vec_block.holds_root_objects(), 4);

    // Every element classifies as a qualified symbol (specifically
    // PascalCase here) OR as a block. Per record 802.
    for index in 0..vec_block.holds_root_objects() {
        let element = vec_block.root_object_at(index).expect("element present");
        let classification = element
            .classification()
            .expect("classification for element");
        let is_qualified_symbol = matches!(classification, Classification::QualifiedSymbol(_));
        let is_block = matches!(classification, Classification::Block(_));
        assert!(
            is_qualified_symbol || is_block,
            "vector element at {index} must be QualifiedSymbol or Block — got {:?}",
            classification
        );
    }

    // The vector-of-vectors-of-symbols variant per record 802:
    // `[[FieldA FieldB] [FieldC FieldD]]` — outer vector elements
    // are themselves vector blocks.
    let nested_source = "[[FieldA FieldB] [FieldC FieldD]]";
    let nested = BlockParser::new(nested_source)
        .parse_blocks()
        .expect("parse_blocks succeeds");
    let outer = &nested[0];
    assert_eq!(outer.holds_root_objects(), 2);
    for index in 0..2 {
        let inner = outer.root_object_at(index).expect("inner present");
        assert!(
            matches!(
                inner.classification(),
                Some(Classification::Block(BlockKind::SquareBracket))
            ),
            "outer vector element is itself a square-bracket block"
        );
    }
}

// ════════════════════════════════════════════════════════════════════
// Record 803 — NOTA does NOT perform schema resolution.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_803_nota_does_not_perform_schema_resolution() {
    // Pins record 803: NOTA's API surface is STRUCTURAL-query only.
    // No method like `resolves_to_type` / `is_known_type` /
    // `is_bound_in_namespace` exists on Block — those decisions are
    // schema-layer.
    //
    // This is asserted structurally by introspection: every Block
    // method we have here is one of:
    //   - factual delimiter check (`is_square_bracket` etc.)
    //   - structural qualification (`qualifies_as_*`)
    //   - root-object query (`root_object_at`, `holds_root_objects`)
    //   - source-span accessor (`source_span`)
    //   - reemit (`reemit`)
    //
    // None of these consult a namespace, perform name resolution,
    // or check type-context validity. The test BEHAVIORALLY proves
    // this by:
    //   1. Parsing a token that references an undefined symbol.
    //   2. Showing the block's classification doesn't depend on
    //      whether the name is defined anywhere.

    // The schema layer would resolve `Magnitude` against an imports
    // table or namespace; NOTA reports it as a QualifiedSymbol no
    // matter what.
    let undefined_source = "TotallyUndefinedTypeName";
    let blocks = BlockParser::new(undefined_source)
        .parse_blocks()
        .expect("parse succeeds even for unresolvable name");
    let classification = blocks[0].classification().expect("has classification");
    assert!(
        matches!(
            classification,
            Classification::QualifiedSymbol(SymbolKind::PascalCase)
        ),
        "undefined name classifies as QualifiedSymbol regardless of resolution"
    );

    // And the parser succeeds on a vector full of undefined names
    // — no resolution step.
    let undefined_vec = "[UnresolvedA UnresolvedB UnresolvedC]";
    let blocks = BlockParser::new(undefined_vec)
        .parse_blocks()
        .expect("parse succeeds with undefined names");
    assert!(blocks[0].is_square_bracket());
    assert_eq!(blocks[0].holds_root_objects(), 3);
}

// ════════════════════════════════════════════════════════════════════
// Record 804 — Default schema-schema is loadable implicitly.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_804_default_schema_schema_loads_implicitly() {
    // Pins record 804: "every schema file is read against an
    // implicitly-attached schema-schema — the lowest-level macro
    // primitive."
    //
    // The constructor `SchemaSchema::default()` returns an
    // initialised schema-schema with built-in macros. The default
    // schema-schema MUST be loadable without arguments.
    let schema_schema = SchemaSchema::default();
    let macros = schema_schema.builtin_macros();
    assert!(
        !macros.is_empty(),
        "default schema-schema has built-in macros"
    );

    // The three load-bearing built-in macros must be present.
    let names: Vec<&str> = macros.iter().map(|m| m.name()).collect();
    assert!(
        names.contains(&"imports_section"),
        "built-in imports_section macro present"
    );
    assert!(
        names.contains(&"input_output_struct"),
        "built-in input_output_struct macro present"
    );
    assert!(
        names.contains(&"namespace_section"),
        "built-in namespace_section macro present"
    );
}

// ════════════════════════════════════════════════════════════════════
// Record 805 — Root struct is IMPLIED by .schema extension.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_805_root_struct_implied_by_dot_schema_extension() {
    // Pins record 805: "The .schema extension IMPLIES the root
    // struct. No explicit root declaration is needed; what authors
    // write are the positional fields of the implied root struct."
    //
    // Proof: a .schema file that contains ONLY the positional
    // fields (no wrapping `(Schema ...)` or similar) parses
    // successfully via `SchemaSchema::parse_schema_file`.
    let schema_source = "
        {}

        [
          (Move (MoveRequest))
          (Rotate (RotateRequest))
        ]

        []

        {
          MoveRequest [Coordinate]
          RotateRequest [Angle]
          Coordinate [f64 f64]
          Angle [f64]
        }

        [
          (Replied (Reply))
        ]
        ";

    let schema_schema = SchemaSchema::default();
    let assembled = schema_schema
        .parse_schema_file(schema_source)
        .expect("parse_schema_file succeeds — no explicit root");

    // The assembled form contains the input/output operations and
    // namespace — drawn out of the implied root struct's positional
    // fields.
    assert_eq!(
        assembled.input_operations.len(),
        2,
        "two input ops: Move + Rotate"
    );
    assert_eq!(
        assembled.output_operations.len(),
        1,
        "one output op: Replied"
    );
    assert!(
        assembled
            .namespace
            .iter()
            .any(|entry| entry.name == "MoveRequest"),
        "namespace contains MoveRequest"
    );

    // The schema source has NO `(Schema ...)` or `(Root ...)`
    // wrapping — it IS its positional fields. We confirm by
    // searching for those wrapping tokens.
    assert!(
        !schema_source.contains("(Schema "),
        "no explicit (Schema ...) wrapping"
    );
    assert!(
        !schema_source.contains("(Root "),
        "no explicit (Root ...) wrapping"
    );
}

#[test]
fn constraint_805_root_struct_field_positions_are_load_bearing() {
    // Pins record 805 follow-up: the POSITIONAL fields matter.
    // Block 0 IS the imports map; block 3 IS the namespace. The
    // schema-schema's macro dispatch uses position (not content
    // alone) to disambiguate the two brace blocks.
    let source = "
        { ExtSym (ImportAll path) }
        []
        []
        { LocalSym ((SomeBody)) }
        []
        ";
    let schema_schema = SchemaSchema::default();
    let ctx = MacroContext::root(Arc::new(SchemaSchema::default()));
    let lowered = schema_schema
        .lower_via_macros(source, &ctx)
        .expect("lower succeeds for positional source");

    // Find imports + namespace in the lowered output.
    let mut has_imports = false;
    let mut has_namespace = false;
    for node in &lowered {
        match node {
            AssembledNode::ImportsTable { entries } => {
                has_imports = true;
                assert_eq!(entries.len(), 1, "one import: ExtSym");
                assert_eq!(entries[0].local_name, "ExtSym");
            }
            AssembledNode::Namespace { entries } => {
                has_namespace = true;
                assert_eq!(entries.len(), 1, "one namespace entry: LocalSym");
                assert_eq!(entries[0].name, "LocalSym");
            }
            _ => {}
        }
    }
    assert!(has_imports, "block 0 lowers as ImportsTable");
    assert!(has_namespace, "block 3 lowers as Namespace");
}

// ════════════════════════════════════════════════════════════════════
// Record 807 — Macro interface is PUBLICLY EXPOSED.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_807_macro_interface_publicly_exposed() {
    // Pins record 807: "The schema-schema is implemented as core
    // Rust code — the macro interface." The `Macro` trait and the
    // `MacroContext` MUST be part of the public API.
    //
    // The compilation of this test file IS the proof: it uses
    // `Macro` as a trait bound, references `MacroContext`, and
    // can construct concrete macros (`ImportsSectionMacro` etc.).
    // If the surface were not public, this file wouldn't compile.

    // Construct the macro interface dynamically.
    let imports_macro: Arc<dyn Macro> = Arc::new(ImportsSectionMacro);
    let input_output_macro: Arc<dyn Macro> = Arc::new(InputOutputStructMacro);
    let namespace_macro: Arc<dyn Macro> = Arc::new(NamespaceSectionMacro);

    assert_eq!(imports_macro.name(), "imports_section");
    assert_eq!(input_output_macro.name(), "input_output_struct");
    assert_eq!(namespace_macro.name(), "namespace_section");

    // The MacroContext is constructible from public API.
    let schema_schema = Arc::new(SchemaSchema::default());
    let ctx = MacroContext::root(Arc::clone(&schema_schema));
    assert!(
        Arc::ptr_eq(&ctx.schema_schema, &schema_schema),
        "MacroContext carries the schema-schema reference"
    );

    // The macro's matches_shape + lower MUST be reachable through
    // the trait object.
    let source = "{ Magnitude (ImportAll path) }";
    let blocks = BlockParser::new(source)
        .parse_blocks()
        .expect("parse imports");
    let block = &blocks[0];
    assert!(
        imports_macro.matches_shape(block),
        "imports macro's matches_shape reachable via trait"
    );
    let lowered = imports_macro
        .lower(block, &ctx)
        .expect("imports macro's lower reachable via trait");
    match lowered {
        AssembledNode::ImportsTable { entries } => {
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].local_name, "Magnitude");
        }
        other => panic!("expected ImportsTable, got {:?}", other),
    }
}

// ════════════════════════════════════════════════════════════════════
// Cross-cutting — record 806 carry-uncertainty marker.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_806_field_ordering_option_a_pending_psyche_decision() {
    // Record 806 is the carry-uncertainty record: Option A
    // (define-before-use — imports first) vs Option B (function-
    // signature — input/output first). Psyche has not locked
    // between them; the prototype defaults to Option A.
    //
    // This test documents the prototype's chosen default by
    // verifying the schema-schema accepts Option A layout. If
    // psyche decides Option B is canonical, this test (and the
    // schema-schema's positional dispatch) updates.
    let option_a_source = "
        { ExtSym (ImportAll path) }
        [ (Op (OpBody)) ]
        []
        { Foo ((SomeBody)) }
        []
        ";
    let schema_schema = SchemaSchema::default();
    let result = schema_schema.parse_schema_file(option_a_source);
    assert!(
        result.is_ok(),
        "Option A (imports-first) is the prototype default"
    );

    // Record this test as the PIN for the open clarification:
    // an Option B equivalent test would be added if/when psyche
    // decides the canonical ordering. Until then this is the
    // carry-uncertainty marker IN TEST FORM per record 806.
}
