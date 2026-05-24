use std::collections::BTreeMap;

use crate::{Declaration, DeclarationBody, Error, Name, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Namespace {
    entries: BTreeMap<Name, DeclarationBody>,
}

impl Namespace {
    pub fn new(entries: Vec<(Name, DeclarationBody)>) -> Result<Self> {
        let mut map = BTreeMap::new();
        for (name, body) in entries {
            if map.insert(name.clone(), body).is_some() {
                return Err(Error::DuplicateDeclaration { name });
            }
        }
        Ok(Self { entries: map })
    }

    pub fn declarations(declarations: Vec<Declaration>) -> Result<Self> {
        Self::new(
            declarations
                .into_iter()
                .map(|declaration| (declaration.name().clone(), declaration.body().clone()))
                .collect(),
        )
    }

    pub fn entries(&self) -> impl Iterator<Item = (&Name, &DeclarationBody)> {
        self.entries.iter()
    }

    pub fn names(&self) -> impl Iterator<Item = &Name> {
        self.entries.keys()
    }

    pub fn body(&self, name: &Name) -> Option<&DeclarationBody> {
        self.entries.get(name)
    }
}
