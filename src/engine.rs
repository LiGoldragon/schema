use nota_next::{Block, Document};

use crate::{
    asschema::{
        Asschema, EnumDeclaration, EnumVariant, FieldDeclaration, ImportDeclaration, Name,
        RootSurface, StructDeclaration, TypeDeclaration, TypeReference,
    },
    macros::{
        MacroContext, MacroObject, MacroOutput, MacroPair, MacroPosition, MacroRegistry,
        SchemaMacro, atom_name,
    },
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
    ExpectedRootObjectCount {
        expected: usize,
        found: usize,
    },
    ExpectedDelimiter {
        expected: &'static str,
    },
    ExpectedEvenMapEntries {
        found: usize,
    },
    ExpectedSymbol {
        found: String,
    },
    ExpectedSurfaceVariant,
    MacroDidNotMatch {
        macro_name: &'static str,
    },
    UnexpectedMacroOutput {
        macro_name: &'static str,
        expected: &'static str,
    },
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

pub struct SchemaEngine {
    registry: MacroRegistry,
}

impl Default for SchemaEngine {
    fn default() -> Self {
        Self {
            registry: default_registry(),
        }
    }
}

impl SchemaEngine {
    pub fn with_registry(registry: MacroRegistry) -> Self {
        Self { registry }
    }

    pub fn lower_source(
        &self,
        source: &str,
        identity: SchemaIdentity,
    ) -> Result<Asschema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document(&document, identity)
    }

    pub fn lower_source_with_context(
        &self,
        source: &str,
        identity: SchemaIdentity,
        context: &mut MacroContext,
    ) -> Result<Asschema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document_with_context(&document, identity, context)
    }

    pub fn lower_document(
        &self,
        document: &Document,
        identity: SchemaIdentity,
    ) -> Result<Asschema, SchemaError> {
        self.lower_document_with_context(document, identity, &mut MacroContext::default())
    }

    pub fn lower_document_with_context(
        &self,
        document: &Document,
        identity: SchemaIdentity,
        context: &mut MacroContext,
    ) -> Result<Asschema, SchemaError> {
        if document.holds_root_objects() != 3 {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: 3,
                found: document.holds_root_objects(),
            });
        }

        let imports = self.lower_imports(
            document.root_object_at(0).expect("checked root count"),
            context,
        )?;
        let surfaces = self.lower_surfaces(
            document.root_object_at(1).expect("checked root count"),
            context,
        )?;
        let namespace = self.lower_namespace(
            document.root_object_at(2).expect("checked root count"),
            context,
        )?;

        Ok(Asschema::new(identity, imports, surfaces, namespace))
    }

    fn lower_imports(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<ImportDeclaration>, SchemaError> {
        match self.registry.lower(
            MacroObject::Block(object),
            MacroPosition::RootImports,
            context,
        )? {
            MacroOutput::Imports(imports) => Ok(imports),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "RootImports",
                expected: "imports",
            }),
        }
    }

    fn lower_surfaces(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<RootSurface>, SchemaError> {
        match self.registry.lower(
            MacroObject::Block(object),
            MacroPosition::RootSurfaces,
            context,
        )? {
            MacroOutput::Surfaces(surfaces) => Ok(surfaces),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "RootSurfaces",
                expected: "surfaces",
            }),
        }
    }

    fn lower_namespace(
        &self,
        object: &Block,
        context: &mut MacroContext,
    ) -> Result<Vec<TypeDeclaration>, SchemaError> {
        match self.registry.lower(
            MacroObject::Block(object),
            MacroPosition::RootNamespace,
            context,
        )? {
            MacroOutput::Types(types) => Ok(types),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "RootNamespace",
                expected: "types",
            }),
        }
    }
}

fn default_registry() -> MacroRegistry {
    let mut registry = MacroRegistry::new();
    registry.register(RootImportsMacro);
    registry.register(RootSurfacesMacro);
    registry.register(RootNamespaceMacro);
    registry.register(SurfaceMacro);
    registry.register(TypeDeclarationMacro);
    registry.register(StructFieldsMacro);
    registry.register(EnumVariantsMacro);
    registry
}

#[derive(Clone, Debug, Default)]
struct RootImportsMacro;

impl SchemaMacro for RootImportsMacro {
    fn name(&self) -> &'static str {
        "RootImports"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::RootImports
            && object.block().is_some_and(|block| block.is_brace())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "{ }" })?;
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
        Ok(MacroOutput::Imports(imports))
    }
}

#[derive(Clone, Debug, Default)]
struct RootSurfacesMacro;

impl SchemaMacro for RootSurfacesMacro {
    fn name(&self) -> &'static str {
        "RootSurfaces"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::RootSurfaces
            && object
                .block()
                .is_some_and(|block| block.is_square_bracket())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "[ ]" })?;
        if !object.is_square_bracket() {
            return Err(SchemaError::ExpectedDelimiter { expected: "[ ]" });
        }

        let mut surfaces = Vec::new();
        for index in 0..object.holds_root_objects() {
            let child = object
                .root_object_at(index)
                .expect("index within surface count");
            match registry.lower(MacroObject::Block(child), MacroPosition::Surface, context)? {
                MacroOutput::Surface(surface) => surfaces.push(surface),
                _ => {
                    return Err(SchemaError::UnexpectedMacroOutput {
                        macro_name: "Surface",
                        expected: "surface",
                    });
                }
            }
        }
        Ok(MacroOutput::Surfaces(surfaces))
    }
}

