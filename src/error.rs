use std::fmt;

use crate::Name;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName { name: String },
    DuplicateDeclaration { name: Name },
    DuplicateVariant { declaration: Name, variant: Name },
    UnknownType { name: Name },
    MissingDeclaration { name: Name },
    MissingVariant { declaration: Name, variant: Name },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { name } => write!(formatter, "invalid schema name `{name}`"),
            Self::DuplicateDeclaration { name } => {
                write!(formatter, "duplicate declaration `{name}`")
            }
            Self::DuplicateVariant {
                declaration,
                variant,
            } => {
                write!(
                    formatter,
                    "duplicate variant `{variant}` in declaration `{declaration}`"
                )
            }
            Self::UnknownType { name } => write!(formatter, "unknown type `{name}`"),
            Self::MissingDeclaration { name } => write!(formatter, "missing declaration `{name}`"),
            Self::MissingVariant {
                declaration,
                variant,
            } => {
                write!(
                    formatter,
                    "missing variant `{variant}` in declaration `{declaration}`"
                )
            }
        }
    }
}

impl std::error::Error for Error {}
