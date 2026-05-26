//! Kernel-level tests — proving the bootstrap parses the small set of
//! NOTA constructs it claims to handle.

use schema_derived_nota_prototype::{Kernel, KernelTokenKind, NodeKind};

#[test]
fn lex_empty_input_yields_no_tokens() {
    let mut kernel = Kernel::new("");
    let tokens = kernel.lex().expect("empty input lexes");
    assert!(tokens.is_empty());
}

#[test]
fn lex_whitespace_only_yields_no_tokens() {
    let mut kernel = Kernel::new("   \n\t\r  ");
    let tokens = kernel.lex().expect("whitespace lexes");
    assert!(tokens.is_empty());
}

#[test]
fn lex_three_delimiter_pairs() {
    let mut kernel = Kernel::new("(){}");
    let tokens = kernel.lex().expect("delimiters lex");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            KernelTokenKind::RecordOpen,
            KernelTokenKind::RecordClose,
            KernelTokenKind::MapOpen,
            KernelTokenKind::MapClose,
        ]
    );
}

#[test]
fn lex_inline_bracket_string_with_apostrophe() {
    // `[he said 'yes']` has an apostrophe, which is a string-only byte,
    // so the kernel resolves to InlineString.
    let mut kernel = Kernel::new("[he said 'yes']");
    let tokens = kernel.lex().expect("inline string lexes");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, KernelTokenKind::InlineString);
}

#[test]
fn lex_token_shaped_brackets_resolve_to_vector() {
    // `[hello world]` looks like either a vector or a string. The
    // kernel boundary (per record 747) defaults to vector; the
    // schema layer reinterprets when the position demands String.
    let mut kernel = Kernel::new("[hello world]");
    let tokens = kernel.lex().expect("bracket form lexes");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind.clone()).collect();
    assert_eq!(kinds[0], KernelTokenKind::VectorOpen);
    assert_eq!(kinds[kinds.len() - 1], KernelTokenKind::VectorClose);
}

#[test]
fn lex_block_string() {
    let source = "[|line one\nline two|]";
    let mut kernel = Kernel::new(source);
    let tokens = kernel.lex().expect("block string lexes");
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0].kind, KernelTokenKind::BlockString);
}

#[test]
fn lex_identifier_classes() {
    let mut kernel = Kernel::new("PascalCase camelCase kebab-case");
    let tokens = kernel.lex().expect("identifiers lex");
    let texts: Vec<_> = tokens
        .iter()
        .filter(|t| t.kind == KernelTokenKind::Identifier)
        .map(|t| t.lexeme(kernel_source("PascalCase camelCase kebab-case")))
        .collect();
    assert_eq!(texts, vec!["PascalCase", "camelCase", "kebab-case"]);
}

fn kernel_source(input: &'static str) -> &'static str {
    input
}

#[test]
fn lex_integers_floats_bytes() {
    let mut kernel = Kernel::new("42 3.14 #a1b2c3");
    let tokens = kernel.lex().expect("number/bytes lex");
    let kinds: Vec<_> = tokens.iter().map(|t| t.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            KernelTokenKind::Integer,
            KernelTokenKind::Float,
            KernelTokenKind::Bytes,
        ]
    );
}

#[test]
fn lex_line_comment() {
    let mut kernel = Kernel::new(";; a comment\n42");
    let tokens = kernel.lex().expect("comment lexes");
    assert_eq!(tokens[0].kind, KernelTokenKind::LineComment);
    assert_eq!(tokens[1].kind, KernelTokenKind::Integer);
}

#[test]
fn lex_unclosed_string_errors() {
    let mut kernel = Kernel::new("[hello");
    assert!(kernel.lex().is_err());
}

#[test]
fn parse_nested_record() {
    let mut kernel = Kernel::new("(Some (3.0 4.0))");
    let node = kernel.parse_single().expect("nested record parses");
    assert!(matches!(node.kind, NodeKind::Record));
    assert_eq!(node.children.len(), 2);
    assert_eq!(node.children[0].kind, NodeKind::Identifier);
    assert_eq!(node.children[0].text, "Some");
    assert_eq!(node.children[1].kind, NodeKind::Record);
}

#[test]
fn parse_map() {
    let mut kernel = Kernel::new("{host localhost port 8080}");
    let node = kernel.parse_single().expect("map parses");
    assert!(matches!(node.kind, NodeKind::Map));
    assert_eq!(node.children.len(), 4);
}

#[test]
fn parse_vector() {
    let mut kernel = Kernel::new("[1 2 3]");
    let node = kernel.parse_single().expect("vector parses");
    assert!(matches!(node.kind, NodeKind::Vector));
    assert_eq!(node.children.len(), 3);
}

#[test]
fn parse_empty_collections() {
    let mut kernel = Kernel::new("() [] {}");
    let nodes = kernel.parse_sequence().expect("empty collections parse");
    assert_eq!(nodes.len(), 3);
    assert!(matches!(nodes[0].kind, NodeKind::Record));
    assert!(matches!(nodes[1].kind, NodeKind::Vector));
    assert!(matches!(nodes[2].kind, NodeKind::Map));
}

#[test]
fn parse_nota_schema_top_level_layout() {
    // Five top-level blocks: { } [ ] [ ] { } [ ]
    let source = include_str!("../schemas/nota.schema");
    let mut kernel = Kernel::new(source);
    let blocks = kernel.parse_sequence().expect("nota.schema parses");
    let kinds: Vec<_> = blocks.iter().map(|n| n.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            NodeKind::Map,
            NodeKind::Vector,
            NodeKind::Vector,
            NodeKind::Map,
            NodeKind::Vector,
        ],
        "nota.schema must follow the canonical five-block layout"
    );
}
