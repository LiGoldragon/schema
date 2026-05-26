//! Block-by-block parsing slice per records 774-777.
//!
//! Methodology (record 774):
//!
//! * NOTA parsing breaks down by OBJECT using positional ranges.
//! * Each `Block` tracks its `SourceSpan` (line/column for both start
//!   and end on the original text).
//! * Each `Block` carries methods/predicates to query its structure:
//!   `is_square_bracket_block` / `is_parenthesis_block` /
//!   `is_brace_block`; `holds_single_root_object` /
//!   `holds_root_objects` (count) / `root_object_at(n)`.
//! * Recursion: each root object inside a block IS itself a typed,
//!   positioned `Block`. The recursive parsing chain IS the chain of
//!   block-level queries.
//!
//! Direction (record 775):
//!
//! * Range-based span tracking on the ORIGINAL source text — NOT a
//!   normalize-first approach. Line and column for both start and
//!   end of every block are preserved exactly.
//!
//! Reassembly (record 776):
//!
//! * Blocks concatenate one-after-another. The parser output is
//!   composable; reassembly does NOT require nesting reconstruction.
//!   Parsed blocks are first-class units re-emittable in order.
//!
//! Intents-as-tests (record 777):
//!
//! * Each load-bearing intent above gets a NAMED constraint test in
//!   `tests/block_parser_constraints.rs`. Tests pin the rule
//!   structurally or behaviorally — not principles-in-docs-only.

use crate::kernel::{Kernel, KernelError, Node, NodeKind};

/// Position on the ORIGINAL source text. Line and column are 1-based
/// to match editor and diagnostic conventions. `byte_offset` is also
/// preserved so callers that want raw byte access never have to
/// re-derive it from line/column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: u32,
    pub column: u32,
    pub byte_offset: usize,
}

impl SourcePosition {
    pub fn at_start() -> Self {
        Self {
            line: 1,
            column: 1,
            byte_offset: 0,
        }
    }
}

/// Range on the original source text. `start` and `end` are
/// `SourcePosition`s; `end` is one past the last byte of the block
/// (exclusive end), matching the kernel's byte-range convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    pub start: SourcePosition,
    pub end: SourcePosition,
}

impl SourceSpan {
    /// Slice the original source by this span's byte range. The
    /// caller passes the full source; this method returns the exact
    /// substring the block covers — load-bearing for the
    /// reassembly-by-concatenation contract (record 776).
    pub fn slice<'src>(&self, source: &'src str) -> &'src str {
        &source[self.start.byte_offset..self.end.byte_offset]
    }
}

/// Delimiter classification per record 774's three predicates.
///
/// `Leaf` covers identifiers, literals, and bracket strings — the
/// kernel emits these without nested delimiter structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DelimiterKind {
    Parenthesis,
    SquareBracket,
    Brace,
    Leaf,
}

/// A `Block` is a typed, positioned unit of NOTA source. Each block
/// carries:
///
/// * Its delimiter kind (`Parenthesis` / `SquareBracket` / `Brace` /
///   `Leaf`).
/// * Its source span (line/column for start AND end on the original
///   text — record 774).
/// * Its root objects (the immediate children, each itself a
///   `Block` — record 774's recursion clause).
/// * Its leaf text (for `Leaf` blocks: the bare identifier or
///   literal lexeme; for delimited blocks: empty).
///
/// Each root object inside a delimited block IS itself a `Block`.
/// The methods on `Block` ARE the recursive parsing chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub delimiter: DelimiterKind,
    pub span: SourceSpan,
    pub root_objects: Vec<Block>,
    /// Leaf text for `Leaf` blocks; empty for delimited blocks.
    pub leaf_text: String,
    /// Best-effort classification of leaf content. `None` for
    /// delimited blocks. `Some(NodeKind::Identifier)` /
    /// `Integer` / `Float` / `InlineString` / `BlockString` /
    /// `Bytes` for leaves.
    pub leaf_kind: Option<NodeKind>,
}

impl Block {
    // ── Predicate methods per record 774 ─────────────────────────