#[derive(Clone, Debug, Default)]
struct RootNamespaceMacro;

impl SchemaMacro for RootNamespaceMacro {
    fn name(&self) -> &'static str {
        "RootNamespace"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::RootNamespace
            && object.block().is_some_and(|block| block.is_brace())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "{ }" })?;
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
            let pair = MacroPair {
                name: object
                    .root_object_at(index)
                    .expect("index within namespace object count"),
                definition: object
                    .root_object_at(index + 1)
                    .expect("index within namespace object count"),
            };
            match registry.lower(
                MacroObject::Pair(pair),
                MacroPosition::NamespaceDeclaration,
                context,
            )? {
                MacroOutput::Type(declaration) => declarations.push(declaration),
                _ => {
                    return Err(SchemaError::UnexpectedMacroOutput {
                        macro_name: "TypeDeclaration",
                        expected: "type",
                    });
                }
            }
        }
        Ok(MacroOutput::Types(declarations))
    }
}

#[derive(Clone, Debug, Default)]
struct SurfaceMacro;

impl SchemaMacro for SurfaceMacro {
    fn name(&self) -> &'static str {
        "Surface"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::Surface
            && object.block().is_some_and(|object| {
                object.is_parenthesis()
                    && object.holds_root_objects() >= 1
                    && object
                        .root_object_at(0)
                        .is_some_and(Block::qualifies_as_pascal_case_symbol)
            })
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "( )" })?;
        let name = atom_name(
            object
                .root_object_at(0)
                .expect("surface match checked first object"),
        )?;
        let mut variants = Vec::new();
        for index in 1..object.holds_root_objects() {
            variants.push(lower_variant(
                object
                    .root_object_at(index)
                    .expect("index within surface object count"),
            )?);
        }
        Ok(MacroOutput::Surface(RootSurface { name, variants }))
    }
}

fn lower_variant(object: &Block) -> Result<EnumVariant, SchemaError> {
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

impl SchemaMacro for TypeDeclarationMacro {
    fn name(&self) -> &'static str {
        "TypeDeclaration"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::NamespaceDeclaration
            && object.pair().is_some_and(|pair| {
                pair.name.qualifies_as_pascal_case_symbol()
                    && (pair.definition.is_square_bracket() || pair.definition.is_parenthesis())
            })
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let pair = object.pair().ok_or(SchemaError::ExpectedDelimiter {
            expected: "namespace pair",
        })?;
        let name = atom_name(pair.name)?;
        if pair.definition.is_square_bracket() {
            let fields = match registry.lower(
                MacroObject::Block(pair.definition),
                MacroPosition::StructFields,
                context,
            )? {
                MacroOutput::Fields(fields) => fields,
                _ => {
                    return Err(SchemaError::UnexpectedMacroOutput {
                        macro_name: "StructFields",
                        expected: "fields",
                    });
                }
            };
            let declaration = StructDeclaration { name, fields };
            if declaration.fields.len() == 1 {
                Ok(MacroOutput::Type(TypeDeclaration::Newtype(declaration)))
            } else {
                Ok(MacroOutput::Type(TypeDeclaration::Struct(declaration)))
            }
        } else if pair.definition.is_parenthesis() {
            let variants = match registry.lower(
                MacroObject::Block(pair.definition),
                MacroPosition::EnumVariants,
                context,
            )? {
                MacroOutput::Variants(variants) => variants,
                _ => {
                    return Err(SchemaError::UnexpectedMacroOutput {
                        macro_name: "EnumVariants",
                        expected: "variants",
                    });
                }
            };
            Ok(MacroOutput::Type(TypeDeclaration::Enum(EnumDeclaration {
                name,
                variants,
            })))
        } else {
            Err(SchemaError::ExpectedDelimiter {
                expected: "[ ] or ( )",
            })
        }
    }
}

#[derive(Clone, Debug, Default)]
struct StructFieldsMacro;

impl SchemaMacro for StructFieldsMacro {
    fn name(&self) -> &'static str {
        "StructFields"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::StructFields
            && object
                .block()
                .is_some_and(|object| object.is_square_bracket())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "[ ]" })?;
        let mut fields = Vec::new();
        for index in 0..object.holds_root_objects() {
            let name = atom_name(object.root_object_at(index).expect("field index in bounds"))?;
            fields.push(FieldDeclaration {
                name: Name::new(name.field_name()),
                reference: TypeReference { name },
            });
        }
        Ok(MacroOutput::Fields(fields))
    }
}

#[derive(Clone, Debug, Default)]
struct EnumVariantsMacro;

impl SchemaMacro for EnumVariantsMacro {
    fn name(&self) -> &'static str {
        "EnumVariants"
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == MacroPosition::EnumVariants
            && object.block().is_some_and(|object| object.is_parenthesis())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        context.remember_macro(self.name());
        context.remember_position(position);
        let object = object
            .block()
            .ok_or(SchemaError::ExpectedDelimiter { expected: "( )" })?;
        let mut variants = Vec::new();
        for index in 0..object.holds_root_objects() {
            let child = object
                .root_object_at(index)
                .expect("variant index in bounds");
            variants.push(if child.is_parenthesis() {
                lower_variant(child)?
            } else {
                EnumVariant {
                    name: atom_name(child)?,
                    payload: None,
                }
            });
        }
        Ok(MacroOutput::Variants(variants))
    }
}
