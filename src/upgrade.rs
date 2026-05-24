use crate::Name;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version(String);

impl Version {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upgrade {
    from_version: Version,
    annotations: Vec<UpgradeAnnotation>,
}

impl Upgrade {
    pub fn new(from_version: Version, annotations: Vec<UpgradeAnnotation>) -> Self {
        Self {
            from_version,
            annotations,
        }
    }

    pub fn from_version(&self) -> &Version {
        &self.from_version
    }

    pub fn annotations(&self) -> &[UpgradeAnnotation] {
        &self.annotations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeAnnotation {
    Migrate(Name),
    RenamedFrom { current: Name, previous: Name },
    Drop(Name),
    Custom { name: Name, implementation: Name },
    Untranslatable(Name),
}

impl UpgradeAnnotation {
    pub fn name(&self) -> &Name {
        match self {
            Self::Migrate(name)
            | Self::RenamedFrom { current: name, .. }
            | Self::Drop(name)
            | Self::Custom { name, .. }
            | Self::Untranslatable(name) => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradePlan {
    projections: Vec<Projection>,
}

impl UpgradePlan {
    pub fn new(projections: Vec<Projection>) -> Self {
        Self { projections }
    }

    pub fn projections(&self) -> &[Projection] {
        &self.projections
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Projection {
    Identity {
        name: Name,
    },
    Standard {
        name: Name,
        kind: StandardProjection,
    },
    Annotated {
        name: Name,
        annotation: UpgradeAnnotation,
    },
    Added {
        name: Name,
    },
    Renamed {
        current: Name,
        previous: Name,
    },
    Dropped {
        name: Name,
    },
    Untranslatable {
        name: Name,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandardProjection {
    AdditiveEnumVariant,
}
