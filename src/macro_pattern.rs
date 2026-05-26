use crate::{ObjectDelimiter, SchemaBlockObject, SymbolClass};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaMacroMatcher {
    name: String,
    pattern: SchemaMacroPattern,
}

impl SchemaMacroMatcher {
    pub fn new(name: impl Into<String>, pattern: SchemaMacroPattern) -> Self {
        Self {
            name: name.into(),
            pattern,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn pattern(&self) -> &SchemaMacroPattern {
        &self.pattern
    }

    pub fn matches(&self, object: &SchemaBlockObject) -> bool {
        self.pattern.matches(object)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SchemaMacroPattern {
    Any,
    Symbol,
    SymbolClass(SymbolClass),
    BlockWithAnyObjects {
        delimiter: ObjectDelimiter,
    },
    Block {
        delimiter: ObjectDelimiter,
        objects: Vec<SchemaMacroPattern>,
    },
}

impl SchemaMacroPattern {
    pub fn any() -> Self {
        Self::Any
    }

    pub fn symbol() -> Self {
        Self::Symbol
    }

    pub fn symbol_class(class: SymbolClass) -> Self {
        Self::SymbolClass(class)
    }

    pub fn pascal_symbol() -> Self {
        Self::symbol_class(SymbolClass::PascalCase)
    }

    pub fn camel_symbol() -> Self {
        Self::symbol_class(SymbolClass::CamelCase)
    }

    pub fn kebab_symbol() -> Self {
        Self::symbol_class(SymbolClass::KebabCase)
    }

    pub fn parenthesized(objects: Vec<Self>) -> Self {
        Self::block(ObjectDelimiter::Parentheses, objects)
    }

    pub fn square_bracketed(objects: Vec<Self>) -> Self {
        Self::block(ObjectDelimiter::SquareBrackets, objects)
    }

    pub fn curly_braced(objects: Vec<Self>) -> Self {
        Self::block(ObjectDelimiter::CurlyBraces, objects)
    }

    pub fn block(delimiter: ObjectDelimiter, objects: Vec<Self>) -> Self {
        Self::Block { delimiter, objects }
    }

    pub fn parenthesized_any() -> Self {
        Self::block_with_any_objects(ObjectDelimiter::Parentheses)
    }

    pub fn square_bracketed_any() -> Self {
        Self::block_with_any_objects(ObjectDelimiter::SquareBrackets)
    }

    pub fn curly_braced_any() -> Self {
        Self::block_with_any_objects(ObjectDelimiter::CurlyBraces)
    }

    pub fn block_with_any_objects(delimiter: ObjectDelimiter) -> Self {
        Self::BlockWithAnyObjects { delimiter }
    }

    pub fn matches(&self, object: &SchemaBlockObject) -> bool {
        match self {
            Self::Any => true,
            Self::Symbol => object.qualified_symbol().is_some(),
            Self::SymbolClass(expected) => object
                .qualified_symbol()
                .is_some_and(|symbol| symbol.class() == *expected),
            Self::BlockWithAnyObjects { delimiter } => object
                .as_block()
                .is_some_and(|block| block.delimiter() == *delimiter),
            Self::Block { delimiter, objects } => {
                let Some(block) = object.as_block() else {
                    return false;
                };
                if block.delimiter() != *delimiter || block.object_count() != objects.len() {
                    return false;
                }
                block
                    .objects()
                    .iter()
                    .zip(objects)
                    .all(|(child, pattern)| pattern.matches(child))
            }
        }
    }
}
