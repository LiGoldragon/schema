use nota_next::{Block, Document};

use crate::{
    asschema::{
        Asschema, EnumDeclaration, EnumVariant, ImportDeclaration, Name, TypeDeclaration,
        TypeReference,
    },
    declarative::DeclarativeMacroLibrary,
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
    Io {
        path: String,
        reason: String,
    },
    ExpectedSymbol {
        found: String,
    },
    ExpectedEnumVariant,
    ExpectedEvenBraceEnumPairs {
        found: usize,
    },
    MacroDidNotMatch {
        macro_name: String,
    },
    UnexpectedMacroOutput {
        macro_name: String,
        expected: &'static str,
    },
    ExpectedMacroDefinition {
        found: String,
    },
    UnknownMacroPosition {
        found: String,
    },
    InvalidMacroCapture {
        found: String,
    },
    MissingMacroBinding {
        name: String,
    },
    ConflictingMacroBinding {
        name: String,
    },
    UnknownAssembledTemplate {
        found: String,
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
        if document.holds_root_objects() != 4 {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: 4,
                found: document.holds_root_objects(),
            });
        }

        let imports = self.lower_imports(
            document.root_object_at(0).expect("checked root count"),
            context,
        )?;
        let input = self.lower_root_enum(
            document.root_object_at(1).expect("checked root count"),
            MacroPosition::RootInput,
            context,
        )?;
        let output = self.lower_root_enum(
            document.root_object_at(2).expect("checked root count"),
            MacroPosition::RootOutput,
            context,
        )?;
        let namespace = self.lower_namespace(
            document.root_object_at(3).expect("checked root count"),
            context,
        )?;

        Ok(Asschema::new(identity, imports, input, output, namespace))
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
                macro_name: "RootImports".to_owned(),
                expected: "imports",
            }),
        }
    }

    fn lower_root_enum(
        &self,
        object: &Block,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<EnumDeclaration, SchemaError> {
        match self
            .registry
            .lower(MacroObject::Block(object), position, context)?
        {
            MacroOutput::RootEnum(declaration) => Ok(declaration),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "RootEnum".to_owned(),
                expected: "root enum",
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
                macro_name: "RootNamespace".to_owned(),
                expected: "types",
            }),
        }
    }
}

impl MacroRegistry {
    pub(crate) fn with_schema_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(RootImportsMacro::new());
        registry.register(RootEnumMacro::new("RootInput", MacroPosition::RootInput));
        registry.register(RootEnumMacro::new("RootOutput", MacroPosition::RootOutput));
        registry.register(RootNamespaceMacro::new());
        registry.register(BraceEnumVariantsMacro::new());
        for schema_macro in DeclarativeMacroLibrary::builtin()
            .expect("builtin schema macros parse")
            .into_macros()
        {
            registry.register_box(schema_macro);
        }
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
    fn name(&self) -> &str {
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
    fn name(&self) -> &str {
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

    fn lower_declarations(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<TypeDeclaration>, SchemaError> {
        let mut declarations = Vec::new();
        if self.object.holds_root_objects() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: self.object.holds_root_objects(),
            });
        }
        for index in (0..self.object.holds_root_objects()).step_by(2) {
            self.object
                .root_object_at(index)
                .expect("index within namespace object count")
                .schema_name()?;
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
                    macro_name: "TypeDeclaration".to_owned(),
                    expected: "type",
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct RootEnumMacro {
    signature: MacroSignature,
}

impl RootEnumMacro {
    fn new(name: &'static str, position: MacroPosition) -> Self {
        Self {
            signature: MacroSignature::new(name, position, "( )"),
        }
    }
}

impl SchemaMacro for RootEnumMacro {
    fn name(&self) -> &str {
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
        let root_enum = RootEnumBlock::new(object);
        let name = root_enum.name()?;
        let variants = root_enum.variants(registry, context)?;
        Ok(MacroOutput::RootEnum(EnumDeclaration { name, variants }))
    }
}

#[derive(Clone, Copy, Debug)]
struct RootEnumBlock<'schema> {
    object: &'schema Block,
}

impl<'schema> RootEnumBlock<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn name(&self) -> Result<Name, SchemaError> {
        self.object
            .root_object_at(0)
            .expect("root enum match checked first object")
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
                .is_some_and(|object| object.is_parenthesis() || object.is_brace())
        {
            return self.variants_from_nested_enum(registry, context);
        }

        let mut variants = Vec::new();
        for index in 1..self.object.holds_root_objects() {
            variants.push(
                SchemaVariant::new(
                    self.object
                        .root_object_at(index)
                        .expect("index within root enum object count"),
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
                    .expect("root enum match checked variant object"),
            ),
            MacroPosition::EnumVariants,
            context,
        )? {
            MacroOutput::Variants(variants) => Ok(variants),
            _ => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: "EnumVariants".to_owned(),
                expected: "variants",
            }),
        }
    }
}

/// Rust macro for the brace-form payload-carrying enum body.
///
/// Pairs up the children of a brace into `(Name Payload)` variants —
/// `{Variant1 Payload1 Variant2 Payload2}` lowers to
/// `[(Variant1, Payload1), (Variant2, Payload2)]`. Unit-variant brace
/// input (odd count, or any pair whose payload isn't a PascalCase
/// symbol the schema can read as a type reference) errors loud rather
/// than silently producing the wrong shape.
#[derive(Clone, Debug)]
struct BraceEnumVariantsMacro {
    signature: MacroSignature,
}

impl BraceEnumVariantsMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new("BraceEnumVariants", MacroPosition::EnumVariants, "{ }"),
        }
    }
}

impl SchemaMacro for BraceEnumVariantsMacro {
    fn name(&self) -> &str {
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
        BraceEnumVariantsBody::new(object).lower_variants()
    }
}

#[derive(Clone, Copy, Debug)]
struct BraceEnumVariantsBody<'schema> {
    object: &'schema Block,
}

impl<'schema> BraceEnumVariantsBody<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn lower_variants(&self) -> Result<MacroOutput, SchemaError> {
        let count = self.object.holds_root_objects();
        if count % 2 != 0 {
            return Err(SchemaError::ExpectedEvenBraceEnumPairs { found: count });
        }
        let mut variants = Vec::with_capacity(count / 2);
        for index in (0..count).step_by(2) {
            let name = self
                .object
                .root_object_at(index)
                .expect("index within brace enum object count")
                .schema_name()?;
            let payload = TypeReference {
                name: self
                    .object
                    .root_object_at(index + 1)
                    .expect("index within brace enum object count")
                    .schema_name()?,
            };
            variants.push(EnumVariant {
                name,
                payload: Some(payload),
            });
        }
        Ok(MacroOutput::Variants(variants))
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
            Err(SchemaError::ExpectedEnumVariant)
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
            _ => Err(SchemaError::ExpectedEnumVariant),
        }
    }
}
