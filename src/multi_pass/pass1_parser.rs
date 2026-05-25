//! Pass 1 — Syntactic. Tokens -> `NotaValue` tree.
//!
//! Per /334 §3.2 this is supposed to live in `nota-codec`. It does NOT.
//! `nota-codec` exposes a streaming `Decoder`, not a tree parser. This
//! module ships the tree parser the multi-pass model needs and threads
//! byte-offset spans (per /334 §8 Q4) by reimplementing the lex loop.
//!
//! Treatment of `[ ]`: in schema context, every `[ ]` is a sequence.
//! `nota-codec`'s lexer also offers a bracket-STRING form via
//! `read_string_after_opening_bracket`, but schema text doesn't use it.

use crate::multi_pass::{NotaValue, Span};
use crate::{Error, Result};
use nota_codec::Token;

pub fn parse(input: &str) -> Result<NotaValue> {
    let mut parser = TreeParser::new(input);
    let value = parser.parse_value()?;
    if parser.next_token()?.is_some() {
        return Err(Error::InvalidSchemaText {
            context: "pass1",
            message: "unexpected trailing content".into(),
        });
    }
    Ok(value)
}

/// Sequence-parser handle exposed for Pass 2's six-value top-level read.
pub struct SequenceParser<'input> {
    inner: TreeParser<'input>,
}

impl<'input> SequenceParser<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            inner: TreeParser::new(input),
        }
    }

    pub fn next_value(&mut self, position: &'static str) -> Result<NotaValue> {
        let (token, span) = self
            .inner
            .next_token()?
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass1",
                message: format!("missing position `{position}`"),
            })?;
        self.inner.parse_value_with(token, span)
    }

    pub fn finish(&mut self) -> Result<()> {
        if self.inner.next_token()?.is_some() {
            return Err(Error::InvalidSchemaText {
                context: "pass1",
                message: "unexpected trailing content after six positions".into(),
            });
        }
        Ok(())
    }
}

struct TreeParser<'input> {
    input: &'input str,
    pos: usize,
    pushback: Option<(Token, Span)>,
}

