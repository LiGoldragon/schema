use nota_next::{Block, Document};

use crate::{
    asschema::{
        Asschema, EnumDeclaration, EnumVariant, FieldDeclaration, ImportDeclaration, Name,
        RootSurface, StructDeclaration, TypeDeclaration, TypeReference,
    },
    macros::{
        MacroContext, MacroObject, MacroOutput, MacroPair, MacroPosition, MacroRegistry,
        SchemaBlockExt, SchemaMacro,
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
            registry: MacroRegistry::with_schema_defaults(),
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

impl MacroRegistry {
    pub(crate) fn with_schema_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(RootImportsMacro::new());
        registry.register(RootSurfacesMacro::new());
        registry.register(RootNamespaceMacro::new());
        registry.register(SurfaceMacro::new());
        registry.register(TypeDeclarationMacro::new());
        registry.register(StructFieldsMacro::new());
        registry.register(EnumVariantsMacro::new());
        registry
    }
}

#[derive(Clone, Copy, Debug)]
struct MacroSignature {
    name: &'static str,
    position: MacroPosition,
    expected_delimiter: &'static str,
}

impl MacroSignature {
    fn new(name: &'static str, position: MacroPosition, expected_delimiter: &'static str) -> Self {
        Self {
            name,
            position,
            expected_delimiter,
        }
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn expected_delimiter(&self) -> &'static str {
        self.expected_delimiter
    }

    fn accepts_position(&self, position: MacroPosition) -> bool {
        position == self.position
    }

    fn remember(&self, position: MacroPosition, context: &mut MacroContext) {
        context.remember_macro(self.name);
        context.remember_position(position);
    }
}

#[derive(Clone, Debug)]
struct RootImportsMacro {
    signature: MacroSignature,
}

impl RootImportsMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("RootImports", MacroPosition::RootImports, "{ }"),
        }
    }
}

impl SchemaMacro for RootImportsMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
            && object.block().is_some_and(|block| block.is_brace())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        if !object.is_brace() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: self.signature.expected_delimiter(),
            });
        }
        if object.holds_root_objects() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: object.holds_root_objects(),
            });
        }

        let mut imports = Vec::new();
        for index in (0..object.holds_root_objects()).step_by(2) {
            let local_name = object
                .root_object_at(index)
                .expect("index within map object count")
                .schema_name()?;
            let source = object
                .root_object_at(index + 1)
                .expect("index within map object count")
                .schema_name()?;
            imports.push(ImportDeclaration {
                local_name,
                source: TypeReference { name: source },
            });
        }
        Ok(MacroOutput::Imports(imports))
    }
}

#[derive(Clone, Debug)]
struct RootSurfacesMacro {
    signature: MacroSignature,
}

impl RootSurfacesMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("RootSurfaces", MacroPosition::RootSurfaces, "[ ]"),
        }
    }
}

impl SchemaMacro for RootSurfacesMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
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
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        if !object.is_square_bracket() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: self.signature.expected_delimiter(),
            });
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

#[derive(Clone, Debug)]
struct RootNamespaceMacro {
    signature: MacroSignature,
}

impl RootNamespaceMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("RootNamespace", MacroPosition::RootNamespace, "{ }"),
        }
    }
}

