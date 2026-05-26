use schema::{
    ModuleName, ObjectDelimiter, SchemaBlockObject, SchemaBlockPass, SchemaMacroMatcher,
    SchemaMacroPattern, SymbolClass,
};

fn pass(text: &str) -> SchemaBlockPass {
    SchemaBlockPass::parse_text(ModuleName::new("object_block").unwrap(), text)
        .expect("block pass parses")
}

#[test]
fn block_pass_preserves_root_delimiters_and_source_spans() {
    let pass = pass(
        "
{ Topic [String] }
(Record [Entry])
",
    );

    assert_eq!(pass.root_count(), 2);

    let namespace = pass.root(0).and_then(SchemaBlockObject::as_block).unwrap();
    assert_eq!(namespace.delimiter(), ObjectDelimiter::CurlyBraces);
    assert_eq!(namespace.span().start().line(), 2);
    assert_eq!(namespace.span().start().column(), 1);
    assert_eq!(namespace.span().end().line(), 2);
    assert_eq!(namespace.span().end().column(), 19);

    let record = pass.root(1).and_then(SchemaBlockObject::as_block).unwrap();
    assert!(record.is_parenthesis_block());
    assert_eq!(record.span().start().line(), 3);
    assert_eq!(record.span().start().column(), 1);
}

#[test]
fn recursive_shape_predicates_describe_macro_inputs() {
    let pass = pass("(State [Statement Declaration])");
    let root = pass
        .single_root()
        .and_then(SchemaBlockObject::as_block)
        .expect("single parenthesized root");

    assert!(root.is_parenthesis_block());
    assert!(root.holds_two_root_objects());
    assert_eq!(
        root.object(0).and_then(SchemaBlockObject::symbol_text),
        Some("State")
    );
    assert!(root.second_object_is_square_bracket_block());

    let variants = root
        .second_object()
        .and_then(SchemaBlockObject::as_block)
        .expect("second object is vector-like block");
    assert!(variants.holds_object_count(2));
    assert_eq!(
        variants
            .objects()
            .iter()
            .map(SchemaBlockObject::symbol_text)
            .collect::<Option<Vec<_>>>(),
        Some(vec!["Statement", "Declaration"])
    );
}

#[test]
fn block_pass_keeps_block_strings_opaque() {
    let pass = pass("[|this (is not parsed) {inside} [the block string]|]");
    let root = pass
        .single_root()
        .and_then(SchemaBlockObject::as_block)
        .expect("block string root");

    assert!(root.is_square_bracket_block());
    assert!(root.is_block_string());
    assert_eq!(root.object_count(), 0);
}

#[test]
fn block_pass_reports_unbalanced_delimiters_with_location() {
    let error = SchemaBlockPass::parse_text(
        ModuleName::new("object_block").unwrap(),
        "{ Topic [String]\n  Entry [Topic]\n",
    )
    .expect_err("missing brace errors");
    let text = error.to_string();

    assert!(text.contains("missing closing `}`"));
    assert!(text.contains("line 3, column 1"));
}

#[test]
fn block_pass_skips_comments_outside_objects() {
    let pass = pass(
        "
;; first root is after a comment
{ Topic [String] } ;; trailing comment
;; another comment
[State Record]
",
    );

    assert_eq!(pass.root_count(), 2);
    assert!(
        pass.root(0)
            .is_some_and(SchemaBlockObject::is_curly_brace_block)
    );
    assert!(
        pass.root(1)
            .is_some_and(SchemaBlockObject::is_square_bracket_block)
    );
}

#[test]
fn atoms_promote_to_qualified_symbols_without_final_type_semantics() {
    let pass = pass("[Topic schema-name camelCase _internal]");
    let root = pass
        .single_root()
        .and_then(SchemaBlockObject::as_block)
        .expect("symbol vector");

    let symbols = root
        .objects()
        .iter()
        .map(SchemaBlockObject::qualified_symbol)
        .collect::<Option<Vec<_>>>()
        .expect("all objects qualify as raw symbols");

    assert_eq!(symbols[0].text(), "Topic");
    assert_eq!(symbols[0].class(), SymbolClass::PascalCase);
    assert_eq!(symbols[0].demote_to_string(), "Topic");
    assert_eq!(symbols[1].class(), SymbolClass::KebabCase);
    assert_eq!(symbols[2].class(), SymbolClass::CamelCase);
    assert_eq!(symbols[3].class(), SymbolClass::Other);
}

#[test]
fn raw_nota_does_not_reject_pascal_case_in_string_like_positions() {
    let pass = pass("[User]");
    let root = pass
        .single_root()
        .and_then(SchemaBlockObject::as_block)
        .expect("square bracket block");
    let user = root.object(0).expect("inner atom");

    assert_eq!(user.symbol_text(), Some("User"));
    assert_eq!(user.symbol_class(), Some(SymbolClass::PascalCase));
}

#[test]
fn macro_pattern_matches_parenthesized_symbol_and_vector_shape() {
    let pass = pass("(State [Statement Declaration])");
    let root = pass.single_root().expect("single root");
    let pattern = SchemaMacroPattern::parenthesized(vec![
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::square_bracketed(vec![
            SchemaMacroPattern::pascal_symbol(),
            SchemaMacroPattern::pascal_symbol(),
        ]),
    ]);

    assert!(pattern.matches(root));
}

#[test]
fn macro_pattern_can_describe_schema_struct_field_vectors() {
    let pass = pass("[Topics Kind Description Magnitude]");
    let root = pass.single_root().expect("single root");
    let struct_fields = SchemaMacroPattern::square_bracketed(vec![
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
    ]);
    let wrong_shape = SchemaMacroPattern::parenthesized(vec![
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
        SchemaMacroPattern::pascal_symbol(),
    ]);

    assert!(struct_fields.matches(root));
    assert!(!wrong_shape.matches(root));
}

#[test]
fn macro_matcher_names_the_macro_position_that_consumes_shape() {
    let pass = pass("(State [Statement Declaration])");
    let root = pass.single_root().expect("single root");
    let matcher = SchemaMacroMatcher::new(
        "HeaderRoot",
        SchemaMacroPattern::parenthesized(vec![
            SchemaMacroPattern::pascal_symbol(),
            SchemaMacroPattern::square_bracketed(vec![
                SchemaMacroPattern::pascal_symbol(),
                SchemaMacroPattern::pascal_symbol(),
            ]),
        ]),
    );

    assert_eq!(matcher.name(), "HeaderRoot");
    assert!(matcher.matches(root));
}
