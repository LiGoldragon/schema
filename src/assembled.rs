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

    // DESIGN-DECISION-REVIEW (second-designer/172 §3.2): routes_by_engine surfaces
    // the engine annotation that previously lived only on the namespace-side
    // Variant. Engine-driven dispatch codegen (per /324 §3.1) now has a stable
    // accessor instead of having to chase variants through declaration bodies.
    pub fn routes_by_engine(&self, engine: Engine) -> impl Iterator<Item = &Route> {
        self.routes
            .iter()
            .filter(move |route| route.engine() == Some(engine))
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

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2): Route now carries
// engine: Option<Engine>. The engine annotation is sourced from the
// namespace-side Variant during endpoint_body resolution (see
// document::Schema::lower_header). Without this field the macro library
// has no way to ask "which routes are assert-engine?" — the information
// would otherwise be locked inside DeclarationBody::Enum::Variant.engine.
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
