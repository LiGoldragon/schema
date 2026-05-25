use crate::{
    AssembledSchema, AssembledType, DeclarationBody, Endpoint, Engine, Feature, ImportBinding, Leg,
    Name, Payload, Primitive, Result, Route, RouteBody, TypeExpression, Upgrade, Variant,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeDefinitionPoint {
    ImportMapValue,
    HeaderRoot,
    NamespaceValue,
    FeatureItem,
    UpgradeRule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinMacroVariant {
    Import(ImportInput),
    Header(HeaderInput),
    Type(TypeInput),
    Feature(FeatureInput),
    UpgradeRule(UpgradeRuleInput),
}

impl BuiltinMacroVariant {
    pub fn point(&self) -> NodeDefinitionPoint {
        match self {
            Self::Import(_) => NodeDefinitionPoint::ImportMapValue,
            Self::Header(_) => NodeDefinitionPoint::HeaderRoot,
            Self::Type(_) => NodeDefinitionPoint::NamespaceValue,
            Self::Feature(_) => NodeDefinitionPoint::FeatureItem,
            Self::UpgradeRule(_) => NodeDefinitionPoint::UpgradeRule,
        }
    }

    pub fn lower(self, context: &mut LoweringContext) -> Result<()> {
        match self {
            Self::Import(input) => ImportMacro.lower(input, context),
            Self::Header(input) => HeaderMacro.lower(input, context),
            Self::Type(input) => TypeMacro.lower(input, context),
            Self::Feature(input) => FeatureMacro.lower(input, context),
            Self::UpgradeRule(input) => UpgradeRuleMacro.lower(input, context),
        }
    }
}

pub trait SchemaMacro<Input> {
    fn lower(&self, input: Input, context: &mut LoweringContext) -> Result<()>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportInput {
    binding: ImportBinding,
}

impl ImportInput {
    pub fn new(binding: ImportBinding) -> Self {
        Self { binding }
    }

    pub fn binding(&self) -> &ImportBinding {
        &self.binding
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderInput {
    leg: Leg,
    root_slot: usize,
    root: Name,
    endpoints: Vec<HeaderEndpointInput>,
}

impl HeaderInput {
    pub fn new(
        leg: Leg,
        root_slot: usize,
        root: Name,
        endpoints: Vec<HeaderEndpointInput>,
    ) -> Self {
        Self {
            leg,
            root_slot,
            root,
            endpoints,
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

    pub fn endpoints(&self) -> &[HeaderEndpointInput] {
        &self.endpoints
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderEndpointInput {
    slot: usize,
    name: Name,
    body: RouteBody,
    engine: Option<Engine>,
}

impl HeaderEndpointInput {
    pub fn new(slot: usize, name: Name, body: RouteBody, engine: Option<Engine>) -> Self {
        Self {
            slot,
            name,
            body,
            engine,
        }
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn body(&self) -> &RouteBody {
        &self.body
    }

    pub fn engine(&self) -> Option<Engine> {
        self.engine
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeInput {
    Local { name: Name, body: DeclarationBody },
    Imported { name: Name, binding: Name },
}

impl TypeInput {
    pub fn local(name: Name, body: DeclarationBody) -> Self {
        Self::Local { name, body }
    }

    pub fn imported(name: Name, binding: Name) -> Self {
        Self::Imported { name, binding }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeatureInput {
    feature: Feature,
}

impl FeatureInput {
    pub fn new(feature: Feature) -> Self {
        Self { feature }
    }

    pub fn feature(&self) -> &Feature {
        &self.feature
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRuleInput {
    upgrade: Upgrade,
}

impl UpgradeRuleInput {
    pub fn new(upgrade: Upgrade) -> Self {
        Self { upgrade }
    }

    pub fn upgrade(&self) -> &Upgrade {
        &self.upgrade
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssembledFragment {
    Import(ImportBinding),
    Route(Route),
    Type(AssembledType),
    Feature(Feature),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoweringContext {
    imports: Vec<ImportBinding>,
    routes: Vec<Route>,
    types: Vec<AssembledType>,
    features: Vec<Feature>,
}

impl LoweringContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, variant: BuiltinMacroVariant) -> Result<()> {
        variant.lower(self)
    }

    pub fn push(&mut self, fragment: AssembledFragment) {
        match fragment {
            AssembledFragment::Import(binding) => self.imports.push(binding),
            AssembledFragment::Route(route) => self.routes.push(route),
            AssembledFragment::Type(schema_type) => self.types.push(schema_type),
            AssembledFragment::Feature(feature) => self.features.push(feature),
        }
    }

    /// Inject `Unknown(String)` into every local RESPONSE-shaped enum
    /// per /346 §9. Runs after all `TypeMacro` invocations but before
    /// features so the assembled schema exposes the universal safety
    /// floor on every actor's response channel.
    ///
    /// Idempotent: enums that already carry an `Unknown` variant are
    /// left untouched.
    pub fn finalize_universal_unknowns(&mut self) {
        for schema_type in &mut self.types {
            let AssembledType::Local { name, body } = schema_type else {
                continue;
            };
            if !UniversalUnknownMacro::is_response_enum_name(name) {
                continue;
            }
            UniversalUnknownMacro::inject_unknown_into_enum_body(body);
        }
    }

    pub fn finish(self) -> AssembledSchema {
        AssembledSchema::new(self.imports, self.routes, self.types, self.features)
    }
}

pub struct ImportMacro;

impl SchemaMacro<ImportInput> for ImportMacro {
    fn lower(&self, input: ImportInput, context: &mut LoweringContext) -> Result<()> {
        context.push(AssembledFragment::Import(input.binding));
        Ok(())
    }
}

pub struct HeaderMacro;

impl SchemaMacro<HeaderInput> for HeaderMacro {
    fn lower(&self, input: HeaderInput, context: &mut LoweringContext) -> Result<()> {
        for endpoint in input.endpoints {
            context.push(AssembledFragment::Route(Route::with_engine(
                input.leg,
                input.root_slot,
                input.root.clone(),
                Endpoint::new(endpoint.slot, endpoint.name),
                endpoint.body,
                endpoint.engine,
            )));
        }
        Ok(())
    }
}

pub struct TypeMacro;

impl SchemaMacro<TypeInput> for TypeMacro {
    fn lower(&self, input: TypeInput, context: &mut LoweringContext) -> Result<()> {
        let schema_type = match input {
            TypeInput::Local { name, body } => AssembledType::local(name, body),
            TypeInput::Imported { name, binding } => AssembledType::imported(name, binding),
        };
        context.push(AssembledFragment::Type(schema_type));
        Ok(())
    }
}

pub struct FeatureMacro;

impl SchemaMacro<FeatureInput> for FeatureMacro {
    fn lower(&self, input: FeatureInput, context: &mut LoweringContext) -> Result<()> {
        context.push(AssembledFragment::Feature(input.feature));
        Ok(())
    }
}

pub struct UpgradeRuleMacro;

impl SchemaMacro<UpgradeRuleInput> for UpgradeRuleMacro {
    fn lower(&self, input: UpgradeRuleInput, context: &mut LoweringContext) -> Result<()> {
        context.push(AssembledFragment::Feature(Feature::Upgrade(input.upgrade)));
        Ok(())
    }
}

/// Universal-Unknown injector for actor RESPONSE-shaped enums per
/// /346 §9.
///
/// Every actor schema's RESPONSE enum carries a `Unknown` variant
/// carrying a `String` reason (per /346 §1 the safety floor). Rather
/// than authoring this manually in every schema, the schema engine
/// injects it through this builtin macro.
///
/// The macro runs as a post-lowering sweep over `LoweringContext`:
/// any local enum type whose name ends in `Response` (the convention
/// per /346 §1) gets an `Unknown(String)` variant appended IF it
/// doesn't already carry one. Parallel to how `signal_channel!`'s
/// existing observable macro injects `Tap`/`Untap` operations per
/// /346 §9.
///
/// BLOCKED: wiring into `BuiltinMacroVariant::lower` requires a
/// post-pass `LoweringContext::finalize_universal_unknowns(&mut self)`
/// hook that runs AFTER all `TypeMacro` invocations but BEFORE
/// `LoweringContext::finish()`. The hook walks `self.types` and
/// mutates each Response enum's variant list. See /346 §11 step 1.
///
/// The implementation below is a stub that documents the intended
/// shape; full wiring is operator slice per /346 §11 step 1.
pub struct UniversalUnknownMacro;

impl UniversalUnknownMacro {
    /// Identifier convention per /346 §1: actor RESPONSE enums end in
    /// `Response`. Schema authors who use a different convention can
    /// opt out by NOT ending the type name in `Response` --- this is
    /// shape-logic detection per /338 §5.1 core macros.
    pub fn is_response_enum_name(name: &Name) -> bool {
        name.as_str().ends_with("Response")
    }

    /// Whether the body is enum-shaped (Unknown injection only
    /// applies to closed enums per /346 §1).
    pub fn body_is_enum(body: &crate::DeclarationBody) -> bool {
        matches!(body, crate::DeclarationBody::Enum { .. })
    }

    /// The injected variant's name. Universal per /346 §9.
    pub const UNKNOWN_VARIANT_NAME: &'static str = "Unknown";

    /// Apply the universal-Unknown injection to a Response enum body.
    /// Idempotent --- if the body already carries an `Unknown` variant,
    /// this is a no-op. Only enum-shaped bodies are touched (records,
    /// newtypes, and aliases are skipped silently).
    ///
    /// The injected variant carries a `String` payload --- the
    /// universal "I don't know what you're asking for" channel per
    /// /346 §1.
    pub fn inject_unknown_into_enum_body(body: &mut DeclarationBody) {
        let DeclarationBody::Enum { variants } = body else {
            return;
        };
        let already_has_unknown = variants
            .iter()
            .any(|variant| variant.name().as_str() == Self::UNKNOWN_VARIANT_NAME);
        if already_has_unknown {
            return;
        }
        let name = Name::new(Self::UNKNOWN_VARIANT_NAME)
            .expect("`Unknown` is a valid PascalCase identifier");
        variants.push(Variant::with_type(
            name,
            TypeExpression::Primitive(Primitive::String),
        ));
    }
}

#[allow(dead_code)]
fn _payload_marker(_: &Payload, _: &RouteBody) {}