impl SchemaMacro for RootNamespaceMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
            && object.block().is_some_and(|block| block.is_brace())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        if !object.is_brace() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: self.signature.expected_delimiter(),
            });
        }
        Ok(MacroOutput::Types(
            NamespaceBlock::new(object).lower_declarations(registry, context)?,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct NamespaceBlock<'schema> {
    object: &'schema Block,
}

impl<'schema> NamespaceBlock<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn uses_named_objects(&self) -> bool {
        self.object.holds_root_objects() == 0
            || (0..self.object.holds_root_objects()).all(|index| {
                self.object
                    .root_object_at(index)
                    .is_some_and(|child| NamedTypeDefinition::new(child).is_some())
            })
    }

    fn lower_declarations(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<TypeDeclaration>, SchemaError> {
        let mut declarations = Vec::new();
        if self.uses_named_objects() {
            for index in 0..self.object.holds_root_objects() {
                let declaration = self
                    .object
                    .root_object_at(index)
                    .expect("index within namespace object count");
                self.push_declaration(
                    MacroObject::Block(declaration),
                    registry,
                    context,
                    &mut declarations,
                )?;
            }
        } else {
            if self.object.holds_root_objects() % 2 != 0 {
                return Err(SchemaError::ExpectedEvenMapEntries {
                    found: self.object.holds_root_objects(),
                });
            }
            for index in (0..self.object.holds_root_objects()).step_by(2) {
                let pair = MacroPair {
                    name: self
                        .object
                        .root_object_at(index)
                        .expect("index within namespace object count"),
                    definition: self
                        .object
                        .root_object_at(index + 1)
                        .expect("index within namespace object count"),
                };
                self.push_declaration(
                    MacroObject::Pair(pair),
                    registry,
                    context,
                    &mut declarations,
                )?;
            }
        }
        Ok(declarations)
    }

    fn push_declaration(
        &self,
        object: MacroObject<'schema>,
        registry: &MacroRegistry,
        context: &mut MacroContext,
        declarations: &mut Vec<TypeDeclaration>,
    ) -> Result<(), SchemaError> {
        match registry.lower(object, MacroPosition::NamespaceDeclaration, context)? {
            MacroOutput::Type(declaration) => declarations.push(declaration),
            _ => {
                return Err(SchemaError::UnexpectedMacroOutput {
                    macro_name: "TypeDeclaration",
                    expected: "type",
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct SurfaceMacro {
    signature: MacroSignature,
}

impl SurfaceMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("Surface", MacroPosition::Surface, "( )"),
        }
    }
}

impl SchemaMacro for SurfaceMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
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
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        let surface = SurfaceBlock::new(object);
        let name = surface.name()?;
        let variants = surface.variants(registry, context)?;
        Ok(MacroOutput::Surface(RootSurface { name, variants }))
    }
}

#[derive(Clone, Copy, Debug)]
struct SurfaceBlock<'schema> {
    object: &'schema Block,
}

impl<'schema> SurfaceBlock<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn name(&self) -> Result<Name, SchemaError> {
        self.object
            .root_object_at(0)
            .expect("surface match checked first object")
            .schema_name()
    }

    fn variants(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        if self.object.holds_root_objects() == 2
            && self
                .object
                .root_object_at(1)
                .is_some_and(Block::is_parenthesis)
        {
            return self.variants_from_nested_enum(registry, context);
        }

        let mut variants = Vec::new();
        for index in 1..self.object.holds_root_objects() {
            variants.push(
                SchemaVariant::new(
                    self.object
                        .root_object_at(index)
                        .expect("index within surface object count"),
                )
                .lower()?,
            );
        }
        Ok(variants)
    }

    fn variants_from_nested_enum(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        match registry.lower(
            MacroObject::Block(
                self.object
                    .root_object_at(1)
                    .expect("surface match checked variant object"),
            ),
            MacroPosition::EnumVariants,
            context,
        )? {
            MacroOutput::Variants(variants) => Ok(variants),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "EnumVariants",
                expected: "variants",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaVariant<'schema> {
    object: &'schema Block,
}

impl<'schema> SchemaVariant<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn lower(&self) -> Result<EnumVariant, SchemaError> {
        if self.object.is_parenthesis() {
            self.lower_parenthesis()
        } else if self.object.qualifies_as_pascal_case_symbol() {
            Ok(EnumVariant {
                name: self.object.schema_name()?,
                payload: None,
            })
        } else {
            Err(SchemaError::ExpectedSurfaceVariant)
        }
    }

    fn lower_parenthesis(&self) -> Result<EnumVariant, SchemaError> {
        match self.object.holds_root_objects() {
            1 => Ok(EnumVariant {
                name: self
                    .object
                    .root_object_at(0)
                    .expect("count checked")
                    .schema_name()?,
                payload: None,
            }),
            2 => Ok(EnumVariant {
                name: self
                    .object
                    .root_object_at(0)
                    .expect("count checked")
                    .schema_name()?,
                payload: Some(TypeReference {
                    name: self
                        .object
                        .root_object_at(1)
                        .expect("count checked")
                        .schema_name()?,
                }),
            }),
            _ => Err(SchemaError::ExpectedSurfaceVariant),
        }
    }
}

