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
    KeyValue(SyntaxKeyValueDeclaration),
}

impl SyntaxDeclaration {
    fn from_named_raw(name: &Name, raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        match raw {
            RawNotaDatatype::Atom(_) => Ok(Self::Alias(SyntaxReference::from_raw(raw)?)),
            RawNotaDatatype::Text(text) => Ok(Self::Text(text.clone())),
            RawNotaDatatype::KeyValue(map) => Ok(Self::KeyValue(
                SyntaxKeyValueDeclaration::from_map(name.clone(), map)?,
            )),
            RawNotaDatatype::PipeParenthesis(sequence) => Ok(Self::Enum(
                SyntaxEnumDeclaration::from_self_named(name, sequence)?,
            )),
            RawNotaDatatype::PipeBrace(sequence) => Ok(Self::Struct(
                SyntaxStructDeclaration::from_self_named(name, sequence)?,
            )),
            RawNotaDatatype::Vector(_) | RawNotaDatatype::Record(_) => {
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

    fn from_self_named(
        expected_name: &Name,
        sequence: &RawNotaSequence,
    ) -> Result<Self, SchemaError> {
        let items = sequence.items();
        let declared_name = SyntaxDeclaredName::from_items(items)?;
        declared_name.must_match(expected_name)?;
        let mut fields = Vec::new();
        let mut index = 1;
        while index < items.len() {
            if let Some(field) = SyntaxField::from_binding(index - 1, &items[index])? {
                fields.push(field);
                index += 1;
                continue;
            }
            if let Some(field) = SyntaxField::from_explicit_pair(index - 1, &items[index])? {
                fields.push(field);
                index += 1;
                continue;
            }
            let Some(reference_item) = items.get(index + 1) else {
                fields.push(SyntaxField::from_derived_reference(
                    index - 1,
                    &items[index],
                )?);
                index += 1;
                continue;
            };
            fields.push(SyntaxField::from_named_pair(
                index - 1,
                &items[index],
                reference_item,
            )?);
            index += 2;
        }
        Ok(Self {
            name: expected_name.clone(),
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

    fn from_named_pair(
        index: usize,
        name_item: &RawNotaDatatype,
        reference_item: &RawNotaDatatype,
    ) -> Result<Self, SchemaError> {
        let Some(field_name) = name_item.as_atom() else {
            return Err(SchemaError::ExpectedSymbol {
                found: name_item.syntax_description(),
            });
        };
        let reference = SyntaxReference::from_raw(reference_item)?;
        Ok(Self {
            name: Name::new(field_name),
            reference: reference.with_positional_fallback(index),
        })
    }

    fn from_derived_reference(index: usize, item: &RawNotaDatatype) -> Result<Self, SchemaError> {
        let reference = SyntaxReference::from_raw(item)?.with_positional_fallback(index);
        let name = reference.derived_field_name();
        Ok(Self { name, reference })
    }

    fn from_binding(index: usize, item: &RawNotaDatatype) -> Result<Option<Self>, SchemaError> {
        let Some(text) = item.as_atom() else {
            return Ok(None);
        };
        let Some(binding) = SyntaxBinding::from_text(text) else {
            return Ok(None);
        };
        Ok(Some(Self {
            name: binding.field_name(),
            reference: SyntaxReference::Plain(binding.reference).with_positional_fallback(index),
        }))
    }

    fn from_explicit_pair(
        index: usize,
        item: &RawNotaDatatype,
    ) -> Result<Option<Self>, SchemaError> {
        let Some(sequence) = item.as_record() else {
            return Ok(None);
        };
        if sequence.items().len() != 2 {
            return Ok(None);
        }
        let Some(field_name) = sequence.items()[0].as_atom() else {
            return Ok(None);
        };
        if !field_name
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_lowercase())
        {
            return Ok(None);
        }
        Ok(Some(Self {
            name: Name::new(field_name),
            reference: SyntaxReference::from_raw(&sequence.items()[1])?
                .with_positional_fallback(index),
        }))
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

    fn from_self_named(
        expected_name: &Name,
        sequence: &RawNotaSequence,
    ) -> Result<Self, SchemaError> {
        let items = sequence.items();
        let declared_name = SyntaxDeclaredName::from_items(items)?;
        declared_name.must_match(expected_name)?;
        Self::from_variant_items(expected_name.clone(), &items[1..])
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
struct SyntaxBinding {
    name: Name,
    reference: Name,
    derives_name_from_reference: bool,
}

impl SyntaxBinding {
    fn field_name(&self) -> Name {
        if self.derives_name_from_reference {
            Name::new(self.name.field_name())
        } else {
            self.name.clone()
        }
    }

    fn from_text(text: &str) -> Option<Self> {
        if let Some(reference) = text.strip_prefix('@') {
            if reference.is_empty() || reference.contains('@') {
                return None;
            }
            let reference = Name::new(reference);
            return Some(Self {
                name: reference.clone(),
                reference,
                derives_name_from_reference: true,
            });
        }
        let (name, reference) = text.split_once('@')?;
        if name.is_empty() || reference.is_empty() || reference.contains('@') {
            return None;
        }
        Some(Self {
            name: Name::new(name),
            reference: Name::new(reference),
            derives_name_from_reference: false,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxKeyValueDeclaration {
    name: Name,
    entries: Vec<SyntaxKeyValueEntry>,
}

impl SyntaxKeyValueDeclaration {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn entries(&self) -> &[SyntaxKeyValueEntry] {
        &self.entries
    }

    fn from_map(name: Name, map: &RawDatatypeMap) -> Result<Self, SchemaError> {
        let mut entries = Vec::new();
        for entry in map.entries() {
            entries.push(SyntaxKeyValueEntry {
                key: entry.name().clone(),
                value: SyntaxReference::from_raw(entry.datatype())?,
            });
        }
        Ok(Self { name, entries })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SyntaxKeyValueEntry {
    key: Name,
    value: SyntaxReference,
}

impl SyntaxKeyValueEntry {
    pub fn key(&self) -> &Name {
        &self.key
    }

    pub fn value(&self) -> &SyntaxReference {
        &self.value
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
            RawNotaDatatype::PipeBrace(sequence) => {
                let name = SyntaxDeclaredName::from_items(sequence.items())?
                    .name()
                    .clone();
                Ok(Self::InlineStruct(
                    SyntaxStructDeclaration::from_self_named(&name, sequence)?,
                ))
            }
            RawNotaDatatype::PipeParenthesis(sequence) => {
                let name = SyntaxDeclaredName::from_items(sequence.items())?
                    .name()
                    .clone();
                Ok(Self::InlineEnum(SyntaxEnumDeclaration::from_self_named(
                    &name, sequence,
                )?))
            }
            RawNotaDatatype::Vector(_)
            | RawNotaDatatype::KeyValue(_)
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

    fn derived_field_name(&self) -> Name {
        match self {
            Self::Plain(name) => Name::new(name.field_name()),
            Self::Vector(inner) => Name::new(format!("{}_vector", inner.derived_field_name())),
            Self::Optional(inner) => Name::new(format!("optional_{}", inner.derived_field_name())),
            Self::Map(key, value) => Name::new(format!(
                "{}_by_{}",
                value.derived_field_name(),
                key.derived_field_name()
            )),
            Self::InlineStruct(declaration) => Name::new(declaration.name().field_name()),
            Self::InlineEnum(declaration) => Name::new(declaration.name().field_name()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SyntaxDeclaredName {
    name: Name,
}

impl SyntaxDeclaredName {
    fn from_items(items: &[RawNotaDatatype]) -> Result<Self, SchemaError> {
        let Some(first) = items.first() else {
            return Err(SchemaError::ExpectedRawDeclarationName {
                found: "empty declaration".to_owned(),
            });
        };
        let Some(name) = first.as_atom() else {
            return Err(SchemaError::ExpectedRawDeclarationName {
                found: first.syntax_description(),
            });
        };
        Ok(Self {
            name: Name::new(name),
        })
    }

    fn name(&self) -> &Name {
        &self.name
    }

    fn must_match(&self, expected_name: &Name) -> Result<(), SchemaError> {
        if self.name == *expected_name {
            return Ok(());
        }
        Err(SchemaError::RawDeclarationNameMismatch {
            key: expected_name.as_str().to_owned(),
            declared: self.name.as_str().to_owned(),
        })
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
