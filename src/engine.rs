use nota_next::{Block, Document};

use crate::{
    asschema::{
        Asschema, EnumDeclaration, EnumVariant, FieldDeclaration, ImportDeclaration, Name,
        RootSurface, StructDeclaration, TypeDeclaration, TypeReference,
    },
    macros::{MacroContext, MacroOutput, MacroPosition, SchemaMacro, atom_name},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaIdentity {
    component: Name,
    version: String,
}

impl SchemaIdentity {
    pub fn new(component: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            component: Name::new(component),
            version: version.into(),
        }
    }

    pub fn component(&self) -> &Name {
        &self.component
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchemaError {
    Nota(String),
    ExpectedRootObjectCount { expected: usize, found: usize },
    ExpectedDelimiter { expected: &'static str },
    ExpectedEvenMapEntries { found: usize },
    ExpectedSymbol { found: String },
    ExpectedSurfaceVariant,
    MacroDidNotMatch { macro_name: &'static str },
}

impl From<nota_next::NotaError> for SchemaError {
    fn from(value: nota_next::NotaError) -> Self {
        Self::Nota(value.to_string())
    }
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for SchemaError {}

#[derive(Clone, Debug, Default)]
pub struct SchemaEngine {
    type_declaration_macro: TypeDeclarationMacro,
    surface_macro: SurfaceMacro,
}

impl SchemaEngine {
    pub fn lower_source(
        &self,
        source: &str,
        identity: SchemaIdentity,
    ) -> Result<Asschema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document(&document, identity)
    }

    pub fn lower_document(
        &self,
        document: &Document,
        identity: SchemaIdentity,
    ) -> Result<Asschema, SchemaError> {
        if document.holds_root_objects() != 3 {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: 3,
                found: document.holds_root_objects(),
            });
        }

        let imports = self.lower_imports(
            document.root_object_at(0).expect("checked root count"),
            &mut MacroContext::default(),
        )?;
        let surfaces = self.lower_surfaces(
            document.root_object_at(1).expect("checked root count"),
            &mut MacroContext::default(),
        )?;
        let namespace = self.lower_namespace(
            document.root_object_at(2).expect("checked root count"),
            &mut MacroContext::default(),
        )?;

        Ok(Asschema::new(identity, imports, surfaces, namespace))
    }

    fn lower_imports(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<ImportDeclaration>, SchemaError> {
        context.remember_position(MacroPosition::RootImports);
        if !object.is_brace() {
            return Err(SchemaError::ExpectedDelimiter { expected: "{ }" });
        }
        if object.holds_root_objects() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: object.holds_root_objects(),
            });
        }

        let mut imports = Vec::new();
        for index in (0..object.holds_root_objects()).step_by(2) {
            let local_name = atom_name(
                object
                    .root_object_at(index)
                    .expect("index within map object count"),
            )?;
            let source = atom_name(
                object
                    .root_object_at(index + 1)
                    .expect("index within map object count"),
            )?;
            imports.push(ImportDeclaration {
                local_name,
                source: TypeReference { name: source },
            });
        }
        Ok(imports)
    }

    fn lower_surfaces(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<RootSurface>, SchemaError> {
        context.remember_position(MacroPosition::RootSurfaces);
        if !object.is_square_bracket() {
            return Err(SchemaError::ExpectedDelimiter { expected: "[ ]" });
        }

        let mut surfaces = Vec::new();
        for index in 0..object.holds_root_objects() {
            let child = object
                .root_object_at(index)
                .expect("index within surface count");
            if !self.surface_macro.matches(child, MacroPosition::Surface) {
                return Err(SchemaError::MacroDidNotMatch {
                    macro_name: self.surface_macro.name(),
                });
            }
            match self
                .surface_macro
                .lower(child, MacroPosition::Surface, context)?
            {
                MacroOutput::Surface(surface) => surfaces.push(surface),
                _ => unreachable!("surface macro returns surface"),
            }
        }
        Ok(surfaces)
    }

    fn lower_namespace(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<TypeDeclaration>, SchemaError> {
        context.remember_position(MacroPosition::RootNamespace);
        if !object.is_brace() {
            return Err(SchemaError::ExpectedDelimiter { expected: "{ }" });
        }
        if object.holds_root_objects() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: object.holds_root_objects(),
            });
        }

        let mut declarations = Vec::new();
        for index in (0..object.holds_root_objects()).step_by(2) {
            let name = object
                .root_object_at(index)
                .expect("index within namespace object count");
            let definition = object
                .root_object_at(index + 1)
                .expect("index within namespace object count");
            let entry = SyntheticPair { name, definition };
            if !self
                .type_declaration_macro
                .matches_pair(&entry, MacroPosition::NamespaceDeclaration)
            {
                return Err(SchemaError::MacroDidNotMatch {
                    macro_name: self.type_declaration_macro.name(),
                });
            }
            declarations.push(self.type_declaration_macro.lower_pair(
                &entry,
                MacroPosition::NamespaceDeclaration,
                context,
            )?);
        }
        Ok(declarations)
    }
}