impl<'input> TreeParser<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            pos: 0,
            pushback: None,
        }
    }

    pub(super) fn next_token(&mut self) -> Result<Option<(Token, Span)>> {
        if let Some(item) = self.pushback.take() {
            return Ok(Some(item));
        }
        let start = self.skip_whitespace_and_comments();
        if self.pos >= self.input.len() {
            return Ok(None);
        }
        // Use nota-codec's lexer on a slice anchored at `self.pos`.
        let remaining = &self.input[self.pos..];
        let mut lexer = nota_codec::Lexer::new(remaining);
        let token = lexer
            .next_token()
            .map_err(|error| Error::InvalidSchemaText {
                context: "pass1",
                message: format!("lex error at byte {start}: {error}"),
            })?;
        // Advance self.pos by the number of bytes consumed.
        let consumed = bytes_consumed_by(remaining, &token);
        let end = self.pos + consumed;
        self.pos = end;
        Ok(token.map(|token| (token, Span::new(start, end))))
    }

    fn peek_token(&mut self) -> Result<Option<(Token, Span)>> {
        if self.pushback.is_some() {
            return Ok(self.pushback.clone());
        }
        let next = self.next_token()?;
        if let Some(item) = &next {
            self.pushback = Some(item.clone());
        }
        Ok(next)
    }

    fn skip_whitespace_and_comments(&mut self) -> usize {
        let bytes = self.input.as_bytes();
        loop {
            match bytes.get(self.pos).copied() {
                Some(b) if b.is_ascii_whitespace() => self.pos += 1,
                Some(b';') if bytes.get(self.pos + 1) == Some(&b';') => {
                    while let Some(b) = bytes.get(self.pos).copied() {
                        self.pos += 1;
                        if b == b'\n' {
                            break;
                        }
                    }
                }
                _ => break,
            }
        }
        self.pos
    }

    pub(super) fn parse_value(&mut self) -> Result<NotaValue> {
        let (token, span) = self.next_token()?.ok_or_else(|| Error::InvalidSchemaText {
            context: "pass1",
            message: "unexpected end of input".into(),
        })?;
        self.parse_value_with(token, span)
    }

    pub(super) fn parse_value_with(&mut self, token: Token, span: Span) -> Result<NotaValue> {
        match token {
            Token::LParen => self.parse_record(span),
            Token::LBracket => self.parse_list(span),
            Token::LBrace => self.parse_map(span),
            Token::Ident(name) => Ok(NotaValue::Identifier(name, span)),
            Token::Str(value) => Ok(NotaValue::String(value, span)),
            Token::Int(value) => Ok(NotaValue::Integer(value, span)),
            Token::UInt(value) => Ok(NotaValue::Integer(value as i128, span)),
            other => Err(Error::InvalidSchemaText {
                context: "pass1",
                message: format!("unexpected token {other:?}"),
            }),
        }
    }

    fn parse_record(&mut self, start: Span) -> Result<NotaValue> {
        let mut items = Vec::new();
        loop {
            let (token, span) = self.next_token()?.ok_or_else(|| Error::InvalidSchemaText {
                context: "pass1",
                message: "unterminated record".into(),
            })?;
            if matches!(token, Token::RParen) {
                return Ok(NotaValue::Record(items, Span::new(start.start, span.end)));
            }
            items.push(self.parse_value_with(token, span)?);
        }
    }

    fn parse_list(&mut self, start: Span) -> Result<NotaValue> {
        let mut items = Vec::new();
        loop {
            let (token, span) = self.next_token()?.ok_or_else(|| Error::InvalidSchemaText {
                context: "pass1",
                message: "unterminated list".into(),
            })?;
            if matches!(token, Token::RBracket) {
                return Ok(NotaValue::List(items, Span::new(start.start, span.end)));
            }
            items.push(self.parse_value_with(token, span)?);
        }
    }

    fn parse_map(&mut self, start: Span) -> Result<NotaValue> {
        let mut entries = Vec::new();
        loop {
            let (token, span) = self.next_token()?.ok_or_else(|| Error::InvalidSchemaText {
                context: "pass1",
                message: "unterminated map".into(),
            })?;
            if matches!(token, Token::RBrace) {
                return Ok(NotaValue::Map(entries, Span::new(start.start, span.end)));
            }
            let key = match token {
                Token::Ident(name) => name,
                Token::Str(text) => text,
                other => {
                    return Err(Error::InvalidSchemaText {
                        context: "pass1",
                        message: format!("map key must be identifier or string, got {other:?}"),
                    });
                }
            };
            // Peek next; if it's a colon, consume it (NOTA map shape).
            let _ = span;
            if let Some((Token::Colon, _)) = self.peek_token()? {
                let _ = self.next_token()?;
            }
            let value = self.parse_value()?;
            entries.push((key, value));
        }
    }
}

