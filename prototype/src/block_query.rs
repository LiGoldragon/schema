//! NOTA library surface — refined narrow API per records 799-803.
//!
//! Layers ON TOP of `blocks::Block` (the sibling subagent's structural
//! block-parsing slice from records 774-777). This module narrows the
//! NOTA-library API to a STRUCTURAL-QUERY surface only — record 803:
//! "NOTA does not perform schema-level interpretation."
//!
//! Two-kind discipline (record 800):
//!
//! * `is_X_bracket()` — FACTUAL delimiter classification. A block IS a
//!   parenthesis / square / brace block; there is no ambiguity.
//! * `qualifies_as_X()` — STRUCTURAL qualification. A token QUALIFIES
//!   as a symbol if its character alphabet permits — but a higher
//!   layer (the schema / macro layer) owns the interpretation
//!   decision. NOTA never decides "this PascalCase token IS a type
//!   name in this context".
//!
//! The default-to-higher rule (record 801):
//!
//! * At parse time, NOTA classifies a token with the HIGHEST
//!   classification it qualifies for. The schema layer can demote
//!   downward when its type context requires (qualified-symbol →
//!   string is easy; string → qualified-symbol is hard).
//!
//! Vector-element rule (record 802):
//!
//! * Inside a vector, every element is either a qualified-symbol or
//!   itself a block. This is a structural classification — the schema
//!   layer interprets the vector AS a struct-with-named-fields.
//!
//! Methods on `Block` here SUPPLEMENT — not replace — the methods in
//! `blocks.rs`. The naming convention `is_square_bracket` (no
//! `_block` suffix) follows the refined API in /357 §2 exactly; the
//! older `_block`-suffixed methods from `blocks.rs` remain available
//! for callers that already use them.

use crate::blocks::{Block, DelimiterKind, SourceSpan};
use crate::kernel::NodeKind;

/// Symbol qualification kind. Per record 800: a token QUALIFIES as
/// one of these by its character alphabet alone; whether it is
/// actually interpreted as that kind in some context is a schema-
/// layer decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    /// `PascalCase` — leading uppercase, no separators. Per the NOTA
    /// identifier classes, this maps onto type names and enum
    /// variant tags in most schemas.
    PascalCase,
    /// `camelCase` — leading lowercase, internal uppercase. Maps onto
    /// field names and macro names in most schemas.
    CamelCase,
    /// `kebab-case` — all-lowercase with dashes. Used for keywords
    /// and configuration keys in most schemas.
    KebabCase,
}

/// Literal classification — what kind of literal value a leaf
/// represents at the structural level. Distinct from `SymbolKind`
/// because literals don't qualify as symbols (no alphabet that
/// could be reinterpreted as a name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralKind {
    Integer,
    Float,
    Bytes,
    /// Bracket-string forms — inline `[...]`-with-string-content or
    /// block-string. NOTA strings come EXCLUSIVELY from bracket
    /// forms per AGENTS.md.
    BracketString,
    BlockString,
}

/// Block kind — for delimited (non-leaf) blocks. This is the
/// FACTUAL delimiter classification per record 799.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Parenthesis,
    SquareBracket,
    Brace,
}

/// The block's structural classification. Per record 801, the parser
/// defaults to the HIGHEST classification a token qualifies for; the
/// schema layer can demote downward when its context requires.
///
/// Ordering high-to-low (per record 801): delimited block > qualified
/// symbol > string > literal. A leaf that qualifies as a symbol is
/// classified as `QualifiedSymbol(...)`, NOT as `String`, even though
/// the same character bytes could be re-interpreted as a string in a
/// string-typed schema position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Classification {
    /// Delimited block — record / vector / map.
    Block(BlockKind),
    /// A leaf that qualifies as a symbol of one of three classes.
    QualifiedSymbol(SymbolKind),
    /// A leaf that qualifies as a string (bracket-form only per
    /// AGENTS.md). When a token COULD be a symbol AND a string,
    /// record 801 says: classify as symbol (the higher form).
    String,
    /// A leaf that qualifies only as a literal value.
    Literal(LiteralKind),
}

impl Block {
    // ── Delimiter classification — FACTUAL (record 799) ──────────
    //
    // These are the refined API names per /357 §2 — no `_block`
    // suffix. The factual nature is highlighted by the `is_` prefix:
    // a block IS a square-bracket block; there is no qualification.