#[derive(Clone, Debug)]
struct SyntheticPair<'block> {
    name: &'block Block,
    definition: &'block Block,
}

#[derive(Clone, Debug, Default)]
struct SurfaceMacro;

impl SchemaMacro for SurfaceMacro {
    fn name(&self) -> &'static str {
        "Surface"
    }

    fn matches(&self, object: &Block, position: MacroPosition) -> bool {
        position == MacroPosition::Surface
            && object.is_parenthesis()
            && object.holds_root_objects() >= 1
            && object
                .root_object_at(0)
                .is_some_and(Block::qualifies_as_pascal_case_symbol)
    }

    fn lower(
        &self,
        object: &Block,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_position(position);
        let name = atom_name(
            object
                .root_object_at(0)
                .expect("surface match checked first object"),
        )?;
        let mut variants = Vec::new();
        for index in 1..object.holds_root_objects() {
            variants.push(lower_surface_variant(
                object
                    .root_object_at(index)
                    .expect("index within surface object count"),
            )?);
        }
        Ok(MacroOutput::Surface(RootSurface { name, variants }))
    }
}

fn lower_surface_variant(object: &Block) -> Result<EnumVariant, SchemaError> {
    if object.is_parenthesis() {
        match object.holds_root_objects() {
            1 => Ok(EnumVariant {
                name: atom_name(object.root_object_at(0).expect("count checked"))?,
                payload: None,
            }),
            2 => Ok(EnumVariant {
                name: atom_name(object.root_object_at(0).expect("count checked"))?,
                payload: Some(TypeReference {
                    name: atom_name(object.root_object_at(1).expect("count checked"))?,
                }),
            }),
            _ => Err(SchemaError::ExpectedSurfaceVariant),
        }
    } else if object.qualifies_as_pascal_case_symbol() {
        Ok(EnumVariant {
            name: atom_name(object)?,
            payload: None,
        })
    } else {
        Err(SchemaError::ExpectedSurfaceVariant)
    }
}

#[derive(Clone, Debug, Default)]
struct TypeDeclarationMacro;

impl TypeDeclarationMacro {
    fn matches_pair(&self, pair: &SyntheticPair<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::NamespaceDeclaration
            && pair.name.qualifies_as_pascal_case_symbol()
            && (pair.definition.is_square_bracket() || pair.definition.is_parenthesis())
    }

    fn lower_pair(
        &self,
        pair: &SyntheticPair<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        context.remember_position(position);
        let name = atom_name(pair.name)?;
        if pair.definition.is_square_bracket() {
            let fields = lower_fields(pair.definition)?;
            let declaration = StructDeclaration { name, fields };
            if declaration.fields.len() == 1 {
                Ok(TypeDeclaration::Newtype(declaration))
            } else {
                Ok(TypeDeclaration::Struct(declaration))
            }
        } else if pair.definition.is_parenthesis() {
            let variants = lower_enum_variants(pair.definition)?;
            Ok(TypeDeclaration::Enum(EnumDeclaration { name, variants }))
        } else {
            Err(SchemaError::ExpectedDelimiter {
                expected: "[ ] or ( )",
            })
        }
    }
}

impl SchemaMacro for TypeDeclarationMacro {
    fn name(&self) -> &'static str {
        "TypeDeclaration"
    }

    fn matches(&self, object: &Block, position: MacroPosition) -> bool {
        position == MacroPosition::NamespaceDeclaration && object.is_brace()
    }

    fn lower(
        &self,
        object: &Block,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_position(position);
        if !object.is_brace() {
            return Err(SchemaError::ExpectedDelimiter { expected: "{ }" });
        }
        Ok(MacroOutput::References(Vec::new()))
    }
}

fn lower_fields(object: &Block) -> Result<Vec<FieldDeclaration>, SchemaError> {
    let mut fields = Vec::new();
    for index in 0..object.holds_root_objects() {
        let name = atom_name(object.root_object_at(index).expect("field index in bounds"))?;
        fields.push(FieldDeclaration {
            name: Name::new(name.field_name()),
            reference: TypeReference { name },
        });
    }
    Ok(fields)
}

fn lower_enum_variants(object: &Block) -> Result<Vec<EnumVariant>, SchemaError> {
    let mut variants = Vec::new();
    for index in 0..object.holds_root_objects() {
        let child = object
            .root_object_at(index)
            .expect("variant index in bounds");
        variants.push(if child.is_parenthesis() {
            lower_surface_variant(child)?
        } else {
            EnumVariant {
                name: atom_name(child)?,
                payload: None,
            }
        });
    }
    Ok(variants)
}
