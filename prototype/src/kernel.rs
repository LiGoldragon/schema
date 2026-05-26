//! Bootstrap kernel — the minimum hand-authored Rust needed to
//! recognise NOTA before any schemas are loaded.
//!
//! Scope (deliberately small):
//!   - Three delimiter pairs:  `( )`  `[ ]`  `{ }`
//!   - Inline bracket strings:    `[text]`     (no nesting of unescaped `]`)
//!   - Block bracket strings:     `[| text |]`
//!   - Line comments:             `;; ... \n`
//!   - Bytes / hash sigil:        `#a1b2c3`
//!   - Identifiers (PascalCase / camelCase / kebab-case)
//!   - Integer + float literals
//!
//! This kernel parses tokens into a tree of `Node` values. Schema
//! interpretation (which `(...)` is a struct vs variant, which `[...]`
//! is a vector vs string) is the JOB of the layer ABOVE this kernel —
//! the schema reader uses kernel `Node`s as raw delimiter tree and
//! decides interpretation per schema position.

use core::fmt;

/// Kernel error surface — a small named-variant set so the schema
/// layer can decode kernel failures without string-matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    UnclosedDelimiter { opener: char, position: usize },
    UnclosedString { position: usize },
    UnclosedBlockString { position: usize },
    UnexpectedClose { closer: char, position: usize },
    InvalidEscape { position: usize },
    InvalidByteLiteral { position: usize },
    InvalidNumber { position: usize },
    UnexpectedEof { position: usize },
    EmptyInput,
}

impl fmt::Display for KernelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KernelError::UnclosedDelimiter { opener, position } => {
                write!(formatter, "unclosed `{opener}` at byte {position}")
            }
            KernelError::UnclosedString { position } => {
                write!(formatter, "unclosed bracket string at byte {position}")
            }
            KernelError::UnclosedBlockString { position } => {
                write!(formatter, "unclosed block string at byte {position}")
            }
            KernelError::UnexpectedClose { closer, position } => {
                write!(formatter, "unexpected `{closer}` at byte {position}")
            }
            KernelError::InvalidEscape { position } => {
                write!(formatter, "invalid escape at byte {position}")
            }
            KernelError::InvalidByteLiteral { position } => {
                write!(
                    formatter,
                    "invalid `#`-prefixed byte literal at byte {position}"
                )
            }
            KernelError::InvalidNumber { position } => {
                write!(formatter, "invalid number at byte {position}")
            }
            KernelError::UnexpectedEof { position } => {
                write!(formatter, "unexpected end of input at byte {position}")
            }
            KernelError::EmptyInput => write!(formatter, "empty input"),
        }
    }
}

impl std::error::Error for KernelError {}

/// Token kinds the kernel emits. Each token carries a byte range
/// into the source. The schema layer (`crate::schema`) consumes
/// tokens via the `Node` tree, not directly — `KernelToken` is
/// public so tests and the demo bin can introspect the lex stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelTokenKind {
    RecordOpen,
    RecordClose,
    VectorOpen,
    VectorClose,
    MapOpen,
    MapClose,
    Identifier,
    Integer,
    Float,
    InlineString,
    BlockString,
    Bytes,
    LineComment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelToken {
    pub kind: KernelTokenKind,
    /// Inclusive start byte.
    pub start: usize,
    /// Exclusive end byte.
    pub end: usize,
}

impl KernelToken {
    pub fn lexeme<'src>(&self, source: &'src str) -> &'src str {
        &source[self.start..self.end]
    }
}

/// A parsed delimiter tree. Schema interpretation layers on top.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    Record,
    Vector,
    Map,
    Identifier,
    Integer,
    Float,
    InlineString,
    BlockString,
    Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub kind: NodeKind,
    pub start: usize,
    pub end: usize,
    pub children: Vec<Node>,
    /// For leaf tokens: the raw lexeme. For records/vectors/maps:
    /// empty (children carry the structure).
    pub text: String,
}

