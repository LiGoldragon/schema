//! Block-parsing constraint tests per record 777.
//!
//! Each load-bearing intent in records 774, 775, 776 manifests as a
//! NAMED constraint test below. Tests pin the rule structurally or
//! behaviorally — not principles-in-docs-only.
//!
//! Test naming convention: `constraint_<record>_<what-it-pins>`.
//! When a test fails, the failure points directly at which intent
//! has been broken.

use schema_derived_nota_prototype::{Block, BlockParser, NodeKind, SourcePosition};

// ════════════════════════════════════════════════════════════════════
// Record 774 — block-by-block parsing, source-span tracking,
//              recursive predicate methods.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_774_blocks_carry_source_spans() {
    // Pins record 774: "Each block tracks its source span (line/column
    // for both start and end on the original text)."
    let source = "(Move (Position 1 2))\n[a b c]\n{key value}";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3, "three top-level blocks");

    // First block — the (Move ...) record on line 1
    let first = &blocks[0];
    assert_eq!(
        first.span.start,
        SourcePosition {
            line: 1,
            column: 1,
            byte_offset: 0
        }
    );
    assert_eq!(first.span.end.line, 1, "first block ends on line 1");
    assert!(
        first.span.end.column > first.span.start.column,
        "end column past start"
    );

    // Second block — the [a b c] vector on line 2
    let second = &blocks[1];
    assert_eq!(second.span.start.line, 2, "second block starts on line 2");
    assert_eq!(
        second.span.start.column, 1,
        "second block starts at column 1"
    );

    // Third block — the {key value} map on line 3
    let third = &blocks[2];
    assert_eq!(third.span.start.line, 3, "third block starts on line 3");
    assert_eq!(third.span.start.column, 1, "third block starts at column 1");
}

#[test]
fn constraint_774_block_predicates_classify_correctly() {
    // Pins record 774: "is_square_bracket_block / is_parenthesis_block
    // / is_brace_block" predicates classify each block by its delimiter.
    let source = "(parens) [bracket] {brace}";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3);

    assert!(blocks[0].is_parenthesis_block(), "first is parens");
    assert!(!blocks[0].is_square_bracket_block());
    assert!(!blocks[0].is_brace_block());

    assert!(blocks[1].is_square_bracket_block(), "second is square");
    assert!(!blocks[1].is_parenthesis_block());
    assert!(!blocks[1].is_brace_block());

    assert!(blocks[2].is_brace_block(), "third is brace");
    assert!(!blocks[2].is_parenthesis_block());
    assert!(!blocks[2].is_square_bracket_block());
}

#[test]
fn constraint_774_root_object_count_methods() {
    // Pins record 774: "holds_single_root_object /
    // holds_two_root_objects / holds_N_root_objects".
    let source = "[a] [a b] [a b c d e]";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3);

    assert!(blocks[0].holds_single_root_object(), "[a] holds single");
    assert_eq!(blocks[0].holds_root_objects(), 1);

    assert!(!blocks[1].holds_single_root_object(), "[a b] doesn't");
    assert!(blocks[1].holds_two_root_objects(), "[a b] holds two");
    assert_eq!(blocks[1].holds_root_objects(), 2);

    assert_eq!(blocks[2].holds_root_objects(), 5, "[a b c d e] holds five");
    assert!(!blocks[2].holds_two_root_objects());
}

#[test]
fn constraint_774_root_objects_are_themselves_blocks_with_spans() {
    // Pins record 774 recursion clause: "each root object inside a
    // block is itself a typed/positioned block".
    let source = "(Move [1 2] {x 3})";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let outer = &blocks[0];
    assert!(outer.is_parenthesis_block());
    assert_eq!(outer.holds_root_objects(), 3, "Move + [1 2] + brace-map");

    // Each root object IS itself a typed positioned block.
    let move_token = outer.root_object_at(0).expect("first root object");
    assert!(move_token.is_leaf(), "Move is leaf identifier");
    assert_eq!(move_token.leaf_kind, Some(NodeKind::Identifier));
    assert_eq!(move_token.leaf_text, "Move");

    let position_vec = outer.root_object_at(1).expect("second root object");
    assert!(position_vec.is_square_bracket_block(), "[1 2] is square");
    assert_eq!(position_vec.holds_root_objects(), 2);

    let x_map = outer.root_object_at(2).expect("third root object");
    assert!(x_map.is_brace_block(), "x_map is brace");
    assert_eq!(x_map.holds_root_objects(), 2);

    // Each of those carries its own source span on the original
    // text — the recursion clause is satisfied at this level.
    assert!(position_vec.span.start.byte_offset > move_token.span.start.byte_offset);
    assert!(x_map.span.start.byte_offset > position_vec.span.start.byte_offset);
}