    /// FACTUAL: this block is a square-bracket `[...]` block.
    /// Per record 799: delimiter classification is structural fact.
    pub fn is_square_bracket(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::SquareBracket)
    }

    /// FACTUAL: this block is a parenthesis `(...)` block.
    pub fn is_parenthesis(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::Parenthesis)
    }

    /// FACTUAL: this block is a brace `{...}` block.
    pub fn is_brace(&self) -> bool {
        matches!(self.delimiter, DelimiterKind::Brace)
    }

    // ── Symbol qualification — STRUCTURAL (records 800, 801) ─────
    //
    // The `qualifies_as_` prefix is load-bearing: NOTA only reports
    // whether a token's alphabet permits a particular interpretation.
    // The schema layer decides whether it IS that interpretation.

    /// STRUCTURAL: this block's leaf content qualifies as a NOTA
    /// symbol of some class. Per record 800: NOTA does not decide if
    /// it IS a symbol in any particular schema context — it only
    /// checks the alphabet.
    pub fn qualifies_as_symbol(&self) -> bool {
        matches!(self.leaf_kind, Some(NodeKind::Identifier)) && !self.leaf_text.is_empty()
    }

    /// STRUCTURAL: this block's leaf content qualifies as a PascalCase
    /// symbol — leading uppercase, identifier-class alphabet, no
    /// separator. Per record 800: NOTA does NOT decide whether it's
    /// allowed for something to be PascalCase or not.
    pub fn qualifies_as_pascal_case_symbol(&self) -> bool {
        if !self.qualifies_as_symbol() {
            return false;
        }
        Self::first_byte_uppercase_ascii(&self.leaf_text) && !self.leaf_text.contains('-')
    }

    /// STRUCTURAL: this block's leaf content qualifies as a camelCase
    /// symbol — leading lowercase, identifier-class alphabet, no
    /// separator.
    pub fn qualifies_as_camel_case_symbol(&self) -> bool {
        if !self.qualifies_as_symbol() {
            return false;
        }
        Self::first_byte_lowercase_ascii(&self.leaf_text) && !self.leaf_text.contains('-')
    }

    /// STRUCTURAL: this block's leaf content qualifies as a
    /// kebab-case symbol — all lowercase, dash-separated.
    pub fn qualifies_as_kebab_case_symbol(&self) -> bool {
        if !self.qualifies_as_symbol() {
            return false;
        }
        self.leaf_text.contains('-')
            && self
                .leaf_text
                .bytes()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-'))
    }

    /// STRUCTURAL: this block's leaf content qualifies as a string.
    /// Per AGENTS.md, NOTA strings come EXCLUSIVELY from bracket
    /// forms (inline `[...]`-with-string-content or block-string).
    /// Per record 801: a token that could BOTH qualify as a string
    /// AND a symbol classifies as a symbol (the higher form); only
    /// the explicit bracket-string syntax qualifies as a string.
    pub fn qualifies_as_string(&self) -> bool {
        matches!(
            self.leaf_kind,
            Some(NodeKind::InlineString) | Some(NodeKind::BlockString)
        )
    }

    /// STRUCTURAL: this block's leaf content qualifies as a numeric
    /// or byte literal.
    pub fn qualifies_as_literal(&self) -> bool {
        matches!(
            self.leaf_kind,
            Some(NodeKind::Integer) | Some(NodeKind::Float) | Some(NodeKind::Bytes)
        )
    }

    // ── The unified classification (record 801) ──────────────────

    /// The block's HIGHEST classification, defaulting upward per
    /// record 801. Resolution order:
    ///   1. Delimited block → `Classification::Block(BlockKind::...)`.
    ///   2. Symbol-qualifying leaf → `QualifiedSymbol(SymbolKind::...)`.
    ///   3. String-qualifying leaf → `String`.
    ///   4. Literal-qualifying leaf → `Literal(LiteralKind::...)`.
    pub fn classification(&self) -> Option<Classification> {
        // Delimited blocks first — they're the highest in the order.
        if let Some(block_kind) = self.delimiter_block_kind() {
            return Some(Classification::Block(block_kind));
        }
        // Then symbols — defaulting UP (record 801).
        if self.qualifies_as_pascal_case_symbol() {
            return Some(Classification::QualifiedSymbol(SymbolKind::PascalCase));
        }
        if self.qualifies_as_camel_case_symbol() {
            return Some(Classification::QualifiedSymbol(SymbolKind::CamelCase));
        }
        if self.qualifies_as_kebab_case_symbol() {
            return Some(Classification::QualifiedSymbol(SymbolKind::KebabCase));
        }
        // Then strings.
        if let Some(NodeKind::InlineString) = self.leaf_kind {
            return Some(Classification::String);
        }
        if let Some(NodeKind::BlockString) = self.leaf_kind {
            return Some(Classification::String);
        }
        // Finally literals.
        match self.leaf_kind {
            Some(NodeKind::Integer) => Some(Classification::Literal(LiteralKind::Integer)),
            Some(NodeKind::Float) => Some(Classification::Literal(LiteralKind::Float)),
            Some(NodeKind::Bytes) => Some(Classification::Literal(LiteralKind::Bytes)),
            _ => None,
        }
    }

    /// Block-kind for delimited blocks. Returns `None` for leaves.
    pub fn delimiter_block_kind(&self) -> Option<BlockKind> {
        match self.delimiter {
            DelimiterKind::Parenthesis => Some(BlockKind::Parenthesis),
            DelimiterKind::SquareBracket => Some(BlockKind::SquareBracket),
            DelimiterKind::Brace => Some(BlockKind::Brace),
            DelimiterKind::Leaf => None,
        }
    }

    /// Source span (record 774) — accessor for callers that prefer
    /// method syntax over field access. Wraps the public
    /// `Block::span` field.
    pub fn source_span(&self) -> SourceSpan {
        self.span
    }

    // ── Internal helpers — ASCII byte tests ──────────────────────

    fn first_byte_uppercase_ascii(text: &str) -> bool {
        matches!(text.bytes().next(), Some(byte) if (b'A'..=b'Z').contains(&byte))
    }

    fn first_byte_lowercase_ascii(text: &str) -> bool {
        matches!(text.bytes().next(), Some(byte) if (b'a'..=b'z').contains(&byte))
    }
}

/// Reassemble a sequence of blocks by concatenation, with a single
/// space between adjacent blocks. Per record 776: blocks are first-
/// class units; reassembly is concatenation. This is the
/// method-on-impl form of the same primitive `BlockParser` exposes,
/// living on a wrapper struct so it can be called without a
/// borrowed parser instance.
pub struct BlockReassembler;

impl BlockReassembler {
    /// Re-emit a slice of block references as concatenated source
    /// text. Each block emits as its exact source-span slice;
    /// adjacent blocks are separated by one space.
    ///
    /// This is the refined-API form of the same primitive in
    /// `blocks::BlockParser::reemit_concatenated`. Per AGENTS.md
    /// methods-on-impl-blocks discipline.
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
