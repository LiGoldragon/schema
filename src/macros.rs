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

pub trait SchemaMacro {
    fn name(&self) -> &'static str;

    fn matches(&self, object: &Block, position: MacroPosition) -> bool;

    fn lower(
        &self,
        object: &Block,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError>;
}

#[derive(Clone, Debug, Default)]
pub struct MacroContext {
    positions_seen: Vec<MacroPosition>,
}

impl MacroContext {
    pub fn remember_position(&mut self, position: MacroPosition) {
        self.positions_seen.push(position);
    }

    pub fn positions_seen(&self) -> &[MacroPosition] {
        &self.positions_seen
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroOutput {
    Asschema(Asschema),
    Imports(Vec<ImportDeclaration>),
    Surface(RootSurface),
    Type(TypeDeclaration),
    Fields(Vec<FieldDeclaration>),
    References(Vec<TypeReference>),
}

pub(crate) fn atom_name(object: &Block) -> Result<Name, SchemaError> {
    object
        .atom()
        .filter(|atom| atom.qualifies_as_symbol())
        .map(|atom| Name::new(atom.text()))
        .ok_or_else(|| SchemaError::ExpectedSymbol {
            found: object.reemit_fallback(),
        })
}

pub(crate) trait BlockDebug {
    fn reemit_fallback(&self) -> String;
}

impl BlockDebug for Block {
    fn reemit_fallback(&self) -> String {
        self.demote_to_string()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{self:?}"))
    }
}
