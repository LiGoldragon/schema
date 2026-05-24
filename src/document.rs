use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    AssembledSchema, AssembledType, Container, DeclarationBody, Endpoint, Engine, Error, Feature,
    Header, ImportBinding, ImportDirective, ImportResolution, ImportedNames, Imports, Leg, Name,
    Namespace, Payload, Result, Route, RouteBody, TypeExpression, UpgradeAnnotation,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Schema {
    imports: Imports,
    ordinary_header: Header,
    owner_header: Header,
    sema_header: Header,
    namespace: Namespace,
    features: Vec<Feature>,
}

pub type Document = Schema;

impl Schema {
    pub fn new(
        imports: Imports,
        ordinary_header: Header,
        owner_header: Header,
        sema_header: Header,
        namespace: Namespace,
        features: Vec<Feature>,
    ) -> Result<Self> {
        let schema = Self {
            imports,
            ordinary_header,
            owner_header,
            sema_header,
            namespace,
            features,
        };
        schema.validate_authored()?;
        Ok(schema)
    }

    pub fn imports(&self) -> &Imports {
        &self.imports
    }

    pub fn ordinary_header(&self) -> &Header {
        &self.ordinary_header
    }

    pub fn owner_header(&self) -> &Header {
        &self.owner_header
    }

    pub fn sema_header(&self) -> &Header {
        &self.sema_header
    }

    pub fn namespace(&self) -> &Namespace {
        &self.namespace
    }

    pub fn features(&self) -> &[Feature] {
        &self.features
    }

    pub fn declaration_body(&self, name: &Name) -> Option<&DeclarationBody> {
        self.namespace.body(name)
    }

