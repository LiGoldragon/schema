use nota_next::{Block, StructureHeader};

use crate::{
    Asschema, EnumDeclaration, FieldDeclaration, ImportDeclaration, Name, SchemaError,
    TypeDeclaration, TypeReference,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroPosition {
    RootImports,
    RootInput,
    RootOutput,
    RootNamespace,
    NamespaceDeclaration,
    StructFields,
    EnumVariants,
    TypeReference,
}

impl MacroPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RootImports => "RootImports",
            Self::RootInput => "RootInput",
            Self::RootOutput => "RootOutput",
            Self::RootNamespace => "RootNamespace",
            Self::NamespaceDeclaration => "NamespaceDeclaration",
            Self::StructFields => "StructFields",
            Self::EnumVariants => "EnumVariants",
            Self::TypeReference => "TypeReference",
        }
    }

    pub(crate) fn from_name(name: &Name) -> Result<Self, SchemaError> {
        match name.as_str() {
            "RootImports" => Ok(Self::RootImports),
            "RootInput" => Ok(Self::RootInput),
            "RootOutput" => Ok(Self::RootOutput),
            "RootNamespace" => Ok(Self::RootNamespace),
            "NamespaceDeclaration" => Ok(Self::NamespaceDeclaration),
            "StructFields" => Ok(Self::StructFields),
            "EnumVariants" => Ok(Self::EnumVariants),
            "TypeReference" => Ok(Self::TypeReference),
            found => Err(SchemaError::UnknownMacroPosition {
                found: found.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MacroObject<'object> {
    Block(&'object Block),
    Pair(MacroPair<'object>),
}

impl<'object> MacroObject<'object> {
    pub fn block(self) -> Option<&'object Block> {
        match self {
            Self::Block(block) => Some(block),
            Self::Pair(_) => None,
        }
    }

    pub fn pair(self) -> Option<MacroPair<'object>> {
        match self {
            Self::Block(_) => None,
            Self::Pair(pair) => Some(pair),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MacroPair<'object> {
    pub name: &'object Block,
    pub definition: &'object Block,
}

pub trait SchemaMacro {
    fn name(&self) -> &str;

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool;

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError>;
}

#[derive(Clone, Debug, Default)]
pub struct MacroContext {
    positions_seen: Vec<MacroPosition>,
    macros_applied: Vec<String>,
    bindings_seen: Vec<String>,
    expanded_templates: Vec<String>,
    structure_headers: Vec<StructureHeader>,
    inline_declarations: Vec<TypeDeclaration>,
}

impl MacroContext {
    pub fn remember_position(&mut self, position: MacroPosition) {
        self.positions_seen.push(position);
    }

    pub fn remember_macro(&mut self, macro_name: impl Into<String>) {
        self.macros_applied.push(macro_name.into());
    }

    pub fn remember_binding(&mut self, macro_name: impl AsRef<str>, binding_name: impl AsRef<str>) {
        self.bindings_seen.push(format!(
            "{}::{}",
            macro_name.as_ref(),
            binding_name.as_ref()
        ));
    }

    pub fn remember_expanded_template(
        &mut self,
        macro_name: impl AsRef<str>,
        template: impl AsRef<str>,
    ) {
        self.expanded_templates
            .push(format!("{} -> {}", macro_name.as_ref(), template.as_ref()));
    }

    pub fn remember_structure_header(&mut self, header: StructureHeader) {
        self.structure_headers.push(header);
    }

    pub(crate) fn remember_inline_declaration(&mut self, declaration: TypeDeclaration) {
        self.inline_declarations.push(declaration);
    }

    pub(crate) fn inline_declaration_count(&self) -> usize {
        self.inline_declarations.len()
    }

    pub(crate) fn drain_inline_declarations_from(&mut self, index: usize) -> Vec<TypeDeclaration> {
        self.inline_declarations.drain(index..).collect()
    }

    pub fn positions_seen(&self) -> &[MacroPosition] {
        &self.positions_seen
    }

    pub fn macros_applied(&self) -> &[String] {
        &self.macros_applied
    }

    pub fn bindings_seen(&self) -> &[String] {
        &self.bindings_seen
    }

    pub fn expanded_templates(&self) -> &[String] {
        &self.expanded_templates
    }

    pub fn structure_headers(&self) -> &[StructureHeader] {
        &self.structure_headers
    }

    pub fn inline_declarations(&self) -> &[TypeDeclaration] {
        &self.inline_declarations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroOutput {
    Asschema(Asschema),
    Imports(Vec<ImportDeclaration>),
    RootEnum(EnumDeclaration),
    Types(Vec<TypeDeclaration>),
    Type(TypeDeclaration),
    Fields(Vec<FieldDeclaration>),
    Variants(Vec<crate::EnumVariant>),
    Reference(TypeReference),
    References(Vec<TypeReference>),
}

pub struct MacroRegistry {
    macros: Vec<Box<dyn SchemaMacro>>,
    node_definitions: Vec<MacroNodeDefinition>,
}

impl Default for MacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self {
            macros: Vec::new(),
            node_definitions: Vec::new(),
        }
    }

    pub fn register(&mut self, schema_macro: impl SchemaMacro + 'static) {
        self.macros.push(Box::new(schema_macro));
    }

    pub fn register_box(&mut self, schema_macro: Box<dyn SchemaMacro>) {
        self.macros.push(schema_macro);
    }

    pub fn register_node_definition(&mut self, definition: MacroNodeDefinition) {
        self.node_definitions.push(definition);
    }

    pub fn node_definition(&self, position: MacroPosition) -> Option<&MacroNodeDefinition> {
        self.node_definitions
            .iter()
            .find(|definition| definition.position == position)
    }

    pub fn node_definitions(&self) -> &[MacroNodeDefinition] {
        &self.node_definitions
    }

    pub fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        for schema_macro in &self.macros {
            if schema_macro.matches(object, position) {
                return schema_macro.lower(object, position, context, self);
            }
        }
        Err(SchemaError::MacroDidNotMatch {
            macro_name: "registered macro".to_owned(),
        })
    }

    pub fn macro_names(&self) -> Vec<String> {
        self.macros
            .iter()
            .map(|schema_macro| schema_macro.name().to_owned())
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacroNodeDefinition {
    position: MacroPosition,
    dispatch: MacroDispatch,
}

impl MacroNodeDefinition {
    pub fn new(position: MacroPosition, dispatch: MacroDispatch) -> Self {
        Self { position, dispatch }
    }

    pub fn position(&self) -> MacroPosition {
        self.position
    }

    pub fn dispatch(&self) -> MacroDispatch {
        self.dispatch
    }

    pub fn accepts_tagged_invocation(&self) -> bool {
        matches!(
            self.dispatch,
            MacroDispatch::TaggedInvocation | MacroDispatch::StructuralOrTaggedInvocation
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroDispatch {
    RootPositional,
    Structural,
    TaggedInvocation,
    StructuralOrTaggedInvocation,
}

pub(crate) trait BlockDebug {
    fn reemit_fallback(&self) -> String;
}

pub(crate) trait SchemaBlockExt {
    fn schema_name(&self) -> Result<Name, SchemaError>;
}

impl BlockDebug for Block {
    fn reemit_fallback(&self) -> String {
        self.demote_to_string()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{self:?}"))
    }
}

impl SchemaBlockExt for Block {
    fn schema_name(&self) -> Result<Name, SchemaError> {
        self.atom()
            .filter(|atom| atom.qualifies_as_symbol())
            .map(|atom| Name::new(atom.text()))
            .ok_or_else(|| SchemaError::ExpectedSymbol {
                found: self.reemit_fallback(),
            })
    }
}
