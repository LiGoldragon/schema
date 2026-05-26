use schema::{ModuleName, SchemaBlockObject, SchemaBlockPass, SchemaMacroPattern};

fn single_root(text: &str) -> SchemaBlockObject {
    SchemaBlockPass::parse_text(ModuleName::new("macro_pattern").unwrap(), text)
        .expect("block pass parses")
        .single_root()
        .expect("single root")
        .clone()
}

#[test]
fn block_matcher_recognizes_symbol_then_square_bracket_macro_shape() {
    let root = single_root("(State [Statement Declaration])");
    let endpoint_variants = [
        SchemaMacroPattern::symbol(),
        SchemaMacroPattern::square_bracketed_any(),
    ];
    let route_root = SchemaMacroPattern::parenthesized(Vec::from(endpoint_variants));

    assert!(route_root.matches(&root));
}

#[test]
fn block_matcher_rejects_symbol_then_atom_when_square_block_required() {
    let root = single_root("(State Statement)");
    let endpoint_variants = [
        SchemaMacroPattern::symbol(),
        SchemaMacroPattern::square_bracketed_any(),
    ];
    let route_root = SchemaMacroPattern::parenthesized(Vec::from(endpoint_variants));

    assert!(!route_root.matches(&root));
}

#[test]
fn symbol_classes_are_candidates_not_schema_type_decisions() {
    let pascal = single_root("State");
    let camel = single_root("state");
    let kebab = single_root("state-root");
    let bracket_string = single_root("[state root]");

    assert!(SchemaMacroPattern::pascal_symbol().matches(&pascal));
    assert!(SchemaMacroPattern::camel_symbol().matches(&camel));
    assert!(SchemaMacroPattern::kebab_symbol().matches(&kebab));

    assert!(SchemaMacroPattern::symbol().matches(&pascal));
    assert!(SchemaMacroPattern::symbol().matches(&camel));
    assert!(SchemaMacroPattern::symbol().matches(&kebab));
    assert!(!SchemaMacroPattern::symbol().matches(&bracket_string));
}