/// How many bytes did the lexer consume to produce this token, starting
/// from the beginning of `slice`? Required because `Lexer::next_token`
/// doesn't expose its position. We re-run the same recognition logic.
///
/// This is a kludge: it duplicates lex-counting because `nota-codec`
/// doesn't surface span info. /334 §8 Q4 surfaced.
fn bytes_consumed_by(slice: &str, token: &Option<Token>) -> usize {
    let bytes = slice.as_bytes();
    let mut pos = 0;
    // Skip leading whitespace + comments (mirrors lexer).
    loop {
        match bytes.get(pos).copied() {
            Some(b) if b.is_ascii_whitespace() => pos += 1,
            Some(b';') if bytes.get(pos + 1) == Some(&b';') => {
                while let Some(b) = bytes.get(pos).copied() {
                    pos += 1;
                    if b == b'\n' {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
    let Some(token) = token else {
        return pos;
    };
    let Some(b) = bytes.get(pos).copied() else {
        return pos;
    };
    match token {
        Token::LParen
        | Token::RParen
        | Token::LBracket
        | Token::RBracket
        | Token::LBrace
        | Token::RBrace
        | Token::Colon => pos + 1,
        Token::Ident(name) => {
            let _ = b;
            pos + name.len()
        }
        Token::Str(text) => {
            // Could be `"..."`, `[...]`, or `[|...|]`. Best-effort:
            // re-scan for the closing delimiter.
            scan_string_end(bytes, pos, text)
        }
        Token::Int(_) | Token::UInt(_) => {
            let raw_len = scan_number_len(bytes, pos);
            pos + raw_len
        }
        Token::Float(_) => pos + scan_number_len(bytes, pos),
        Token::DateLiteral { .. } => pos + 10,
        Token::TimeLiteral { .. } => pos + 8,
        Token::Bytes(bytes_value) => {
            // `#` prefix + 2 hex chars per byte.
            pos + 1 + bytes_value.len() * 2
        }
    }
}

fn scan_string_end(bytes: &[u8], start: usize, _text: &str) -> usize {
    let mut pos = start;
    let first = bytes.get(pos).copied();
    if first == Some(b'"') {
        // Triple-quote case.
        if bytes.get(pos + 1) == Some(&b'"') && bytes.get(pos + 2) == Some(&b'"') {
            pos += 3;
            while pos + 2 < bytes.len() {
                if bytes[pos] == b'"' && bytes[pos + 1] == b'"' && bytes[pos + 2] == b'"' {
                    return pos + 3;
                }
                pos += 1;
            }
            return bytes.len();
        }
        pos += 1;
        while pos < bytes.len() {
            match bytes[pos] {
                b'\\' => pos += 2,
                b'"' => return pos + 1,
                _ => pos += 1,
            }
        }
        bytes.len()
    } else if first == Some(b'[') {
        pos += 1;
        if bytes.get(pos) == Some(&b'|') {
            pos += 1;
            while pos + 1 < bytes.len() {
                if bytes[pos] == b'|' && bytes[pos + 1] == b']' {
                    return pos + 2;
                }
                pos += 1;
            }
            bytes.len()
        } else {
            while pos < bytes.len() {
                match bytes[pos] {
                    b'\\' => pos += 2,
                    b']' => return pos + 1,
                    _ => pos += 1,
                }
            }
            bytes.len()
        }
    } else {
        pos
    }
}

fn scan_number_len(bytes: &[u8], start: usize) -> usize {
    let mut pos = start;
    if bytes.get(pos) == Some(&b'-') {
        pos += 1;
    }
    // Allow 0x / 0b / 0o prefixes.
    if bytes.get(pos) == Some(&b'0') {
        match bytes.get(pos + 1).copied() {
            Some(b'x') | Some(b'X') | Some(b'b') | Some(b'B') | Some(b'o') | Some(b'O') => {
                pos += 2;
                while let Some(b) = bytes.get(pos).copied() {
                    if b.is_ascii_alphanumeric() || b == b'_' {
                        pos += 1;
                    } else {
                        break;
                    }
                }
                return pos - start;
            }
            _ => {}
        }
    }
    let mut saw_dot = false;
    let mut saw_exp = false;
    while let Some(b) = bytes.get(pos).copied() {
        match b {
            b'0'..=b'9' | b'_' => pos += 1,
            b'.' if !saw_dot && !saw_exp => {
                saw_dot = true;
                pos += 1;
            }
            b'e' | b'E' if !saw_exp => {
                saw_exp = true;
                pos += 1;
                if matches!(bytes.get(pos).copied(), Some(b'+') | Some(b'-')) {
                    pos += 1;
                }
            }
            _ => break,
        }
    }
    pos - start
}
