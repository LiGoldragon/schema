use nota_next::Block;

use crate::{
    Asschema, FieldDeclaration, ImportDeclaration, Name, RootSurface, SchemaError, TypeDeclaration,
    TypeReference,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroPosition {
    RootImports,
    RootSurfaces,
    RootNamespace,
    Surface,
    NamespaceDeclaration,
    StructFields,
    EnumVariants,
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
    fn name(&self) -> &'static str;

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
    macros_applied: Vec<&'static str>,
}

impl MacroContext {
    pub fn remember_position(&mut self, position: MacroPosition) {
        self.positions_seen.push(position);
    }

    pub fn remember_macro(&mut self, macro_name: &'static str) {
        self.macros_applied.push(macro_name);
    }

    pub fn positions_seen(&self) -> &[MacroPosition] {
        &self.positions_seen
    }

    pub fn macros_applied(&self) -> &[&'static str] {
        &self.macros_applied
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroOutput {
    Asschema(Asschema),
    Imports(Vec<ImportDeclaration>),
    Surfaces(Vec<RootSurface>),
    Types(Vec<TypeDeclaration>),
    Surface(RootSurface),
    Type(TypeDeclaration),
    Fields(Vec<FieldDeclaration>),
    Variants(Vec<crate::EnumVariant>),
    References(Vec<TypeReference>),
}

#[derive(Default)]
pub struct MacroRegistry {
    macros: Vec<Box<dyn SchemaMacro>>,
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, schema_macro: impl SchemaMacro + 'static) {
        self.macros.push(Box::new(schema_macro));
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
            macro_name: "registered macro",
        })
    }

    pub fn macro_names(&self) -> Vec<&'static str> {
        self.macros
            .iter()
            .map(|schema_macro| schema_macro.name())
            .collect()
    }
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
