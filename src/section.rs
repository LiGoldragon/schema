use std::collections::BTreeMap;

use crate::{Declaration, DeclarationBody, Error, Name, Reference, Result, Variant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Section {
    Messaging(Vec<Declaration>),
    Namespace(Namespace),
}

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

    pub fn local(entries: Vec<(Name, Vec<Variant>)>) -> Result<Self> {
        Self::new(
            entries
                .into_iter()
                .map(|(name, variants)| (name, DeclarationBody::Local { variants }))
                .collect(),
        )
    }

    pub fn references(entries: Vec<(Name, Reference)>) -> Result<Self> {
        Self::new(
            entries
                .into_iter()
                .map(|(name, reference)| (name, DeclarationBody::Reference(reference)))
                .collect(),
        )
    }

    pub fn entries(&self) -> impl Iterator<Item = (&Name, &DeclarationBody)> {
        self.entries.iter()
    }

    pub fn body(&self, name: &Name) -> Option<&DeclarationBody> {
        self.entries.get(name)
    }
}