#[test]
fn constraint_774_recursive_shape_predicates_compose() {
    // Pins record 774: "recursive predicates like
    // second_root_object_is_a_square_bracket_object /
    // second_root_object_qualifies_as_a_symbol".
    //
    // Example: `(Move [x y])` — second root object is a square bracket
    // block. `(Some x)` — second root object qualifies as a symbol.
    let parser = BlockParser::new("(Move [x y])");
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    let outer = &blocks[0];
    assert!(
        outer.second_root_object_is_a_square_bracket_object(),
        "(Move [x y]) — second root is square bracket"
    );
    assert!(
        !outer.second_root_object_qualifies_as_a_symbol(),
        "second root is a vector, not a symbol"
    );

    let parser = BlockParser::new("(Some x)");
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    let outer = &blocks[0];
    assert!(
        outer.second_root_object_qualifies_as_a_symbol(),
        "(Some x) — second root is the symbol `x`"
    );
    assert!(
        !outer.second_root_object_is_a_square_bracket_object(),
        "second root is a leaf, not a square bracket block"
    );
}

// ════════════════════════════════════════════════════════════════════
// Record 775 — range-based span tracking, NOT normalization.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_775_range_based_not_normalization() {
    // Pins record 775: "Direction: range-based span tracking on the
    // original source text (line/column start + end per block), not
    // normalization."
    //
    // The proof: a multi-line block preserves its original layout in
    // its source span. The block's `reemit` returns the exact
    // substring from the original source — no whitespace collapse,
    // no line-break removal, no re-indentation.
    let source = "(Move\n  [1\n   2]\n  )";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let outer = &blocks[0];
    // The reemit IS the substring of the original source.
    let reemitted = outer.reemit(source);
    assert_eq!(
        reemitted, source,
        "outer block reemit equals the entire source — exact range, not normalized"
    );

    // Specifically: line breaks are preserved.
    assert!(reemitted.contains('\n'), "newlines preserved");
    // And so is the original indentation.
    assert!(reemitted.contains("  ["), "indentation preserved");

    // Nested block reemit also preserves layout exactly.
    let inner = outer.root_object_at(1).expect("the [1 2] block");
    assert!(inner.is_square_bracket_block());
    let inner_text = inner.reemit(source);
    assert_eq!(inner_text, "[1\n   2]", "inner block reemit is exact");
    assert!(inner_text.contains('\n'), "inner preserves newline too");
}

#[test]
fn constraint_775_spans_use_lines_and_columns() {
    // Pins record 775 + 774's span shape: "line and column spans
    // directly rather than normalizing every root object onto one
    // line".
    let source = "abc\n  (Move\n    1)";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 2);

    // First block: the `abc` identifier on line 1.
    assert_eq!(blocks[0].span.start.line, 1);
    assert_eq!(blocks[0].span.start.column, 1);
    assert_eq!(blocks[0].span.end.line, 1);
    assert_eq!(blocks[0].span.end.column, 4); // exclusive end after 'c'

    // Second block: the (Move 1) record starts on line 2, column 3
    // and ends on line 3.
    assert_eq!(blocks[1].span.start.line, 2);
    assert_eq!(blocks[1].span.start.column, 3, "after the two-space indent");
    assert_eq!(blocks[1].span.end.line, 3);
}

#[test]
fn constraint_775_byte_offset_preserved_alongside_line_column() {
    // Defensive: the implementation tracks BOTH byte offset AND
    // line/column. Either is callable; they agree.
    let source = "abc def";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 2);

    assert_eq!(blocks[0].span.start.byte_offset, 0);
    assert_eq!(blocks[0].span.end.byte_offset, 3); // exclusive after 'c'
    assert_eq!(blocks[1].span.start.byte_offset, 4); // after the space
    assert_eq!(blocks[1].span.end.byte_offset, 7); // exclusive after 'f'

    // And the line/column projection agrees with byte offset.
    assert_eq!(blocks[0].span.start.column, 1);
    assert_eq!(blocks[1].span.start.column, 5);
}