    pub fn variant(&self, declaration: &Name, variant: &Name) -> Result<&crate::Variant> {
        let body = self
            .declaration_body(declaration)
            .ok_or_else(|| Error::MissingDeclaration {
                name: declaration.clone(),
            })?;
        let DeclarationBody::Enum { variants } = body else {
            return Err(Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant.clone(),
            });
        };
        variants
            .iter()
            .find(|candidate| candidate.name() == variant)
            .ok_or_else(|| Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant.clone(),
            })
    }

    pub fn assemble(&self, resolutions: &[ImportResolution]) -> Result<AssembledSchema> {
        let import_index = self.resolve_imports(resolutions)?;
        self.validate_with_imports(&import_index)?;

        let mut routes = Vec::new();
        routes.extend(self.lower_header(Leg::Ordinary, &self.ordinary_header, &import_index)?);
        routes.extend(self.lower_header(Leg::Owner, &self.owner_header, &import_index)?);
        routes.extend(self.lower_header(Leg::Sema, &self.sema_header, &import_index)?);

        let mut types = Vec::new();
        for (name, body) in self.namespace.entries() {
            types.push(AssembledType::local(name.clone(), body.clone()));
        }
        for (name, binding) in &import_index.names {
            types.push(AssembledType::imported(name.clone(), binding.clone()));
        }

        Ok(AssembledSchema::new(
            import_index.bindings,
            routes,
            types,
            self.features.clone(),
        ))
    }

    fn validate_authored(&self) -> Result<()> {
        self.validate_variants()?;
        let import_index = self.resolve_selected_imports()?;
        self.validate_with_imports(&import_index)
    }

    fn validate_variants(&self) -> Result<()> {
        for (name, body) in self.namespace.entries() {
            let DeclarationBody::Enum { variants } = body else {
                continue;
            };
            let mut variant_names = HashSet::new();
            for variant in variants {
                if !variant_names.insert(variant.name()) {
                    return Err(Error::DuplicateVariant {
                        declaration: name.clone(),
                        variant: variant.name().clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_with_imports(&self, imports: &ResolvedImports) -> Result<()> {
        for (name, body) in self.namespace.entries() {
            match body {
                DeclarationBody::Enum { variants } => {
                    for variant in variants {
                        self.validate_payload(variant.payload(), imports)?;
                    }
                }
                DeclarationBody::Newtype(expression) | DeclarationBody::Alias(expression) => {
                    self.validate_expression(expression, imports)?
                }
                DeclarationBody::Record(expressions) => {
                    for expression in expressions {
                        self.validate_expression(expression, imports)?;
                    }
                }
            }

            if let Some(binding) = imports.names.get(name) {
                return Err(Error::ImportCollisionWithLocal {
                    name: name.clone(),
                    binding: binding.clone(),
                });
            }
        }
        self.validate_features(imports)
    }

    fn validate_payload(&self, payload: &Payload, imports: &ResolvedImports) -> Result<()> {
        match payload {
            Payload::Unit => Ok(()),
            Payload::Type(expression) => self.validate_expression(expression, imports),
            Payload::Fields(expressions) => {
                for expression in expressions {
                    self.validate_expression(expression, imports)?;
                }
                Ok(())
            }
        }
    }

    fn validate_expression(
        &self,
        expression: &TypeExpression,
        imports: &ResolvedImports,
    ) -> Result<()> {
        match expression {
            TypeExpression::Primitive(_) => Ok(()),
            TypeExpression::Named(name) => {
                if self.namespace.body(name).is_some()
                    || imports.names.contains_key(name)
                    || imports.has_unresolved_import_all
                {
                    Ok(())
                } else {
                    Err(Error::UnknownType { name: name.clone() })
                }
            }
            TypeExpression::Container(container) => match container {
                Container::Vector(inner) | Container::Optional(inner) => {
                    self.validate_expression(inner, imports)
                }
                Container::Map { key, value } => {
                    self.validate_expression(key, imports)?;
                    self.validate_expression(value, imports)
                }
            },
        }
    }

    fn validate_features(&self, imports: &ResolvedImports) -> Result<()> {
        for feature in &self.features {
            match feature {
                Feature::Reply(names) => {
                    for name in names {
                        self.validate_named_body(name, imports)?;
                    }
                }
                Feature::Event(event) => {
                    for name in event.events() {
                        self.validate_named_body(name, imports)?;
                    }
                }
                Feature::Observable(observable) => {
                    if let Some(name) = observable.operation_event() {
                        self.validate_named_body(name, imports)?;
                    }
                    if let Some(name) = observable.effect_event() {
                        self.validate_named_body(name, imports)?;
                    }
                }
                Feature::Upgrade(upgrade) => {
                    for annotation in upgrade.annotations() {
                        match annotation {
                            UpgradeAnnotation::Migrate(name)
                            | UpgradeAnnotation::RenamedFrom { current: name, .. }
                            | UpgradeAnnotation::Custom { name, .. } => {
                                self.validate_named_body(name, imports)?;
                            }
                            UpgradeAnnotation::Drop(_) | UpgradeAnnotation::Untranslatable(_) => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn lower_header(
        &self,
        leg: Leg,
        header: &Header,
        imports: &ResolvedImports,
    ) -> Result<Vec<Route>> {
        let mut routes = Vec::new();
        for (root_slot, root) in header.roots().iter().enumerate() {
            self.validate_route_body_variants(root.name(), root.endpoints())?;
            for (endpoint_slot, endpoint) in root.endpoints().iter().enumerate() {
                // DESIGN-DECISION-REVIEW (second-designer/172 §3.2): resolve
                // body AND engine in one pass so the Route carries both.
                let (body, engine) = self.endpoint_body(root.name(), endpoint, imports)?;
                routes.push(Route::new(
                    leg,
                    root_slot,
                    root.name().clone(),
                    Endpoint::new(endpoint_slot, endpoint.clone()),
                    body,
                    engine,
                ));
            }
        }
        Ok(routes)
    }

    fn validate_route_body_variants(&self, root: &Name, endpoints: &[Name]) -> Result<()> {
        let Some(DeclarationBody::Enum { variants }) = self.declaration_body(root) else {
            return Ok(());
        };
        let endpoint_names = endpoints.iter().collect::<BTreeSet<_>>();
        for variant in variants {
            if !endpoint_names.contains(variant.name()) {
                return Err(Error::UnmatchedRouteBodyVariant {
                    root: root.clone(),
                    variant: variant.name().clone(),
                });
            }
        }
        Ok(())
    }

    fn endpoint_body(
        &self,
        root: &Name,
        endpoint: &Name,
        imports: &ResolvedImports,
    ) -> Result<(RouteBody, Option<Engine>)> {
        let declaration = self
            .declaration_body(root)
            .ok_or_else(|| Error::MissingRouteBody {
                root: root.clone(),
                endpoint: endpoint.clone(),
            })?;
        let DeclarationBody::Enum { variants } = declaration else {
            return Err(Error::MissingRouteBody {
                root: root.clone(),
                endpoint: endpoint.clone(),
            });
        };
        let variant = variants
            .iter()
            .find(|variant| variant.name() == endpoint)
            .ok_or_else(|| Error::MissingRouteBody {
                root: root.clone(),
                endpoint: endpoint.clone(),
            })?;

        let body = match variant.payload() {
            Payload::Unit => RouteBody::Unit,
            Payload::Type(TypeExpression::Named(name)) => {
                self.validate_named_body(name, imports)?;
                RouteBody::Type(name.clone())
            }
            Payload::Type(_) | Payload::Fields(_) => {
                return Err(Error::InvalidRouteBody {
                    root: root.clone(),
                    endpoint: endpoint.clone(),
                    reason: "endpoint routes must resolve to a named body type or unit".into(),
                });
            }
        };

        Ok((body, variant.engine()))
    }

    fn validate_named_body(&self, name: &Name, imports: &ResolvedImports) -> Result<()> {
        if self.namespace.body(name).is_some() || imports.names.contains_key(name) {
            Ok(())
        } else {
            Err(Error::UnknownType { name: name.clone() })
        }
    }

    fn resolve_selected_imports(&self) -> Result<ResolvedImports> {
        let mut names = BTreeMap::new();
        let mut bindings = Vec::new();
        let mut has_unresolved_import_all = false;

        for (binding, directive) in self.imports.entries() {
            match directive {
                ImportDirective::Import {
                    path,
                    names: selected,
                } => {
                    record_import_names(&mut names, binding, selected)?;
                    bindings.push(ImportBinding::new(
                        binding.clone(),
                        path.clone(),
                        ImportedNames::Selected(selected.clone()),
                    ));
                }
                ImportDirective::ImportAll { path } => {
                    has_unresolved_import_all = true;
                    bindings.push(ImportBinding::new(
                        binding.clone(),
                        path.clone(),
                        ImportedNames::All(Vec::new()),
                    ));
                }
            }
        }

        Ok(ResolvedImports {
            bindings,
            names,
            has_unresolved_import_all,
        })
    }

    fn resolve_imports(&self, resolutions: &[ImportResolution]) -> Result<ResolvedImports> {
        let mut resolution_map = BTreeMap::new();
        for resolution in resolutions {
            if self.imports.directive(resolution.binding()).is_none() {
                return Err(Error::UnknownImportResolution {
                    binding: resolution.binding().clone(),
                });
            }
            resolution_map.insert(resolution.binding().clone(), resolution.names().to_vec());
        }

        let mut names = BTreeMap::new();
        let mut bindings = Vec::new();

        for (binding, directive) in self.imports.entries() {
            match directive {
                ImportDirective::Import {
                    path,
                    names: selected,
                } => {
                    record_import_names(&mut names, binding, selected)?;
                    bindings.push(ImportBinding::new(
                        binding.clone(),
                        path.clone(),
                        ImportedNames::Selected(selected.clone()),
                    ));
                }
                ImportDirective::ImportAll { path } => {
                    let resolved = resolution_map.get(binding).ok_or_else(|| {
                        Error::MissingImportResolution {
                            binding: binding.clone(),
                        }
                    })?;
                    record_import_names(&mut names, binding, resolved)?;
                    bindings.push(ImportBinding::new(
                        binding.clone(),
                        path.clone(),
                        ImportedNames::All(resolved.clone()),
                    ));
                }
            }
        }

        Ok(ResolvedImports {
            bindings,
            names,
            has_unresolved_import_all: false,
        })
    }
}

struct ResolvedImports {
    bindings: Vec<ImportBinding>,
    names: BTreeMap<Name, Name>,
    has_unresolved_import_all: bool,
}

fn record_import_names(
    imports: &mut BTreeMap<Name, Name>,
    binding: &Name,
    names: &[Name],
) -> Result<()> {
    let mut local_names = BTreeSet::new();
    for name in names {
        if !local_names.insert(name) {
            return Err(Error::DuplicateResolvedImportName {
                binding: binding.clone(),
                name: name.clone(),
            });
        }
        if let Some(first_binding) = imports.insert(name.clone(), binding.clone()) {
            return Err(Error::DuplicateImportedName {
                name: name.clone(),
                first_binding,
                second_binding: binding.clone(),
            });
        }
    }
    Ok(())
}
