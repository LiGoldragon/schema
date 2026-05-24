use std::collections::{BTreeMap, BTreeSet};

use crate::{
    DeclarationBody, Engine, Error, Feature, ImportBinding, Name, Projection, Result,
    StandardProjection, UpgradeAnnotation, UpgradePlan,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledSchema {
    imports: Vec<ImportBinding>,
    routes: Vec<Route>,
    types: BTreeMap<Name, AssembledType>,
    features: Vec<Feature>,
}

impl AssembledSchema {
    pub fn new(
        imports: Vec<ImportBinding>,
        routes: Vec<Route>,
        types: Vec<AssembledType>,
        features: Vec<Feature>,
    ) -> Self {
        Self {
            imports,
            routes,
            types: types
                .into_iter()
                .map(|schema_type| (schema_type.name().clone(), schema_type))
                .collect(),
            features,
        }
    }

    pub fn imports(&self) -> &[ImportBinding] {
        &self.imports
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
    }

    pub fn route_for_short_header(&self, leg: Leg, short_header: u64) -> Option<&Route> {
        let bytes = short_header.to_le_bytes();
        let root_slot = usize::from(bytes[0]);
        let endpoint_slot = usize::from(bytes[1]);
        self.routes.iter().find(|route| {
            route.leg == leg && route.root_slot == root_slot && route.endpoint.slot == endpoint_slot
        })
    }

    pub fn types(&self) -> impl Iterator<Item = &AssembledType> {
        self.types.values()
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn body(&self, name: &Name) -> Option<&DeclarationBody> {
        match self.types.get(name) {
            Some(AssembledType::Local { body, .. }) => Some(body),
            Some(AssembledType::Imported { .. }) | None => None,
        }
    }

    pub fn plan_upgrade_from(&self, previous: &Self) -> Result<UpgradePlan> {
        let annotations = self.upgrade_annotations()?;
        let mut projections = Vec::new();
        let mut renamed_previous = BTreeSet::new();

        for (name, body) in self.local_bodies() {
            if let Some(previous_body) = previous.body(name) {
                if body == previous_body {
                    projections.push(Projection::Identity { name: name.clone() });
                } else if standard_projection(previous_body, body).is_some() {
                    projections.push(Projection::Standard {
                        name: name.clone(),
                        kind: StandardProjection::AdditiveEnumVariant,
                    });
                } else if let Some(annotation) = annotations.get(name) {
                    projections.push(Projection::Annotated {
                        name: name.clone(),
                        annotation: (*annotation).clone(),
                    });
                } else {
                    return Err(Error::MissingUpgradeAnnotation { name: name.clone() });
                }
                continue;
            }

            if let Some(UpgradeAnnotation::RenamedFrom {
                previous: previous_name,
                ..
            }) = annotations.get(name)
            {
                if previous.body(previous_name).is_some() {
                    renamed_previous.insert(previous_name.clone());
                    projections.push(Projection::Renamed {
                        current: name.clone(),
                        previous: previous_name.clone(),
                    });
                    continue;
                }
            }

            projections.push(Projection::Added { name: name.clone() });
        }

        for (name, _) in previous.local_bodies() {
            if self.body(name).is_some() || renamed_previous.contains(name) {
                continue;
            }
            match annotations.get(name) {
                Some(UpgradeAnnotation::Drop(_)) => {
                    projections.push(Projection::Dropped { name: name.clone() });
                }
                Some(UpgradeAnnotation::Untranslatable(_)) => {
                    projections.push(Projection::Untranslatable { name: name.clone() });
                }
                _ => return Err(Error::RemovedTypeRequiresAnnotation { name: name.clone() }),
            }
        }

        Ok(UpgradePlan::new(projections))
    }

    fn local_bodies(&self) -> impl Iterator<Item = (&Name, &DeclarationBody)> {
        self.types.iter().filter_map(|(name, schema_type)| {
            if let AssembledType::Local { body, .. } = schema_type {
                Some((name, body))
            } else {
                None
            }
        })
    }

    fn upgrade_annotations(&self) -> Result<BTreeMap<Name, &UpgradeAnnotation>> {
        let mut annotations = BTreeMap::new();
        for feature in &self.features {
            let Feature::Upgrade(upgrade) = feature else {
                continue;
            };
            for annotation in upgrade.annotations() {
                let name = annotation.name();
                if annotations.insert(name.clone(), annotation).is_some() {
                    return Err(Error::DuplicateUpgradeAnnotation { name: name.clone() });
                }
            }
        }
        Ok(annotations)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssembledType {
    Local { name: Name, body: DeclarationBody },
    Imported { name: Name, binding: Name },
}

impl AssembledType {
    pub fn local(name: Name, body: DeclarationBody) -> Self {
        Self::Local { name, body }
    }

    pub fn imported(name: Name, binding: Name) -> Self {
        Self::Imported { name, binding }
    }

    pub fn name(&self) -> &Name {
        match self {
            Self::Local { name, .. } | Self::Imported { name, .. } => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    leg: Leg,
    root_slot: usize,
    root: Name,
    endpoint: Endpoint,
    body: RouteBody,
    engine: Option<Engine>,
}

impl Route {
    pub fn new(
        leg: Leg,
        root_slot: usize,
        root: Name,
        endpoint: Endpoint,
        body: RouteBody,
    ) -> Self {
        Self::with_engine(leg, root_slot, root, endpoint, body, None)
    }

    pub fn with_engine(
        leg: Leg,
        root_slot: usize,
        root: Name,
        endpoint: Endpoint,
        body: RouteBody,
        engine: Option<Engine>,
    ) -> Self {
        Self {
            leg,
            root_slot,
            root,
            endpoint,
            body,
            engine,
        }
    }

    pub fn leg(&self) -> Leg {
        self.leg
    }

    pub fn root_slot(&self) -> usize {
        self.root_slot
    }

    pub fn root(&self) -> &Name {
        &self.root
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn body(&self) -> &RouteBody {
        &self.body
    }

    pub fn engine(&self) -> Option<Engine> {
        self.engine
    }

    pub fn short_header(&self) -> Result<u64> {
        let root = u8::try_from(self.root_slot).map_err(|_| Error::ShortHeaderSlotOverflow {
            root: self.root.clone(),
            endpoint: None,
            slot: self.root_slot,
        })?;
        let endpoint =
            u8::try_from(self.endpoint.slot).map_err(|_| Error::ShortHeaderSlotOverflow {
                root: self.root.clone(),
                endpoint: Some(self.endpoint.name.clone()),
                slot: self.endpoint.slot,
            })?;
        Ok(u64::from_le_bytes([root, endpoint, 0, 0, 0, 0, 0, 0]))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Leg {
    Ordinary,
    Owner,
    Sema,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    slot: usize,
    name: Name,
}

impl Endpoint {
    pub fn new(slot: usize, name: Name) -> Self {
        Self { slot, name }
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn name(&self) -> &Name {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RouteBody {
    Type(Name),
    Unit,
}

fn standard_projection(
    previous: &DeclarationBody,
    current: &DeclarationBody,
) -> Option<StandardProjection> {
    let (
        DeclarationBody::Enum {
            variants: previous_variants,
        },
        DeclarationBody::Enum {
            variants: current_variants,
        },
    ) = (previous, current)
    else {
        return None;
    };

    if current_variants.len() <= previous_variants.len() {
        return None;
    }

    if current_variants
        .iter()
        .zip(previous_variants)
        .all(|(current, previous)| current == previous)
    {
        Some(StandardProjection::AdditiveEnumVariant)
    } else {
        None
    }
}
