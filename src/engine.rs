use crate::{
    AssembledSchema, AssembledType, DeclarationBody, Endpoint, Engine, Feature, ImportBinding, Leg,
    Name, Result, Route, RouteBody,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeDefinitionPoint {
    ImportMapValue,
    HeaderRoot,
    NamespaceValue,
    FeatureItem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuiltinMacroVariant {
    Import(ImportInput),
    Header(HeaderInput),
    Type(TypeInput),
    Feature(FeatureInput),
}

impl BuiltinMacroVariant {
    pub fn point(&self) -> NodeDefinitionPoint {
        match self {
            Self::Import(_) => NodeDefinitionPoint::ImportMapValue,
            Self::Header(_) => NodeDefinitionPoint::HeaderRoot,
            Self::Type(_) => NodeDefinitionPoint::NamespaceValue,
            Self::Feature(_) => NodeDefinitionPoint::FeatureItem,
        }
    }

    pub fn lower(self, context: &mut LoweringContext) -> Result<()> {
        match self {
            Self::Import(input) => ImportMacro.lower(input, context),
            Self::Header(input) => HeaderMacro.lower(input, context),
            Self::Type(input) => TypeMacro.lower(input, context),
            Self::Feature(input) => FeatureMacro.lower(input, context),
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