impl Node {
    pub fn is_record(&self) -> bool {
        matches!(self.kind, NodeKind::Record)
    }
    pub fn is_vector(&self) -> bool {
        matches!(self.kind, NodeKind::Vector)
    }
    pub fn is_map(&self) -> bool {
        matches!(self.kind, NodeKind::Map)
    }
    pub fn is_identifier(&self) -> bool {
        matches!(self.kind, NodeKind::Identifier)
    }
    pub fn as_identifier(&self) -> Option<&str> {
        if self.is_identifier() {
            Some(self.text.as_str())
        } else {
            None
        }
    }
}

/// The kernel itself. Stateless construction; `parse` is the entry
/// point. Lifetime'd over the source so token lexemes can borrow.
pub struct Kernel<'src> {
    source: &'src str,
    cursor: usize,
}

impl<'src> Kernel<'src> {
    pub fn new(source: &'src str) -> Self {
        Self { source, cursor: 0 }
    }

    /// Lex the source into a flat token stream. Comments are kept
    /// in the stream so callers can decide whether to discard them;
    /// the parser stage drops them.
    pub fn lex(&mut self) -> Result<Vec<KernelToken>, KernelError> {
        let mut tokens = Vec::new();
        while let Some(token) = self.next_token()? {
            tokens.push(token);
        }
        Ok(tokens)
    }

    /// Parse the source into a sequence of top-level `Node`s.
    /// `nota.schema` is a top-level sequence: `{...} [...] [...] {...} [...]`.
    pub fn parse_sequence(&mut self) -> Result<Vec<Node>, KernelError> {
        let tokens = self.lex()?;
        let mut walker = NodeWalker::new(&tokens, self.source);
        walker.parse_top_level()
    }

    /// Parse the source into a single Node — for embedded contexts
    /// where exactly one value is expected.
    pub fn parse_single(&mut self) -> Result<Node, KernelError> {
        let mut tokens = self.lex()?;
        tokens.retain(|token| token.kind != KernelTokenKind::LineComment);
        if tokens.is_empty() {
            return Err(KernelError::EmptyInput);
        }
        let mut walker = NodeWalker::new(&tokens, self.source);
        let node = walker.parse_one()?;
        if walker.peek().is_some() {
            return Err(KernelError::UnexpectedEof {
                position: walker.peek().map(|t| t.start).unwrap_or(self.source.len()),
            });
        }
        Ok(node)
    }

    fn next_token(&mut self) -> Result<Option<KernelToken>, KernelError> {
        self.skip_whitespace();
        if self.cursor >= self.source.len() {
            return Ok(None);
        }
        let start = self.cursor;
        let byte = self.source.as_bytes()[self.cursor];
        match byte {
            b'(' => self.single(start, KernelTokenKind::RecordOpen),
            b')' => self.single(start, KernelTokenKind::RecordClose),
            b'{' => self.single(start, KernelTokenKind::MapOpen),
            b'}' => self.single(start, KernelTokenKind::MapClose),
            b'[' => self.lex_bracket(start),
            b']' => self.single(start, KernelTokenKind::VectorClose),
            b';' if self.peek_byte(1) == Some(b';') => self.lex_line_comment(start),
            b'#' => self.lex_bytes(start),
            b'-' | b'0'..=b'9' => self.lex_number(start),
            byte if Self::is_identifier_start(byte) => self.lex_identifier(start),
            _ => Err(KernelError::UnclosedString { position: start }),
        }
    }

