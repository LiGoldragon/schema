//! Precompiled schema library.
//!
//! Per records 742, 749: the workspace ships a library of precompiled
//! schemas with a standard core namespace always implicitly loaded.
//! Other schemas load as needed. Precompilation means the schemas
//! live as in-memory namespace tables, not re-parsed at every
//! interpretation site.
//!
//! The prototype models the library as a struct holding:
//!   - `core` — the always-loaded foundational schema (nota.schema)
//!   - `loaded` — a name → AssembledSchema map of per-component
//!     schemas loaded on demand.
//!
//! A full implementation would back this with the schema daemon
//! (record 750) for cross-session caching; the prototype keeps it
//! in-process.

use crate::schema::{AssembledSchema, SchemaError};
use core::fmt;
use std::collections::BTreeMap;

#[derive(Debug)]
pub enum LibraryError {
    Schema(SchemaError),
    NotLoaded { schema_name: String },
    AlreadyLoaded { schema_name: String },
}

impl fmt::Display for LibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Schema(error) => write!(formatter, "schema: {error}"),
            LibraryError::NotLoaded { schema_name } => {
                write!(formatter, "schema `{schema_name}` not loaded")
            }
            LibraryError::AlreadyLoaded { schema_name } => {
                write!(formatter, "schema `{schema_name}` already loaded")
            }
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<SchemaError> for LibraryError {
    fn from(error: SchemaError) -> Self {
        LibraryError::Schema(error)
    }
}

pub struct Library {
    core: AssembledSchema,
    loaded: BTreeMap<String, AssembledSchema>,
}

impl Library {
    /// Construct a library with the given source as the foundational
    /// (always implicitly loaded) core schema. In production the core
    /// would be `nota.schema` plus a core-macros schema; the prototype
    /// uses just `nota.schema`.
    pub fn with_core(core_source: &str) -> Result<Self, LibraryError> {
        let core = AssembledSchema::read(core_source)?;
        Ok(Self {
            core,
            loaded: BTreeMap::new(),
        })
    }

    /// The always-loaded core. Every name lookup falls through to
    /// the core if the per-schema namespace doesn't carry it.
    pub fn core(&self) -> &AssembledSchema {
        &self.core
    }

    /// Load a per-component schema. The name should be the schema
    /// file's stem (e.g. `spirit` for `spirit.schema`).
    pub fn load(
        &mut self,
        schema_name: &str,
        source: &str,
    ) -> Result<&AssembledSchema, LibraryError> {
        if self.loaded.contains_key(schema_name) {
            return Err(LibraryError::AlreadyLoaded {
                schema_name: schema_name.to_string(),
            });
        }
        let assembled = AssembledSchema::read(source)?;
        self.loaded.insert(schema_name.to_string(), assembled);
        Ok(self.loaded.get(schema_name).expect("just inserted"))
    }

    /// Look up a loaded per-component schema by name.
    pub fn get(&self, schema_name: &str) -> Option<&AssembledSchema> {
        self.loaded.get(schema_name)
    }

    /// Resolve a typename, checking the schema first then the core.
    /// This is the namespace-fallthrough rule (core is always
    /// implicitly available).
    pub fn resolve<'library>(
        &'library self,
        schema_name: &str,
        type_name: &str,
    ) -> Option<&'library crate::schema::NamespaceEntry> {
        let component_hit = self
            .loaded
            .get(schema_name)
            .and_then(|schema| schema.lookup(type_name));
        component_hit.or_else(|| self.core.lookup(type_name))
    }

    /// List of currently-loaded per-component schema names.
    pub fn loaded_names(&self) -> Vec<&str> {
        self.loaded.keys().map(String::as_str).collect()
    }
}