// ════════════════════════════════════════════════════════════════════
// Record 776 — reassembly is concatenation; no nesting reconstruction.
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_776_reassembly_is_concatenation() {
    // Pins record 776: "Blocks concatenate one-after-another. The
    // parser output is composable — blocks join sequentially.
    // Reassembly does not require nesting reconstruction; parsed
    // blocks are first-class units that can be re-emitted in order."
    let source = "(Move [1 2]) [a b c] {x y}";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3);

    // Take the three top-level blocks as a slice; reassemble.
    let block_refs: Vec<&Block> = blocks.iter().collect();
    let reassembled = BlockParser::reemit_concatenated(source, &block_refs);

    // The reassembled output is the concatenation of each block's
    // source slice. Whitespace between is one space (the join sep).
    assert!(reassembled.starts_with("(Move [1 2])"));
    assert!(reassembled.contains("[a b c]"));
    assert!(reassembled.ends_with("{x y}"));

    // No nested reconstruction: the blocks are first-class units.
    // Reordering them produces a still-valid concatenation.
    let reordered: Vec<&Block> = vec![&blocks[2], &blocks[0], &blocks[1]];
    let out = BlockParser::reemit_concatenated(source, &reordered);
    assert!(out.starts_with("{x y}"));
    assert!(out.ends_with("[a b c]"));
}

#[test]
fn constraint_776_blocks_are_first_class_units_filtered_and_rejoined() {
    // Pins record 776: blocks are first-class units. A caller can
    // FILTER blocks and reassemble the surviving set without parsing
    // them again.
    let source = "(keep me) [drop me] {keep me too}";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 3);

    // Keep the parens and brace blocks; drop the square brackets.
    let kept: Vec<&Block> = blocks
        .iter()
        .filter(|block| !block.is_square_bracket_block())
        .collect();
    assert_eq!(kept.len(), 2);

    let reassembled = BlockParser::reemit_concatenated(source, &kept);
    assert_eq!(reassembled, "(keep me) {keep me too}");
    // Nothing from the dropped block survives.
    assert!(!reassembled.contains("drop"));
}

#[test]
fn constraint_776_single_block_reemit_round_trips() {
    // Pins record 776 at the unit level: a single block re-emits as
    // its exact source slice.
    let source = "  [a [b [c [d]]]]   ";
    let parser = BlockParser::new(source);
    let blocks = parser.parse_blocks().expect("parse_blocks succeeds");
    assert_eq!(blocks.len(), 1);

    let outer = &blocks[0];
    let reemitted = outer.reemit(source);
    assert_eq!(
        reemitted, "[a [b [c [d]]]]",
        "exact span slice — no whitespace"
    );

    // And the deepest nested block also reemits exactly.
    let mut current = outer;
    while let Some(child) = current
        .root_objects
        .iter()
        .find(|child| child.is_square_bracket_block())
    {
        current = child;
    }
    // The innermost square-bracket block holds the [d] vector.
    let innermost = current;
    assert_eq!(innermost.reemit(source), "[d]");
}

// ════════════════════════════════════════════════════════════════════
// Cross-cutting — record 777 itself (intents-as-constraint-tests).
// ════════════════════════════════════════════════════════════════════

#[test]
fn constraint_777_named_tests_pin_each_intent() {
    // Pins record 777: "Every load-bearing intent statement should
    // manifest as a constraint test that fails if the intent is not
    // honored. Not principles-in-docs-only — verifiable tests that
    // pin the rule structurally or behaviorally."
    //
    // This test is a meta-assertion: it documents that the file
    // ABOVE contains named constraint tests for records 774, 775,
    // and 776. The naming convention is `constraint_<record>_*`.
    //
    // The test simply parses a small block to confirm the surface
    // is reachable and to act as a "smoke test" for the meta-claim.
    let parser = BlockParser::new("(Record (Topic [test]) Decision)");
    let blocks = parser.parse_blocks().expect("smoke test");
    assert_eq!(blocks.len(), 1);
    assert!(blocks[0].is_parenthesis_block());
    assert_eq!(
        blocks[0].holds_root_objects(),
        3,
        "Record + (Topic [test]) + Decision are three root objects"
    );
}