    fn lex_bracket(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        // Bracket disambiguation (kernel-level):
        //
        //   `[|...|]` → block string, unambiguous.
        //
        //   `[...]`   → If the interior contains a byte that CANNOT
        //               appear as a token (apostrophe, escape, colon
        //               outside a path field, raw quote), this is an
        //               inline bracket string. Otherwise emit as
        //               VectorOpen. The schema layer (per record 747)
        //               re-interprets a single-child vector as an
        //               inline string when the schema position
        //               demands `String`.
        //
        // This matches the README's "schema-positioned square brackets"
        // section: the kernel resolves the textbook cases; residual
        // ambiguity flows to the schema layer.
        if self.peek_byte(1) == Some(b'|') {
            return self.lex_block_string(start);
        }
        let bytes = self.source.as_bytes();
        let mut probe = self.cursor + 1;
        let mut has_string_only_byte = false;
        let mut found_delimiter = false;
        let mut closed = false;
        while probe < bytes.len() {
            let byte = bytes[probe];
            match byte {
                b']' => {
                    closed = true;
                    break;
                }
                b'\\' => {
                    has_string_only_byte = true;
                    probe += 2;
                    continue;
                }
                // Any of these indicates we're inside a vector
                // containing structured nota — not a flat string.
                b'(' | b')' | b'{' | b'}' | b'[' => {
                    found_delimiter = true;
                    break;
                }
                // Comments inside a bracket-string would also indicate
                // structured content.
                b';' if probe + 1 < bytes.len() && bytes[probe + 1] == b';' => {
                    found_delimiter = true;
                    break;
                }
                b'\'' | b'"' | b':' | b'?' | b'!' | b'@' | b'~' | b'*' | b'=' | b'<' | b'>' => {
                    has_string_only_byte = true;
                }
                b' ' | b'\t' | b'\n' | b'\r' | b'.' | b'/' | b'-' | b'_' => {}
                byte if byte.is_ascii_alphanumeric() => {}
                _ => has_string_only_byte = true,
            }
            probe += 1;
        }
        if found_delimiter {
            return self.single(start, KernelTokenKind::VectorOpen);
        }
        if !closed {
            return Err(KernelError::UnclosedString { position: start });
        }
        if has_string_only_byte {
            self.lex_inline_string(start)
        } else {
            self.single(start, KernelTokenKind::VectorOpen)
        }
    }

    fn lex_inline_string(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        // Consume `[`
        self.cursor += 1;
        let bytes = self.source.as_bytes();
        while self.cursor < bytes.len() {
            match bytes[self.cursor] {
                b'\\' => {
                    self.cursor += 1;
                    if self.cursor >= bytes.len() {
                        return Err(KernelError::InvalidEscape {
                            position: self.cursor,
                        });
                    }
                    // Accept any escape; the schema layer interprets.
                    self.cursor += 1;
                }
                b']' => {
                    self.cursor += 1;
                    return Ok(Some(KernelToken {
                        kind: KernelTokenKind::InlineString,
                        start,
                        end: self.cursor,
                    }));
                }
                b'\n' => return Err(KernelError::UnclosedString { position: start }),
                _ => self.cursor += 1,
            }
        }
        Err(KernelError::UnclosedString { position: start })
    }

    fn lex_block_string(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        // Skip `[|`
        self.cursor += 2;
        let bytes = self.source.as_bytes();
        while self.cursor + 1 < bytes.len() {
            if bytes[self.cursor] == b'|' && bytes[self.cursor + 1] == b']' {
                self.cursor += 2;
                return Ok(Some(KernelToken {
                    kind: KernelTokenKind::BlockString,
                    start,
                    end: self.cursor,
                }));
            }
            self.cursor += 1;
        }
        Err(KernelError::UnclosedBlockString { position: start })
    }

    fn lex_line_comment(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        let bytes = self.source.as_bytes();
        self.cursor += 2;
        while self.cursor < bytes.len() && bytes[self.cursor] != b'\n' {
            self.cursor += 1;
        }
        Ok(Some(KernelToken {
            kind: KernelTokenKind::LineComment,
            start,
            end: self.cursor,
        }))
    }

    fn lex_bytes(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        self.cursor += 1;
        let bytes = self.source.as_bytes();
        let body_start = self.cursor;
        while self.cursor < bytes.len() && bytes[self.cursor].is_ascii_hexdigit() {
            self.cursor += 1;
        }
        let body_len = self.cursor - body_start;
        if body_len == 0 || body_len % 2 != 0 {
            return Err(KernelError::InvalidByteLiteral { position: start });
        }
        Ok(Some(KernelToken {
            kind: KernelTokenKind::Bytes,
            start,
            end: self.cursor,
        }))
    }

