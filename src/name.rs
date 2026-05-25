use std::fmt;
use std::str::FromStr;

use crate::error::{Error, Result};

/// PascalCase schema name used for declarations and variants.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Name(String);

impl Name {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if is_pascal_case_name(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidName { name: value })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl nota_codec::NotaMapKey for Name {
    fn as_map_key(&self) -> &str {
        self.as_str()
    }

    fn from_map_key(text: String) -> nota_codec::Result<Self> {
        Self::new(text).map_err(|error| nota_codec::Error::Validation {
            type_name: "Name",
            message: error.to_string(),
        })
    }
}

/// Rust-facing field name derived from a schema field's type expression.
///
/// Authored schema fields do not carry direct lowercase names. When a field
/// needs a more specific generated name, the schema names the type more
/// specifically and the field name is derived from that PascalCase type.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldName(String);

impl FieldName {
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        if is_field_name(&value) {
            Ok(Self(value))
        } else {
            Err(Error::InvalidFieldName { name: value })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for FieldName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

fn is_pascal_case_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_uppercase()
        && chars.all(|character| character.is_ascii_alphanumeric())
        && value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}

fn is_field_name(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    first.is_ascii_lowercase()
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        && value
            .chars()
            .any(|character| character.is_ascii_alphabetic())
}
