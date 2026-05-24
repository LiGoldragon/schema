use std::collections::HashSet;

use crate::{Error, Name, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    roots: Vec<HeaderRoot>,
}

impl Header {
    pub fn empty() -> Self {
        Self { roots: Vec::new() }
    }

    pub fn new(roots: Vec<HeaderRoot>) -> Result<Self> {
        let mut names = HashSet::new();
        for root in &roots {
            if !names.insert(root.name()) {
                return Err(Error::DuplicateHeaderRoot {
                    name: root.name().clone(),
                });
            }
        }
        Ok(Self { roots })
    }

    pub fn roots(&self) -> &[HeaderRoot] {
        &self.roots
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}

impl Default for Header {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderRoot {
    name: Name,
    endpoints: Vec<Name>,
}

impl HeaderRoot {
    pub fn new(name: Name, endpoints: Vec<Name>) -> Result<Self> {
        if endpoints.is_empty() {
            return Err(Error::EmptyHeaderRoot { name });
        }
        let mut names = HashSet::new();
        for endpoint in &endpoints {
            if !names.insert(endpoint) {
                return Err(Error::DuplicateHeaderEndpoint {
                    root: name.clone(),
                    endpoint: endpoint.clone(),
                });
            }
        }
        Ok(Self { name, endpoints })
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn endpoints(&self) -> &[Name] {
        &self.endpoints
    }
}