    pub fn is_parenthesis_block(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::Parenthesis)
    }

    pub fn is_square_bracket_block(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::SquareBracket)
    }

    pub fn is_brace_block(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::Brace)
    }

    pub fn is_leaf(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::Leaf)
    }

    pub fn holds_single_root_object(&self) -> bool {
        self.root_objects.len() == 1
    }

    /// Returns the count of root objects this block holds. Per
    /// record 774's `holds_N_root_objects` family — a single
    /// counting method generalises the N-specific predicates.
    pub fn holds_root_objects(&self) -> usize {
        self.root_objects.len()
    }

    /// Returns the root object at index `n`, if any. Per record
    /// 774's recursion clause: each root object IS itself a block.
    pub fn root_object_at(&self, n: usize) -> Option<&Block> {
        self.root_objects.get(n)
    }

    // ── Recursive shape predicates (record 774 + 772) ────────────

    /// `second_root_object_is_a_square_bracket_object` — example
    /// of a recursive shape predicate per record 774. Returns true
    /// when this block has at least 2 root objects AND the second
    /// (index 1) is a square-bracket block. Used by macro
    /// recognizers (record 772).
    pub fn second_root_object_is_a_square_bracket_object(&self) -> bool {
        self.root_object_at(1)
            .map(|block| block.is_square_bracket_block())
            .unwrap_or(false)
    }

    /// `second_root_object_qualifies_as_a_symbol` — recursive shape
    /// predicate per records 774 + 772. Returns true when the
    /// second root object is a leaf identifier. Used by macro
    /// shape-interpretation.
    pub fn second_root_object_qualifies_as_a_symbol(&self) -> bool {
        self.root_object_at(1)
            .map(|block| block.is_leaf() && block.leaf_kind == Some(NodeKind::Identifier))
            .unwrap_or(false)
    }

    /// `holds_two_root_objects` — explicit count predicate per
    /// record 774. Provided as a convenience for callers that want
    /// the named predicate rather than the count comparison.
    pub fn holds_two_root_objects(&self) -> bool {
        self.root_objects.len() == 2
    }

    /// Re-emit this block as the substring of the original source
    /// it spans. Range-based — no reconstruction from children.
    /// Per record 775: the implementation tracks source ranges, NOT
    /// a normalized form.
    pub fn reemit<'src>(&self, source: &'src str) -> &'src str {
        self.span.slice(source)
    }
}

/// Block parsing entry point. Parses NOTA source into a sequence of
/// top-level blocks, each carrying its source span and recursive
/// root-object structure.
pub struct BlockParser<'src> {
    source: &'src str,
}

impl<'src> BlockParser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source }
    }

    /// Parse the source into a top-level sequence of blocks. The
    /// kernel's parser produces `Node`s; this method lifts each
    /// `Node` into a `Block` carrying line/column spans.
    pub fn parse_blocks(&self) -> Result<Vec<Block>, KernelError> {
        let mut kernel = Kernel::new(self.source);
        let nodes = kernel.parse_sequence()?;
        let line_index = LineIndex::build(self.source);
        let mut blocks = Vec::with_capacity(nodes.len());
        for node in &nodes {
            blocks.push(Self::node_to_block(node, &line_index));
        }
        Ok(blocks)
    }

    /// Convert a kernel `Node` into a `Block`, recursively. The
    /// kernel already tracks byte ranges; this method adds the
    /// line/column projection and the delimiter classification.
    fn node_to_block(node: &Node, line_index: &LineIndex) -> Block {
        let span = SourceSpan {
            start: line_index.position_of(node.start),
            end: line_index.position_of(node.end),
        };
        let delimiter = match node.kind {
            NodeKind::Record => DelimiterKind::Parenthesis,
            NodeKind::Vector => DelimiterKind::SquareBracket,
            NodeKind::Map => DelimiterKind::Brace,
            _ => DelimiterKind::Leaf,
        };
        let leaf_kind = match &node.kind {
            NodeKind::Record | NodeKind::Vector | NodeKind::Map => None,
            other => Some(other.clone()),
        };
        let leaf_text = match node.kind {
            NodeKind::Record | NodeKind::Vector | NodeKind::Map => String::new(),
            _ => node.text.clone(),
        };
        let root_objects = node
            .children
            .iter()
            .map(|child| Self::node_to_block(child, line_index))
            .collect();
        Block {
            delimiter,
            span,
            root_objects,
            leaf_text,
            leaf_kind,
        }
    }

    /// Reassemble a sequence of blocks by sequential concatenation
    /// (record 776). Each block re-emits as its source slice; the
    /// joined result is the concatenation of those slices with a
    /// single space between blocks. No nesting reconstruction.
    ///
    /// Per record 776: blocks are first-class units; reassembly is
    /// concatenation.
    pub fn reemit_concatenated(source: &str, blocks: &[&Block]) -> String {
        let mut output = String::new();
        for (index, block) in blocks.iter().enumerate() {
            if index > 0 {
                output.push(' ');
            }
            output.push_str(block.reemit(source));
        }
        output
    }
}

/// Line index — projects byte offsets into the original source onto
/// (line, column) coordinates. Built once per parse.
///
/// Lines and columns are 1-based to match editor and diagnostic
/// conventions. `line_starts` holds the byte offset of the first
/// byte of each line; column = (byte_offset - line_start) + 1.
struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    fn build(source: &str) -> Self {
        let mut line_starts = vec![0usize];
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self { line_starts }
    }

    fn position_of(&self, byte_offset: usize) -> SourcePosition {
        // Find the largest line_starts[i] <= byte_offset.
        let line_index = match self.line_starts.binary_search(&byte_offset) {
            Ok(found) => found,
            Err(insertion) => insertion.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let column = byte_offset - line_start + 1;
        SourcePosition {
            line: (line_index + 1) as u32,
            column: column as u32,
            byte_offset,
        }
    }
}