    fn lex_number(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        let bytes = self.source.as_bytes();
        if bytes[self.cursor] == b'-' {
            self.cursor += 1;
            if self.cursor >= bytes.len() || !bytes[self.cursor].is_ascii_digit() {
                // Not actually a number — was misclassified by next_token.
                // We don't backtrack; surface as InvalidNumber so the
                // caller sees the failure precisely.
                return Err(KernelError::InvalidNumber { position: start });
            }
        }
        let mut saw_dot = false;
        let mut saw_x_or_b_or_o = false;
        while self.cursor < bytes.len() {
            let byte = bytes[self.cursor];
            match byte {
                b'0'..=b'9' | b'_' => self.cursor += 1,
                b'.' if !saw_dot && !saw_x_or_b_or_o => {
                    saw_dot = true;
                    self.cursor += 1;
                }
                b'x' | b'X' | b'b' | b'B' | b'o' | b'O' if !saw_dot => {
                    saw_x_or_b_or_o = true;
                    self.cursor += 1;
                }
                b'a'..=b'f' | b'A'..=b'F' if saw_x_or_b_or_o => self.cursor += 1,
                _ => break,
            }
        }
        let kind = if saw_dot {
            KernelTokenKind::Float
        } else {
            KernelTokenKind::Integer
        };
        Ok(Some(KernelToken {
            kind,
            start,
            end: self.cursor,
        }))
    }

    fn lex_identifier(&mut self, start: usize) -> Result<Option<KernelToken>, KernelError> {
        let bytes = self.source.as_bytes();
        while self.cursor < bytes.len() && Self::is_identifier_continue(bytes[self.cursor]) {
            self.cursor += 1;
        }
        Ok(Some(KernelToken {
            kind: KernelTokenKind::Identifier,
            start,
            end: self.cursor,
        }))
    }

    fn single(
        &mut self,
        start: usize,
        kind: KernelTokenKind,
    ) -> Result<Option<KernelToken>, KernelError> {
        self.cursor += 1;
        Ok(Some(KernelToken {
            kind,
            start,
            end: self.cursor,
        }))
    }

    fn skip_whitespace(&mut self) {
        let bytes = self.source.as_bytes();
        while self.cursor < bytes.len() {
            match bytes[self.cursor] {
                b' ' | b'\t' | b'\n' | b'\r' => self.cursor += 1,
                _ => break,
            }
        }
    }

    fn peek_byte(&self, offset: usize) -> Option<u8> {
        self.source.as_bytes().get(self.cursor + offset).copied()
    }

    fn is_identifier_start(byte: u8) -> bool {
        byte.is_ascii_alphabetic() || byte == b'_'
    }

    fn is_identifier_continue(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
    }
}

/// Walker over the kernel token stream that builds the `Node` tree.
struct NodeWalker<'tokens, 'src> {
    tokens: &'tokens [KernelToken],
    cursor: usize,
    source: &'src str,
}

impl<'tokens, 'src> NodeWalker<'tokens, 'src> {
    fn new(tokens: &'tokens [KernelToken], source: &'src str) -> Self {
        let mut walker = Self {
            tokens,
            cursor: 0,
            source,
        };
        walker.skip_comments();
        walker
    }

    fn parse_top_level(&mut self) -> Result<Vec<Node>, KernelError> {
        let mut nodes = Vec::new();
        while self.cursor < self.tokens.len() {
            nodes.push(self.parse_one()?);
            self.skip_comments();
        }
        Ok(nodes)
    }

