use std::collections::BTreeMap;

use crate::{Error, Name, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Imports {
    entries: BTreeMap<Name, ImportDirective>,
}

impl Imports {
    pub fn empty() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    pub fn new(entries: Vec<(Name, ImportDirective)>) -> Result<Self> {
        let mut map = BTreeMap::new();
        for (binding, directive) in entries {
            if map.insert(binding.clone(), directive).is_some() {
                return Err(Error::DuplicateImportBinding { binding });
            }
        }
        Ok(Self { entries: map })
    }

    pub fn entries(&self) -> impl Iterator<Item = (&Name, &ImportDirective)> {
        self.entries.iter()
    }

    pub fn directive(&self, binding: &Name) -> Option<&ImportDirective> {
        self.entries.get(binding)
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for Imports {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportDirective {
    Import { path: SchemaPath, names: Vec<Name> },
    ImportAll { path: SchemaPath },
}

impl ImportDirective {
    pub fn import(path: SchemaPath, names: Vec<Name>) -> Self {
        Self::Import { path, names }
    }

    pub fn import_all(path: SchemaPath) -> Self {
        Self::ImportAll { path }
    }

    pub fn path(&self) -> &SchemaPath {
        match self {
            Self::Import { path, .. } | Self::ImportAll { path } => path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SchemaPath(String);

impl SchemaPath {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportResolution {
    binding: Name,
    names: Vec<Name>,
}

impl ImportResolution {
    pub fn new(binding: Name, names: Vec<Name>) -> Result<Self> {
        let mut seen = BTreeMap::new();
        for name in &names {
            if seen.insert(name.clone(), ()).is_some() {
                return Err(Error::DuplicateResolvedImportName {
                    binding: binding.clone(),
                    name: name.clone(),
                });
            }
        }
        Ok(Self { binding, names })
    }

    pub fn binding(&self) -> &Name {
        &self.binding
    }

    pub fn names(&self) -> &[Name] {
        &self.names
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportBinding {
    binding: Name,
    path: SchemaPath,
    names: ImportedNames,
}

impl ImportBinding {
    pub fn new(binding: Name, path: SchemaPath, names: ImportedNames) -> Self {
        Self {
            binding,
            path,
            names,
        }
    }

    pub fn binding(&self) -> &Name {
        &self.binding
    }

    pub fn path(&self) -> &SchemaPath {
        &self.path
    }

    pub fn names(&self) -> &ImportedNames {
        &self.names
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportedNames {
    All(Vec<Name>),
    Selected(Vec<Name>),
}

impl ImportedNames {
    pub fn names(&self) -> &[Name] {
        match self {
            Self::All(names) | Self::Selected(names) => names,
        }
    }
}
