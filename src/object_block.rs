use crate::{Error, ModuleName, ObjectDelimiter, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaBlockPass {
    namespace_prefix: ModuleName,
    roots: Vec<SchemaBlockObject>,
}

impl SchemaBlockPass {
    pub fn parse_text(namespace_prefix: ModuleName, text: &str) -> Result<Self> {
        let mut scanner = BlockScanner::new(text);
        let roots = scanner.parse_roots()?;
        Ok(Self {
            namespace_prefix,
            roots,
        })
    }

    pub fn namespace_prefix(&self) -> &ModuleName {
        &self.namespace_prefix
    }

    pub fn roots(&self) -> &[SchemaBlockObject] {
        &self.roots
    }

    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    pub fn holds_single_root_object(&self) -> bool {
        self.root_count() == 1
    }

    pub fn holds_two_root_objects(&self) -> bool {
        self.root_count() == 2
    }

    pub fn root(&self, index: usize) -> Option<&SchemaBlockObject> {
        self.roots.get(index)
    }

    pub fn single_root(&self) -> Option<&SchemaBlockObject> {
        if self.holds_single_root_object() {
            self.roots.first()
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaBlockObject {
    Block(SchemaBlock),
    Atom(SchemaAtom),
}

impl SchemaBlockObject {
    pub fn span(&self) -> &SourceSpan {
        match self {
            Self::Block(block) => block.span(),
            Self::Atom(atom) => atom.span(),
        }
    }

    pub fn as_block(&self) -> Option<&SchemaBlock> {
        match self {
            Self::Block(block) => Some(block),
            Self::Atom(_) => None,
        }
    }

    pub fn as_atom(&self) -> Option<&SchemaAtom> {
        match self {
            Self::Atom(atom) => Some(atom),
            Self::Block(_) => None,
        }
    }

    pub fn is_parenthesis_block(&self) -> bool {
        self.as_block()
            .is_some_and(SchemaBlock::is_parenthesis_block)
    }

    pub fn is_square_bracket_block(&self) -> bool {
        self.as_block()
            .is_some_and(SchemaBlock::is_square_bracket_block)
    }

    pub fn is_curly_brace_block(&self) -> bool {
        self.as_block()
            .is_some_and(SchemaBlock::is_curly_brace_block)
    }

    pub fn qualifies_as_symbol(&self) -> bool {
        self.as_atom().is_some_and(SchemaAtom::qualifies_as_symbol)
    }

    pub fn symbol_text(&self) -> Option<&str> {
        self.as_atom()
            .filter(|atom| atom.qualifies_as_symbol())
            .map(SchemaAtom::text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaBlock {
    delimiter: ObjectDelimiter,
    span: SourceSpan,
    objects: Vec<SchemaBlockObject>,
    block_string: bool,
}

impl SchemaBlock {
    fn new(delimiter: ObjectDelimiter, span: SourceSpan, objects: Vec<SchemaBlockObject>) -> Self {
        Self {
            delimiter,
            span,
            objects,
            block_string: false,
        }
    }

    fn block_string(span: SourceSpan) -> Self {
        Self {
            delimiter: ObjectDelimiter::SquareBrackets,
            span,
            objects: Vec::new(),
            block_string: true,
        }
    }

    pub fn delimiter(&self) -> ObjectDelimiter {
        self.delimiter
    }

    pub fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn objects(&self) -> &[SchemaBlockObject] {
        &self.objects
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn object(&self, index: usize) -> Option<&SchemaBlockObject> {
        self.objects.get(index)
    }

    pub fn second_object(&self) -> Option<&SchemaBlockObject> {
        self.object(1)
    }

    pub fn holds_object_count(&self, count: usize) -> bool {
        self.object_count() == count
    }

    pub fn holds_single_root_object(&self) -> bool {
        self.holds_object_count(1)
    }

    pub fn holds_two_root_objects(&self) -> bool {
        self.holds_object_count(2)
    }

    pub fn is_parenthesis_block(&self) -> bool {
        self.delimiter == ObjectDelimiter::Parentheses
    }

    pub fn is_square_bracket_block(&self) -> bool {
        self.delimiter == ObjectDelimiter::SquareBrackets
    }

    pub fn is_curly_brace_block(&self) -> bool {
        self.delimiter == ObjectDelimiter::CurlyBraces
    }

    pub fn is_block_string(&self) -> bool {
        self.block_string
    }

    pub fn second_object_is_square_bracket_block(&self) -> bool {
        self.second_object()
            .is_some_and(SchemaBlockObject::is_square_bracket_block)
    }

    pub fn second_object_qualifies_as_symbol(&self) -> bool {
        self.second_object()
            .is_some_and(SchemaBlockObject::qualifies_as_symbol)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaAtom {
    text: String,
    span: SourceSpan,
}

impl SchemaAtom {
    fn new(text: String, span: SourceSpan) -> Self {
        Self { text, span }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self) -> &SourceSpan {
        &self.span
    }

    pub fn qualifies_as_symbol(&self) -> bool {
        let mut chars = self.text.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        (first.is_ascii_alphabetic() || first == '_')
            && chars.all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSpan {
    start: SourcePosition,
    end: SourcePosition,
}

impl SourceSpan {
    fn new(start: SourcePosition, end: SourcePosition) -> Self {
        Self { start, end }
    }

    pub fn start(&self) -> SourcePosition {
        self.start
    }

    pub fn end(&self) -> SourcePosition {
        self.end
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourcePosition {
    byte: usize,
    line: usize,
    column: usize,
}

impl SourcePosition {
    fn new(byte: usize, line: usize, column: usize) -> Self {
        Self { byte, line, column }
    }

    pub fn byte(&self) -> usize {
        self.byte
    }

    pub fn line(&self) -> usize {
        self.line
    }

    pub fn column(&self) -> usize {
        self.column
    }
}

struct BlockScanner<'input> {
    input: &'input str,
    byte: usize,
    line: usize,
    column: usize,
}

impl<'input> BlockScanner<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            input,
            byte: 0,
            line: 1,
            column: 1,
        }
    }

    fn parse_roots(&mut self) -> Result<Vec<SchemaBlockObject>> {
        self.parse_until(None)
    }

    fn parse_until(&mut self, closing: Option<char>) -> Result<Vec<SchemaBlockObject>> {
        let mut objects = Vec::new();
        loop {
            self.skip_layout();
            let Some(character) = self.peek_char() else {
                if let Some(closing) = closing {
                    return Err(
                        self.error(format!("missing closing `{closing}` for delimiter block"))
                    );
                }
                return Ok(objects);
            };

            if Some(character) == closing {
                self.advance_char();
                return Ok(objects);
            }

            if self.is_closing_delimiter(character) {
                return Err(self.error(format!("unexpected closing delimiter `{character}`")));
            }

            objects.push(self.parse_object()?);
        }
    }

    fn parse_object(&mut self) -> Result<SchemaBlockObject> {
        match self.peek_char() {
            Some('(') => self.parse_block('(', ')', ObjectDelimiter::Parentheses),
            Some('[') => self.parse_square_bracket_block(),
            Some('{') => self.parse_block('{', '}', ObjectDelimiter::CurlyBraces),
            Some(_) => Ok(SchemaBlockObject::Atom(self.parse_atom()?)),
            None => Err(self.error("unexpected end of input while parsing object")),
        }
    }

    fn parse_block(
        &mut self,
        opening: char,
        closing: char,
        delimiter: ObjectDelimiter,
    ) -> Result<SchemaBlockObject> {
        let start = self.position();
        self.expect_char(opening)?;
        let objects = self.parse_until(Some(closing))?;
        Ok(SchemaBlockObject::Block(SchemaBlock::new(
            delimiter,
            SourceSpan::new(start, self.position()),
            objects,
        )))
    }

    fn parse_square_bracket_block(&mut self) -> Result<SchemaBlockObject> {
        let start = self.position();
        self.expect_char('[')?;
        if self.peek_char() == Some('|') {
            self.advance_char();
            self.consume_block_string()?;
            return Ok(SchemaBlockObject::Block(SchemaBlock::block_string(
                SourceSpan::new(start, self.position()),
            )));
        }

        let objects = self.parse_until(Some(']'))?;
        Ok(SchemaBlockObject::Block(SchemaBlock::new(
            ObjectDelimiter::SquareBrackets,
            SourceSpan::new(start, self.position()),
            objects,
        )))
    }

    fn parse_atom(&mut self) -> Result<SchemaAtom> {
        let start = self.position();
        let start_byte = self.byte;
        while let Some(character) = self.peek_char() {
            if character.is_whitespace()
                || self.is_opening_delimiter(character)
                || self.is_closing_delimiter(character)
                || self.starts_comment()
            {
                break;
            }
            self.advance_char();
        }

        if self.byte == start_byte {
            return Err(self.error("expected atom"));
        }

        Ok(SchemaAtom::new(
            self.input[start_byte..self.byte].to_owned(),
            SourceSpan::new(start, self.position()),
        ))
    }

    fn consume_block_string(&mut self) -> Result<()> {
        loop {
            if self.starts_with("|]") {
                self.advance_char();
                self.advance_char();
                return Ok(());
            }
            if self.peek_char().is_none() {
                return Err(self.error("unterminated `[|...|]` block string"));
            }
            self.advance_char();
        }
    }

    fn skip_layout(&mut self) {
        loop {
            while self.peek_char().is_some_and(char::is_whitespace) {
                self.advance_char();
            }

            if self.starts_comment() {
                while let Some(character) = self.peek_char() {
                    self.advance_char();
                    if character == '\n' {
                        break;
                    }
                }
                continue;
            }

            return;
        }
    }

    fn expect_char(&mut self, expected: char) -> Result<()> {
        match self.peek_char() {
            Some(character) if character == expected => {
                self.advance_char();
                Ok(())
            }
            Some(character) => Err(self.error(format!("expected `{expected}`, got `{character}`"))),
            None => Err(self.error(format!("expected `{expected}`, got end of input"))),
        }
    }

    fn position(&self) -> SourcePosition {
        SourcePosition::new(self.byte, self.line, self.column)
    }

    fn peek_char(&self) -> Option<char> {
        self.input[self.byte..].chars().next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let character = self.peek_char()?;
        self.byte += character.len_utf8();
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    fn starts_with(&self, value: &str) -> bool {
        self.input[self.byte..].starts_with(value)
    }

    fn starts_comment(&self) -> bool {
        self.starts_with(";;")
    }

    fn is_opening_delimiter(&self, character: char) -> bool {
        matches!(character, '(' | '[' | '{')
    }

    fn is_closing_delimiter(&self, character: char) -> bool {
        matches!(character, ')' | ']' | '}')
    }

    fn error(&self, message: impl Into<String>) -> Error {
        Error::InvalidSchemaText {
            context: "schema object block pass",
            message: format!(
                "{} at line {}, column {}",
                message.into(),
                self.line,
                self.column
            ),
        }
    }
}