    fn parse_one(&mut self) -> Result<Node, KernelError> {
        self.skip_comments();
        let token = self
            .tokens
            .get(self.cursor)
            .cloned()
            .ok_or(KernelError::UnexpectedEof {
                position: self.source.len(),
            })?;
        match token.kind {
            KernelTokenKind::RecordOpen => self.parse_record(token.start),
            KernelTokenKind::VectorOpen => self.parse_vector(token.start),
            KernelTokenKind::MapOpen => self.parse_map(token.start),
            KernelTokenKind::Identifier => {
                self.cursor += 1;
                Ok(Node {
                    kind: NodeKind::Identifier,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: token.lexeme(self.source).to_string(),
                })
            }
            KernelTokenKind::Integer => {
                self.cursor += 1;
                Ok(Node {
                    kind: NodeKind::Integer,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: token.lexeme(self.source).to_string(),
                })
            }
            KernelTokenKind::Float => {
                self.cursor += 1;
                Ok(Node {
                    kind: NodeKind::Float,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: token.lexeme(self.source).to_string(),
                })
            }
            KernelTokenKind::InlineString => {
                self.cursor += 1;
                let lexeme = token.lexeme(self.source);
                // strip leading `[` and trailing `]`
                let inner = &lexeme[1..lexeme.len() - 1];
                Ok(Node {
                    kind: NodeKind::InlineString,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: inner.to_string(),
                })
            }
            KernelTokenKind::BlockString => {
                self.cursor += 1;
                let lexeme = token.lexeme(self.source);
                // strip leading `[|` and trailing `|]`
                let inner = &lexeme[2..lexeme.len() - 2];
                Ok(Node {
                    kind: NodeKind::BlockString,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: inner.to_string(),
                })
            }
            KernelTokenKind::Bytes => {
                self.cursor += 1;
                Ok(Node {
                    kind: NodeKind::Bytes,
                    start: token.start,
                    end: token.end,
                    children: Vec::new(),
                    text: token.lexeme(self.source).to_string(),
                })
            }
            KernelTokenKind::RecordClose => Err(KernelError::UnexpectedClose {
                closer: ')',
                position: token.start,
            }),
            KernelTokenKind::VectorClose => Err(KernelError::UnexpectedClose {
                closer: ']',
                position: token.start,
            }),
            KernelTokenKind::MapClose => Err(KernelError::UnexpectedClose {
                closer: '}',
                position: token.start,
            }),
            KernelTokenKind::LineComment => {
                self.cursor += 1;
                self.parse_one()
            }
        }
    }

    fn parse_record(&mut self, start: usize) -> Result<Node, KernelError> {
        self.cursor += 1; // consume `(`
        let mut children = Vec::new();
        loop {
            self.skip_comments();
            let token = self
                .tokens
                .get(self.cursor)
                .ok_or(KernelError::UnclosedDelimiter {
                    opener: '(',
                    position: start,
                })?;
            if token.kind == KernelTokenKind::RecordClose {
                let end = token.end;
                self.cursor += 1;
                return Ok(Node {
                    kind: NodeKind::Record,
                    start,
                    end,
                    children,
                    text: String::new(),
                });
            }
            children.push(self.parse_one()?);
        }
    }

    fn parse_vector(&mut self, start: usize) -> Result<Node, KernelError> {
        self.cursor += 1;
        let mut children = Vec::new();
        loop {
            self.skip_comments();
            let token = self
                .tokens
                .get(self.cursor)
                .ok_or(KernelError::UnclosedDelimiter {
                    opener: '[',
                    position: start,
                })?;
            if token.kind == KernelTokenKind::VectorClose {
                let end = token.end;
                self.cursor += 1;
                return Ok(Node {
                    kind: NodeKind::Vector,
                    start,
                    end,
                    children,
                    text: String::new(),
                });
            }
            children.push(self.parse_one()?);
        }
    }

    fn parse_map(&mut self, start: usize) -> Result<Node, KernelError> {
        self.cursor += 1;
        let mut children = Vec::new();
        loop {
            self.skip_comments();
            let token = self
                .tokens
                .get(self.cursor)
                .ok_or(KernelError::UnclosedDelimiter {
                    opener: '{',
                    position: start,
                })?;
            if token.kind == KernelTokenKind::MapClose {
                let end = token.end;
                self.cursor += 1;
                return Ok(Node {
                    kind: NodeKind::Map,
                    start,
                    end,
                    children,
                    text: String::new(),
                });
            }
            children.push(self.parse_one()?);
        }
    }

    fn peek(&self) -> Option<&KernelToken> {
        self.tokens.get(self.cursor)
    }

    fn skip_comments(&mut self) {
        while let Some(token) = self.tokens.get(self.cursor) {
            if token.kind == KernelTokenKind::LineComment {
                self.cursor += 1;
            } else {
                break;
            }
        }
    }
}
