//! Span metadata for Pass 1 NotaValue nodes.
//!
//! Per /334 §8 Q4: each NotaValue needs source position so later passes
//! can point error messages back at authored text. `nota-codec`'s
//! `Lexer` does NOT thread spans — `next_token` returns just the Token,
//! not its byte offset. The proof-of-concept hand-threads byte offsets
//! via an alternate lex-with-offset pass.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn empty() -> Self {
        Self { start: 0, end: 0 }
    }
}
