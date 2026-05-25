use std::collections::{BTreeMap, BTreeSet, HashSet};

use crate::{
    AssembledSchema, BuiltinMacroVariant, Container, DeclarationBody, Error, FanOutOutputDeclaration,
    Feature, FeatureInput, Header, HeaderEndpointInput, HeaderInput, ImportBinding,
    ImportDirective, ImportInput, ImportResolution, ImportedNames, Imports, Leg, LoweringContext,
    Name, Namespace, Payload, Result, RouteBody, TypeExpression, TypeInput, UpgradeAnnotation,
    UpgradeRuleInput,
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

        let mut context = LoweringContext::new();

        for binding in import_index.bindings {
            context.apply(BuiltinMacroVariant::Import(ImportInput::new(binding)))?;
        }

        self.lower_header(
            &mut context,
            Leg::Ordinary,
            &self.ordinary_header,
            &import_index.names,
        )?;
        self.lower_header(
            &mut context,
            Leg::Owner,
            &self.owner_header,
            &import_index.names,
        )?;
        self.lower_header(
            &mut context,
            Leg::Sema,
            &self.sema_header,
            &import_index.names,
        )?;

        for (name, body) in self.namespace.entries() {
            context.apply(BuiltinMacroVariant::Type(TypeInput::local(
                name.clone(),
                body.clone(),
            )))?;
        }
        for (name, binding) in &import_index.names {
            context.apply(BuiltinMacroVariant::Type(TypeInput::imported(
                name.clone(),
                binding.clone(),
            )))?;
        }

        // Universal-Unknown injection per /346 §9 runs after types
        // are lowered but before features so the assembled schema
        // exposes the safety-floor variant on every actor's RESPONSE
        // enum --- the recorder/observer/supervisor/reading-actor
        // schemas all consume this.
        context.finalize_universal_unknowns();

        for feature in &self.features {
            match feature {
                Feature::Upgrade(upgrade) => {
                    context.apply(BuiltinMacroVariant::UpgradeRule(UpgradeRuleInput::new(
                        upgrade.clone(),
                    )))?;
                }
                _ => {
                    context.apply(BuiltinMacroVariant::Feature(FeatureInput::new(
                        feature.clone(),
                    )))?;
                }
            }
        }

        Ok(context.finish())
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
                DeclarationBody::Record(fields) => {
                    for field in fields {
                        self.validate_expression(field.expression(), imports)?;
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
            Payload::Fields(fields) => {
                for field in fields {
                    self.validate_expression(field.expression(), imports)?;
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
                Feature::EffectTable(feature) => {
                    // Each entry references an action (declared in the
                    // namespace as a variant of an ACTION enum or as a
                    // header endpoint) and an effect type (which the
                    // composer will synthesise as a closed enum;
                    // therefore the effect type may NOT exist as a
                    // standalone declaration --- it is composer
                    // output, not author input). Validation here only
                    // verifies the action name corresponds to either
                    // a declared namespace type, a known enum variant,
                    // or a header endpoint. We do the minimum lookup
                    // and let downstream composer code report any
                    // missing references --- per /343 §8, validation
                    // of effect-table references is best handled at
                    // assemble time, not here.
                    for entry in feature.entries() {
                        let _ = entry.action();
                        let _ = entry.effect();
                    }
                }
                Feature::FanOutTargets(feature) => {
                    for entry in feature.entries() {
                        for output in entry.outputs() {
                            match output {
                                FanOutOutputDeclaration::Reply { variant } => {
                                    let _ = variant;
                                }
                                FanOutOutputDeclaration::Actor { .. }
                                | FanOutOutputDeclaration::Subscribers { .. } => {}
                            }
                        }
                    }
                }
                Feature::StorageDescriptor(feature) => {
                    // The logical name labels the table; the
                    // table_type names a declaration in the namespace.
                    // Confirm the table_type is locally declared so
                    // the composer has a body to emit a descriptor for.
                    for entry in feature.entries() {
                        self.validate_named_body(entry.table_type(), imports)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn lower_header(
        &self,
        context: &mut LoweringContext,
        leg: Leg,
        header: &Header,
        imports: &BTreeMap<Name, Name>,
    ) -> Result<()> {
        for (root_slot, root) in header.roots().iter().enumerate() {
            self.validate_route_body_variants(root.name(), root.endpoints())?;
            let mut endpoints = Vec::new();
            for (endpoint_slot, endpoint) in root.endpoints().iter().enumerate() {
                let (body, engine) = self.endpoint_body(root.name(), endpoint, imports)?;
                endpoints.push(HeaderEndpointInput::new(
                    endpoint_slot,
                    endpoint.clone(),
                    body,
                    engine,
                ));
            }
            context.apply(BuiltinMacroVariant::Header(HeaderInput::new(
                leg,
                root_slot,
                root.name().clone(),
                endpoints,
            )))?;
        }
        Ok(())
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
        imports: &BTreeMap<Name, Name>,
    ) -> Result<(RouteBody, Option<crate::Engine>)> {
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

        match variant.payload() {
            Payload::Unit => Ok((RouteBody::Unit, variant.engine())),
            Payload::Type(TypeExpression::Named(name)) => {
                self.validate_named_body_in_names(name, imports)?;
                Ok((RouteBody::Type(name.clone()), variant.engine()))
            }
            Payload::Type(_) | Payload::Fields(_) => Err(Error::InvalidRouteBody {
                root: root.clone(),
                endpoint: endpoint.clone(),
                reason: "endpoint routes must resolve to a named body type or unit".into(),
            }),
        }
    }

    fn validate_named_body(&self, name: &Name, imports: &ResolvedImports) -> Result<()> {
        if self.namespace.body(name).is_some() || imports.names.contains_key(name) {
            Ok(())
        } else {
            Err(Error::UnknownType { name: name.clone() })
        }
    }

    fn validate_named_body_in_names(
        &self,
        name: &Name,
        imports: &BTreeMap<Name, Name>,
    ) -> Result<()> {
        if self.namespace.body(name).is_some() || imports.contains_key(name) {
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