#[derive(Clone, Debug)]
struct TypeDeclarationMacro {
    signature: MacroSignature,
}

impl TypeDeclarationMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new(
                "TypeDeclaration",
                MacroPosition::NamespaceDeclaration,
                "named type definition",
            ),
        }
    }
}

impl SchemaMacro for TypeDeclarationMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        if !self.signature.accepts_position(position) {
            return false;
        }
        NamedTypeDefinition::from_macro_object(object).is_some()
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let definition = NamedTypeDefinition::from_macro_object(object).ok_or(
            SchemaError::ExpectedDelimiter {
                expected: self.signature.expected_delimiter(),
            },
        )?;
        let name = definition.name()?;
        if definition.body().is_square_bracket() {
            let fields = match registry.lower(
                MacroObject::Block(definition.body()),
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
        } else if definition.body().is_parenthesis() {
            let variants = match registry.lower(
                MacroObject::Block(definition.body()),
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

#[derive(Clone, Copy, Debug)]
struct NamedTypeDefinition<'schema> {
    name: &'schema Block,
    body: &'schema Block,
}

impl<'schema> NamedTypeDefinition<'schema> {
    fn new(object: &'schema Block) -> Option<Self> {
        if !object.is_parenthesis() || object.holds_root_objects() != 2 {
            return None;
        }
        let name = object.root_object_at(0).expect("definition shape checked");
        let body = object.root_object_at(1).expect("definition shape checked");
        Self::from_parts(name, body)
    }

    fn from_macro_object(object: MacroObject<'schema>) -> Option<Self> {
        if let Some(pair) = object.pair() {
            return Self::from_parts(pair.name, pair.definition);
        }
        Self::new(object.block()?)
    }

    fn from_parts(name: &'schema Block, body: &'schema Block) -> Option<Self> {
        if name.qualifies_as_pascal_case_symbol()
            && (body.is_square_bracket() || body.is_parenthesis())
        {
            Some(Self { name, body })
        } else {
            None
        }
    }

    fn name(&self) -> Result<Name, SchemaError> {
        self.name.schema_name()
    }

    fn body(&self) -> &'schema Block {
        self.body
    }
}

#[derive(Clone, Debug)]
struct StructFieldsMacro {
    signature: MacroSignature,
}

impl StructFieldsMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("StructFields", MacroPosition::StructFields, "[ ]"),
        }
    }
}

impl SchemaMacro for StructFieldsMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
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
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        let mut fields = Vec::new();
        for index in 0..object.holds_root_objects() {
            let name = object
                .root_object_at(index)
                .expect("field index in bounds")
                .schema_name()?;
            fields.push(FieldDeclaration {
                name: Name::new(name.field_name()),
                reference: TypeReference { name },
            });
        }
        Ok(MacroOutput::Fields(fields))
    }
}

#[derive(Clone, Debug)]
struct EnumVariantsMacro {
    signature: MacroSignature,
}

impl EnumVariantsMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("EnumVariants", MacroPosition::EnumVariants, "( )"),
        }
    }
}

impl SchemaMacro for EnumVariantsMacro {
    fn name(&self) -> &'static str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
            && object.block().is_some_and(|object| object.is_parenthesis())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        _registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let object = object.block().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        let mut variants = Vec::new();
        for index in 0..object.holds_root_objects() {
            let child = object
                .root_object_at(index)
                .expect("variant index in bounds");
            variants.push(SchemaVariant::new(child).lower()?);
        }
        Ok(MacroOutput::Variants(variants))
    }
}
