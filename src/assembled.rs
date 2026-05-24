use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::{
    DeclarationBody, Error, Feature, ImportBinding, Name, Projection, Result, StandardProjection,
    UpgradeAnnotation, UpgradePlan,
};

/// Reserved namespace label between the component and the type name in a UID
/// per intent 469 (`component::namespace::Type`).
const NAMESPACE_LABEL: &str = "namespace";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledSchema {
    component: Name,
    imports: Vec<ImportBinding>,
    routes: Vec<Route>,
    types: BTreeMap<Name, AssembledType>,
    features: Vec<Feature>,
    import_widths: BTreeMap<Name, bool>,
}

impl AssembledSchema {
    pub fn new(
        component: Name,
        imports: Vec<ImportBinding>,
        routes: Vec<Route>,
        types: Vec<AssembledType>,
        features: Vec<Feature>,
    ) -> Self {
        Self {
            component,
            imports,
            routes,
            types: types
                .into_iter()
                .map(|schema_type| (schema_type.name().clone(), schema_type))
                .collect(),
            features,
            import_widths: BTreeMap::new(),
        }
    }

    /// Attach fixed-width hints for imported types, returning a new
    /// AssembledSchema with the hints folded in. Per audit 171 §4.3 +
    /// recommendation §5(b): the layout planner uses these hints when
    /// classifying an imported `TypeExpression::Named` reference.
    ///
    /// Each map entry says: "the imported type `Name` is fixed-width (`true`)
    /// or variable-width (`false`)". Missing entries fall back to the
    /// conservative variable-width classification.
    pub fn with_import_widths(mut self, widths: BTreeMap<Name, bool>) -> Self {
        for (name, fixed) in widths {
            self.import_widths.insert(name, fixed);
        }
        self
    }

    pub fn component(&self) -> &Name {
        &self.component
    }

    /// Render the UID for a declared (local or imported) type as
    /// `component::namespace::TypeName` per intent 469. The type does not
    /// need to be declared in this schema — the Uid is built from the
    /// component name alone — but callers that pass an unknown name receive
    /// the same shape, on the assumption the caller knows what UID it wants.
    pub fn uid_for(&self, type_name: &Name) -> Uid {
        Uid::new(self.component.clone(), type_name.clone())
    }

    pub fn imports(&self) -> &[ImportBinding] {
        &self.imports
    }

    pub fn routes(&self) -> &[Route] {
        &self.routes
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

    /// Look up the declared type (local or imported) for a name, or None if
    /// the schema does not surface it.
    pub fn assembled_type(&self, name: &Name) -> Option<&AssembledType> {
        self.types.get(name)
    }

    /// Fixed-width hint for an imported type. Returns Some(true) when the
    /// imported type is known fixed-width, Some(false) when known
    /// variable-width, and None when no hint was supplied (callers should
    /// fall back to the conservative variable-width default).
    pub fn import_width(&self, name: &Name) -> Option<bool> {
        self.import_widths.get(name).copied()
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
}

impl Route {
    pub fn new(
        leg: Leg,
        root_slot: usize,
        root: Name,
        endpoint: Endpoint,
        body: RouteBody,
    ) -> Self {
        Self {
            leg,
            root_slot,
            root,
            endpoint,
            body,
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

/// Component-qualified type identifier per intent 469 + audit 171 §7.
/// Displays as `component::namespace::TypeName`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Uid {
    component: Name,
    type_name: Name,
}

impl Uid {
    pub fn new(component: Name, type_name: Name) -> Self {
        Self {
            component,
            type_name,
        }
    }

    pub fn component(&self) -> &Name {
        &self.component
    }

    pub fn type_name(&self) -> &Name {
        &self.type_name
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}::{}::{}",
            self.component, NAMESPACE_LABEL, self.type_name
        )
    }
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
