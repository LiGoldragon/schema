use std::fmt;

use crate::Name;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidName {
        name: String,
    },
    InvalidSchemaText {
        context: &'static str,
        message: String,
    },
    SchemaReadFailed {
        path: String,
        message: String,
    },
    SchemaImportCycle {
        path: String,
    },
    MissingImportedName {
        binding: Name,
        name: Name,
        path: String,
    },
    DuplicateDeclaration {
        name: Name,
    },
    DuplicateVariant {
        declaration: Name,
        variant: Name,
    },
    EmptyHeaderRoot {
        name: Name,
    },
    DuplicateHeaderRoot {
        name: Name,
    },
    DuplicateHeaderEndpoint {
        root: Name,
        endpoint: Name,
    },
    DuplicateImportBinding {
        binding: Name,
    },
    DuplicateImportedName {
        name: Name,
        first_binding: Name,
        second_binding: Name,
    },
    ImportCollisionWithLocal {
        name: Name,
        binding: Name,
    },
    MissingImportResolution {
        binding: Name,
    },
    UnknownImportResolution {
        binding: Name,
    },
    DuplicateResolvedImportName {
        binding: Name,
        name: Name,
    },
    UnknownType {
        name: Name,
    },
    MissingDeclaration {
        name: Name,
    },
    MissingVariant {
        declaration: Name,
        variant: Name,
    },
    MissingRouteBody {
        root: Name,
        endpoint: Name,
    },
    UnmatchedRouteBodyVariant {
        root: Name,
        variant: Name,
    },
    InvalidRouteBody {
        root: Name,
        endpoint: Name,
        reason: String,
    },
    ShortHeaderSlotOverflow {
        root: Name,
        endpoint: Option<Name>,
        slot: usize,
    },
    MissingUpgradeAnnotation {
        name: Name,
    },
    RemovedTypeRequiresAnnotation {
        name: Name,
    },
    DuplicateUpgradeAnnotation {
        name: Name,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { name } => write!(formatter, "invalid schema name `{name}`"),
            Self::InvalidSchemaText { context, message } => {
                write!(
                    formatter,
                    "invalid schema text while parsing {context}: {message}"
                )
            }
            Self::SchemaReadFailed { path, message } => {
                write!(formatter, "failed to read schema file `{path}`: {message}")
            }
            Self::SchemaImportCycle { path } => {
                write!(formatter, "schema import cycle at `{path}`")
            }
            Self::MissingImportedName {
                binding,
                name,
                path,
            } => {
                write!(
                    formatter,
                    "import binding `{binding}` selected `{name}`, but `{path}` does not export it"
                )
            }
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
            Self::EmptyHeaderRoot { name } => {
                write!(formatter, "header root `{name}` has no sub-variants")
            }
            Self::DuplicateHeaderRoot { name } => {
                write!(formatter, "duplicate header root `{name}`")
            }
            Self::DuplicateHeaderEndpoint { root, endpoint } => {
                write!(
                    formatter,
                    "duplicate endpoint `{endpoint}` in header root `{root}`"
                )
            }
            Self::DuplicateImportBinding { binding } => {
                write!(formatter, "duplicate import binding `{binding}`")
            }
            Self::DuplicateImportedName {
                name,
                first_binding,
                second_binding,
            } => {
                write!(
                    formatter,
                    "duplicate imported name `{name}` from bindings `{first_binding}` and `{second_binding}`"
                )
            }
            Self::ImportCollisionWithLocal { name, binding } => {
                write!(
                    formatter,
                    "imported name `{name}` from binding `{binding}` collides with a local declaration"
                )
            }
            Self::MissingImportResolution { binding } => {
                write!(
                    formatter,
                    "missing resolved names for import-all binding `{binding}`"
                )
            }
            Self::UnknownImportResolution { binding } => {
                write!(
                    formatter,
                    "import resolution names unknown binding `{binding}`"
                )
            }
            Self::DuplicateResolvedImportName { binding, name } => {
                write!(
                    formatter,
                    "duplicate resolved name `{name}` for import-all binding `{binding}`"
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
            Self::MissingRouteBody { root, endpoint } => {
                write!(
                    formatter,
                    "missing route body for `{root}.{endpoint}` in namespace"
                )
            }
            Self::UnmatchedRouteBodyVariant { root, variant } => {
                write!(
                    formatter,
                    "route body variant `{variant}` under `{root}` is not present in the header"
                )
            }
            Self::InvalidRouteBody {
                root,
                endpoint,
                reason,
            } => {
                write!(
                    formatter,
                    "invalid route body for `{root}.{endpoint}`: {reason}"
                )
            }
            Self::ShortHeaderSlotOverflow {
                root,
                endpoint,
                slot,
            } => match endpoint {
                Some(endpoint) => {
                    write!(
                        formatter,
                        "route `{root}.{endpoint}` slot {slot} does not fit in one short-header byte"
                    )
                }
                None => {
                    write!(
                        formatter,
                        "route root `{root}` slot {slot} does not fit in one short-header byte"
                    )
                }
            },
            Self::MissingUpgradeAnnotation { name } => {
                write!(
                    formatter,
                    "missing upgrade annotation for changed type `{name}`"
                )
            }
            Self::RemovedTypeRequiresAnnotation { name } => {
                write!(
                    formatter,
                    "removed type `{name}` requires Drop or Untranslatable annotation"
                )
            }
            Self::DuplicateUpgradeAnnotation { name } => {
                write!(formatter, "duplicate upgrade annotation for `{name}`")
            }
        }
    }
}

impl std::error::Error for Error {}
