use std::path::Path;

use crate::{
    Name, RawDatatypeEntry, RawDatatypeMap, RawNotaDatatype, RawNotaSequence, RawSchemaFile,
    SchemaError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxSchema {
    root_name: Name,
    datatypes: Vec<SyntaxDatatype>,
}

impl SyntaxSchema {
    pub fn from_path_and_source(path: impl AsRef<Path>, source: &str) -> Result<Self, SchemaError> {
        let raw = RawSchemaFile::from_path_and_source(path, source)?;
        Self::from_raw(&raw)
    }

    pub fn from_raw(raw: &RawSchemaFile) -> Result<Self, SchemaError> {
        let mut datatypes = Vec::new();
        for entry in raw.datatypes().entries() {
            datatypes.push(SyntaxDatatype::from_raw_entry(entry)?);
        }
        Ok(Self {
            root_name: raw.root_name().clone(),
            datatypes,
        })
    }

    pub fn root_name(&self) -> &Name {
        &self.root_name
    }

    pub fn datatypes(&self) -> &[SyntaxDatatype] {
        &self.datatypes
    }

    pub fn datatype_named(&self, name: &str) -> Option<&SyntaxDatatype> {
        self.datatypes
            .iter()
            .find(|datatype| datatype.name.as_str() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxDatatype {
    name: Name,
    declaration: SyntaxDeclaration,
}

impl SyntaxDatatype {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn declaration(&self) -> &SyntaxDeclaration {
        &self.declaration
    }

    fn from_raw_entry(entry: &RawDatatypeEntry) -> Result<Self, SchemaError> {
        Ok(Self {
            name: entry.name().clone(),
            declaration: SyntaxDeclaration::from_named_raw(entry.name(), entry.datatype())?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxDeclaration {
    Alias(SyntaxReference),
    Text(String),
    Struct(SyntaxStructDeclaration),
    Enum(SyntaxEnumDeclaration),
}

impl SyntaxDeclaration {
    fn from_named_raw(name: &Name, raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        match raw {
            RawNotaDatatype::Atom(_) | RawNotaDatatype::Record(_) => {
                Ok(Self::Alias(SyntaxReference::from_raw(raw)?))
            }
            RawNotaDatatype::Text(text) => Ok(Self::Text(text.clone())),
            RawNotaDatatype::KeyValue(map) => {
                Ok(Self::Struct(SyntaxStructDeclaration::from_map(name, map)?))
            }
            RawNotaDatatype::Vector(sequence) => Ok(Self::Enum(
                SyntaxEnumDeclaration::from_vector(name.clone(), sequence)?,
            )),
            RawNotaDatatype::PipeParenthesis(_) | RawNotaDatatype::PipeBrace(_) => {
                Err(SchemaError::ExpectedSyntaxDeclaration {
                    found: raw.syntax_description(),
                })
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxStructDeclaration {
    name: Name,
    fields: Vec<SyntaxField>,
}

impl SyntaxStructDeclaration {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn fields(&self) -> &[SyntaxField] {
        &self.fields
    }

    pub fn is_newtype(&self) -> bool {
        self.fields.len() == 1
    }

    fn from_map(name: &Name, map: &RawDatatypeMap) -> Result<Self, SchemaError> {
        let mut fields = Vec::new();
        for (index, entry) in map.entries().iter().enumerate() {
            fields.push(SyntaxField::from_map_entry(
                index,
                entry.name(),
                entry.datatype(),
            )?);
        }
        Ok(Self {
            name: name.clone(),
            fields,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxField {
    name: Name,
    reference: SyntaxReference,
}

impl SyntaxField {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &SyntaxReference {
        &self.reference
    }

    fn from_map_entry(
        index: usize,
        name: &Name,
        reference_item: &RawNotaDatatype,
    ) -> Result<Self, SchemaError> {
        let reference = if reference_item.as_atom() == Some("*") {
            SyntaxReference::Plain(name.clone())
        } else {
            SyntaxReference::from_raw(reference_item)?
        };
        Ok(Self {
            name: Name::new(name.field_name()),
            reference: reference.with_positional_fallback(index),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxEnumDeclaration {
    name: Name,
    variants: Vec<SyntaxVariant>,
}

impl SyntaxEnumDeclaration {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn variants(&self) -> &[SyntaxVariant] {
        &self.variants
    }

    fn from_vector(name: Name, sequence: &RawNotaSequence) -> Result<Self, SchemaError> {
        Self::from_variant_items(name, sequence.items())
    }

    fn from_variant_items(name: Name, items: &[RawNotaDatatype]) -> Result<Self, SchemaError> {
        let mut variants = Vec::new();
        for item in items {
            variants.push(SyntaxVariant::from_raw(item)?);
        }
        Ok(Self { name, variants })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxVariant {
    name: Name,
    payload: Option<SyntaxReference>,
}

impl SyntaxVariant {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> Option<&SyntaxReference> {
        self.payload.as_ref()
    }

    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        if let Some(name) = raw.as_atom() {
            if Self::is_variant_symbol(name) {
                return Ok(Self {
                    name: Name::new(name),
                    payload: None,
                });
            }
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: raw.syntax_description(),
            });
        }

        let Some(sequence) = raw.as_record() else {
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: raw.syntax_description(),
            });
        };
        if sequence.items().len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "data-carrying enum variant",
                expected: "tag plus one payload object",
                found: sequence.items().len(),
            });
        }
        let Some(name) = sequence.items()[0].as_atom() else {
            return Err(SchemaError::ExpectedSymbol {
                found: sequence.items()[0].syntax_description(),
            });
        };
        if !Self::is_variant_symbol(name) {
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: sequence.items()[0].syntax_description(),
            });
        }
        Ok(Self {
            name: Name::new(name),
            payload: Some(SyntaxReference::from_raw(&sequence.items()[1])?),
        })
    }

    fn is_variant_symbol(value: &str) -> bool {
        value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
            && !value.contains('@')
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SyntaxReference {
    Plain(Name),
    Vector(Box<SyntaxReference>),
    Optional(Box<SyntaxReference>),
    Map(Box<SyntaxReference>, Box<SyntaxReference>),
    InlineStruct(SyntaxStructDeclaration),
    InlineEnum(SyntaxEnumDeclaration),
}

impl SyntaxReference {
    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        match raw {
            RawNotaDatatype::Atom(name) => Ok(Self::Plain(Name::new(name))),
            RawNotaDatatype::Record(sequence) => Self::from_record(sequence),
            RawNotaDatatype::Vector(_)
            | RawNotaDatatype::KeyValue(_)
            | RawNotaDatatype::PipeBrace(_)
            | RawNotaDatatype::PipeParenthesis(_)
            | RawNotaDatatype::Text(_) => Err(SchemaError::ExpectedSyntaxReference {
                found: raw.syntax_description(),
            }),
        }
    }

    fn from_record(sequence: &RawNotaSequence) -> Result<Self, SchemaError> {
        let items = sequence.items();
        if items.len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "typed reference record",
                expected: "tag plus one grouped payload object",
                found: items.len(),
            });
        }
        let Some(head) = items[0].as_atom() else {
            return Err(SchemaError::ExpectedSymbol {
                found: items[0].syntax_description(),
            });
        };
        match head {
            "Vec" | "Vector" => Ok(Self::Vector(Box::new(Self::from_raw(&items[1])?))),
            "Optional" | "Option" => Ok(Self::Optional(Box::new(Self::from_raw(&items[1])?))),
            "Map" | "KeyValue" => Self::from_map_record(&items[1]),
            _ => Err(SchemaError::ExpectedSyntaxReference {
                found: sequence.syntax_description(),
            }),
        }
    }

    fn from_map_record(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        let Some(sequence) = raw.as_record() else {
            return Err(SchemaError::ExpectedSyntaxReference {
                found: raw.syntax_description(),
            });
        };
        if sequence.items().len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "map reference payload",
                expected: "key type plus value type",
                found: sequence.items().len(),
            });
        }
        Ok(Self::Map(
            Box::new(Self::from_raw(&sequence.items()[0])?),
            Box::new(Self::from_raw(&sequence.items()[1])?),
        ))
    }

    fn with_positional_fallback(self, _index: usize) -> Self {
        self
    }
}

impl RawNotaDatatype {
    fn syntax_description(&self) -> String {
        match self {
            Self::Atom(text) => format!("atom {text}"),
            Self::Text(_) => "text".to_owned(),
            Self::Record(_) => "parenthesis record".to_owned(),
            Self::Vector(_) => "square-bracket vector".to_owned(),
            Self::KeyValue(_) => "brace key-value map".to_owned(),
            Self::PipeParenthesis(_) => "pipe-parenthesis enum declaration".to_owned(),
            Self::PipeBrace(_) => "pipe-brace struct declaration".to_owned(),
        }
    }
}

impl RawNotaSequence {
    fn syntax_description(&self) -> String {
        format!("parenthesis record with {} objects", self.items().len())
    }
}
