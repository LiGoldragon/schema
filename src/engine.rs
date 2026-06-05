use nota_next::{Block, Delimiter, Document, NotaBody};

use crate::{
    ImportResolver, SchemaSource,
    declarative::{AssembledStructBody, AssembledVariants},
    macros::{
        MacroContext, MacroNodeDefinition, MacroObject, MacroOutput, MacroPair, MacroPosition,
        MacroRegistry, SchemaBlockExt, SchemaMacroHandler,
    },
    schema::{
        AliasDeclaration, Declaration, EnumDeclaration, EnumVariant, ImportDeclaration, Name,
        Schema, TypeDeclaration, TypeReference,
    },
};

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::NotaDecode,
    nota_next::NotaEncode,
    Clone,
    Debug,
    Eq,
    PartialEq,
)]
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
    NotaDecode(String),
    ArchiveEncode,
    ArchiveDecode,
    ExpectedRootObjectCount {
        expected: &'static str,
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
    MalformedSchemaPath {
        path: String,
    },
    ExpectedSymbol {
        found: String,
    },
    ExpectedEnumVariant,
    MalformedSchemaNode {
        found: String,
    },
    UnsupportedMacroNodeStructure {
        position: String,
        expected: Vec<String>,
        found: String,
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
    EmptyTypeReference,
    UnknownTypeReferenceForm {
        head: String,
        argument_count: usize,
    },
    ReservedScalarTypeName {
        name: String,
    },
    MalformedImportSource {
        found: String,
    },
    UnresolvedImportCrate {
        crate_name: String,
    },
    ImportedTypeNotFound {
        crate_name: String,
        module: String,
        type_name: String,
    },
    ExpectedRawDeclarationName {
        found: String,
    },
    RawDeclarationNameMismatch {
        key: String,
        declared: String,
    },
    ExpectedRawFieldPairCount {
        declaration: String,
        found: usize,
    },
    ExpectedSyntaxDeclaration {
        found: String,
    },
    ExpectedSyntaxReference {
        found: String,
    },
    ExpectedSyntaxReferenceArity {
        form: &'static str,
        expected: &'static str,
        found: usize,
    },
    ExpectedSyntaxEnumVariant {
        found: String,
    },
    DuplicateSourceDeclaration {
        name: String,
    },
    SchemaEditTargetNotFound {
        type_name: String,
    },
    SchemaEditExpectedStruct {
        type_name: String,
    },
    SchemaEditExpectedEnum {
        type_name: String,
    },
    SchemaEditDuplicateField {
        type_name: String,
        field_name: String,
    },
    SchemaEditDuplicateVariant {
        type_name: String,
        variant_name: String,
    },
    SchemaEditFieldNotFound {
        type_name: String,
        field_name: String,
    },
    SchemaEditIdentityMismatch {
        expected: String,
        found: String,
    },
}

impl From<nota_next::NotaError> for SchemaError {
    fn from(value: nota_next::NotaError) -> Self {
        Self::Nota(value.to_string())
    }
}

impl From<nota_next::NotaDecodeError> for SchemaError {
    fn from(value: nota_next::NotaDecodeError) -> Self {
        Self::NotaDecode(value.to_string())
    }
}

impl From<nota_next::MacroError> for SchemaError {
    fn from(value: nota_next::MacroError) -> Self {
        match value {
            nota_next::MacroError::NoMatch {
                position,
                expected,
                found,
                ..
            } => Self::UnsupportedMacroNodeStructure {
                position,
                expected,
                found,
            },
            nota_next::MacroError::Conflict(conflict) => Self::UnsupportedMacroNodeStructure {
                position: "structural macro registry".to_owned(),
                expected: vec![format!(
                    "non-conflicting macro cases, found conflict between {} and {}",
                    conflict.first(),
                    conflict.second()
                )],
                found: "conflicting structural macro definitions".to_owned(),
            },
        }
    }
}

impl From<nota_next::StructuralVariantError> for SchemaError {
    fn from(value: nota_next::StructuralVariantError) -> Self {
        match value {
            nota_next::StructuralVariantError::NoMatch {
                position,
                expected,
                found,
                ..
            } => Self::UnsupportedMacroNodeStructure {
                position,
                expected,
                found,
            },
            nota_next::StructuralVariantError::Conflict(conflict) => {
                Self::UnsupportedMacroNodeStructure {
                    position: "structural macro node enum".to_owned(),
                    expected: vec![format!(
                        "non-conflicting structural variants, found conflict between {} and {}",
                        conflict.first(),
                        conflict.second()
                    )],
                    found: "conflicting structural macro node variants".to_owned(),
                }
            }
        }
    }
}

impl From<nota_next::StructuralMacroError<SchemaError>> for SchemaError {
    fn from(value: nota_next::StructuralMacroError<SchemaError>) -> Self {
        match value {
            nota_next::StructuralMacroError::Parse { error } => Self::Nota(error),
            nota_next::StructuralMacroError::ExpectedSingleRoot { found } => {
                Self::ExpectedRootObjectCount {
                    expected: "one structural macro node root object",
                    found,
                }
            }
            nota_next::StructuralMacroError::Dispatch(error) => Self::from(error),
            nota_next::StructuralMacroError::MatchedNode(error) => error,
        }
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
    ) -> Result<Schema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document(&document, identity)
    }

    pub fn lower_schema_source(
        &self,
        source: &SchemaSource,
        identity: SchemaIdentity,
    ) -> Result<Schema, SchemaError> {
        self.lower_schema_source_with_resolver(source, identity, &ImportResolver::new())
    }

    pub fn lower_schema_source_with_resolver(
        &self,
        source: &SchemaSource,
        identity: SchemaIdentity,
        resolver: &ImportResolver,
    ) -> Result<Schema, SchemaError> {
        let imports = source.imports().to_schema_imports()?;
        let resolved_imports = resolver.resolve_all(&imports, self)?;
        source.to_schema(identity, imports, resolved_imports)
    }

    pub fn lower_source_with_context(
        &self,
        source: &str,
        identity: SchemaIdentity,
        context: &mut MacroContext,
    ) -> Result<Schema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document_with_context(&document, identity, context)
    }

    pub fn lower_document(
        &self,
        document: &Document,
        identity: SchemaIdentity,
    ) -> Result<Schema, SchemaError> {
        self.lower_document_with_context(document, identity, &mut MacroContext::default())
    }

    pub fn lower_document_with_context(
        &self,
        document: &Document,
        identity: SchemaIdentity,
        context: &mut MacroContext,
    ) -> Result<Schema, SchemaError> {
        self.lower_document_with_resolver(document, identity, context, &ImportResolver::new())
    }

    /// Lower a document, resolving its imports against `resolver`.
    ///
    /// This is the cross-crate boundary: the consumer build script
    /// registers dependency crate schema directories on the resolver,
    /// and the resolver turns each collected import declaration into a
    /// resolved import that the Rust emitter can use as a `pub use`
    /// alias instead of re-declaring the dependency's type.
    pub fn lower_document_with_resolver(
        &self,
        document: &Document,
        identity: SchemaIdentity,
        context: &mut MacroContext,
        resolver: &ImportResolver,
    ) -> Result<Schema, SchemaError> {
        context.remember_structure_header(document.structure_header());

        if !matches!(document.holds_root_objects(), 3 | 4) {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: "3 root values (input output namespace) or 4 with leading imports",
                found: document.holds_root_objects(),
            });
        }

        let (imports, input_index, output_index, namespace_index) =
            if document.holds_root_objects() == 4 {
                (
                    self.lower_imports(
                        document.root_object_at(0).expect("checked root count"),
                        context,
                    )?,
                    1,
                    2,
                    3,
                )
            } else {
                (Vec::new(), 0, 1, 2)
            };
        let resolved_imports = resolver.resolve_all(&imports, self)?;
        let input = self.lower_root_enum(
            document
                .root_object_at(input_index)
                .expect("checked root count"),
            MacroPosition::RootInput,
            context,
        )?;
        let output = self.lower_root_enum(
            document
                .root_object_at(output_index)
                .expect("checked root count"),
            MacroPosition::RootOutput,
            context,
        )?;
        let namespace = self.lower_namespace(
            document
                .root_object_at(namespace_index)
                .expect("checked root count"),
            context,
        )?;

        Ok(Schema::new(
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
            Vec::new(),
        ))
    }

    pub fn lower_source_with_resolver(
        &self,
        source: &str,
        identity: SchemaIdentity,
        context: &mut MacroContext,
        resolver: &ImportResolver,
    ) -> Result<Schema, SchemaError> {
        let document = Document::parse(source)?;
        self.lower_document_with_resolver(&document, identity, context, resolver)
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
    ) -> Result<Vec<Declaration>, SchemaError> {
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
    pub fn with_schema_defaults() -> Self {
        let mut registry = Self::new();
        registry.register_node_definition(MacroNodeDefinition::root_imports());
        registry.register_node_definition(MacroNodeDefinition::root_input());
        registry.register_node_definition(MacroNodeDefinition::root_output());
        registry.register_node_definition(MacroNodeDefinition::root_namespace());
        registry.register_node_definition(MacroNodeDefinition::namespace_declaration());
        registry.register_node_definition(MacroNodeDefinition::struct_fields());
        registry.register_node_definition(MacroNodeDefinition::enum_variants());
        registry.register_node_definition(MacroNodeDefinition::type_reference());
        registry.register(RootImportsMacro::new());
        registry.register(RootEnumMacro::new(
            "RootInput",
            MacroPosition::RootInput,
            "Input",
        ));
        registry.register(RootEnumMacro::new(
            "RootOutput",
            MacroPosition::RootOutput,
            "Output",
        ));
        registry.register(RootNamespaceMacro::new());
        registry.register(KeyValueDeclarationMacro::new());
        registry
    }
}

#[derive(Clone, Debug)]
struct KeyValueDeclarationMacro {
    signature: MacroSignature,
    node: MacroNodeDefinition,
}

impl KeyValueDeclarationMacro {
    fn new() -> Self {
        Self {
            signature: MacroSignature::new(
                "KeyValueDeclaration",
                MacroPosition::NamespaceDeclaration,
                "Name value",
            ),
            node: MacroNodeDefinition::namespace_declaration(),
        }
    }
}

impl SchemaMacroHandler for KeyValueDeclarationMacro {
    fn name(&self) -> &str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position) && self.node.matches(object)
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        self.signature.remember(position, context);
        let pair = object.pair().ok_or(SchemaError::ExpectedDelimiter {
            expected: self.signature.expected_delimiter(),
        })?;
        KeyValueDeclaration::new(pair)
            .lower(registry, context)
            .map(MacroOutput::Type)
    }
}

#[derive(Clone, Copy, Debug)]
struct KeyValueDeclaration<'schema> {
    pair: MacroPair<'schema>,
}

impl<'schema> KeyValueDeclaration<'schema> {
    fn new(pair: MacroPair<'schema>) -> Self {
        Self { pair }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.pair.name.schema_name()?;
        match self.pair.definition {
            Block::Delimited {
                delimiter: nota_next::Delimiter::Brace,
                root_objects,
                ..
            } => self.lower_struct(name, root_objects, registry, context),
            Block::Delimited {
                delimiter: nota_next::Delimiter::SquareBracket,
                root_objects,
                ..
            } => self.lower_enum(name, root_objects, registry, context),
            definition => self.lower_newtype(name, definition, registry, context),
        }
    }

    fn lower_struct(
        &self,
        name: Name,
        root_objects: &'schema [Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        AssembledStructBody::from_blocks(name, root_objects).lower_type(registry, context)
    }

    fn lower_enum(
        &self,
        name: Name,
        root_objects: &'schema [Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let variants = AssembledVariants::new(root_objects).lower(registry, context)?;
        Ok(TypeDeclaration::Enum(EnumDeclaration::new(name, variants)))
    }

    fn lower_newtype(
        &self,
        name: Name,
        definition: &'schema Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        if matches!(
            definition,
            Block::Delimited {
                delimiter: nota_next::Delimiter::PipeBrace | nota_next::Delimiter::PipeParenthesis,
                ..
            }
        ) {
            return Err(SchemaError::ExpectedDelimiter {
                expected: "namespace value reference, not pipe declaration block",
            });
        }
        let reference = TypeReference::from_block_with_registry(definition, registry, context)?;
        Ok(TypeDeclaration::Alias(AliasDeclaration::new(
            name, reference,
        )))
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

impl SchemaMacroHandler for RootImportsMacro {
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
        let body = object.delimited_body(Delimiter::Brace, self.signature.expected_delimiter())?;
        if body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: body.root_objects().len(),
            });
        }

        let mut imports = Vec::new();
        for chunk in body.root_objects().chunks_exact(2) {
            let local_name = chunk[0].schema_name()?;
            let source = chunk[1].schema_name()?;
            imports.push(ImportDeclaration {
                local_name,
                source: TypeReference::from_name(source),
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

impl SchemaMacroHandler for RootNamespaceMacro {
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
        let body = object.delimited_body(Delimiter::Brace, self.signature.expected_delimiter())?;
        Ok(MacroOutput::Types(
            NamespaceBlock::new(body).lower_declarations(registry, context)?,
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct NamespaceBlock<'schema> {
    body: NotaBody<'schema>,
}

impl<'schema> NamespaceBlock<'schema> {
    fn new(body: NotaBody<'schema>) -> Self {
        Self { body }
    }

    fn lower_declarations(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<Declaration>, SchemaError> {
        self.lower_key_value_declarations(registry, context)
    }

    fn lower_key_value_declarations(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<Declaration>, SchemaError> {
        let mut declarations = Vec::new();
        for pair in self.key_value_pairs()? {
            let name = pair.name.schema_name()?;
            if TypeReference::is_reserved_scalar_name(&name) {
                return Err(SchemaError::ReservedScalarTypeName {
                    name: name.as_str().to_owned(),
                });
            }
            self.push_declaration(
                MacroObject::Pair(pair),
                registry,
                context,
                &mut declarations,
            )?;
        }
        Ok(declarations)
    }

    fn key_value_pairs(&self) -> Result<Vec<MacroPair<'schema>>, SchemaError> {
        if self.body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: self.body.root_objects().len(),
            });
        }
        Ok(self
            .body
            .root_objects()
            .chunks_exact(2)
            .map(|chunk| MacroPair {
                name: &chunk[0],
                definition: &chunk[1],
            })
            .collect())
    }

    fn push_declaration(
        &self,
        object: MacroObject<'schema>,
        registry: &MacroRegistry,
        context: &mut MacroContext,
        declarations: &mut Vec<Declaration>,
    ) -> Result<(), SchemaError> {
        let inline_start = context.inline_declaration_count();
        match registry.lower(object, MacroPosition::NamespaceDeclaration, context)? {
            MacroOutput::Type(declaration) => {
                declarations.extend(context.drain_inline_declarations_from(inline_start));
                declarations.push(Declaration::public(declaration));
            }
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
    enum_name: &'static str,
}

impl RootEnumMacro {
    fn new(name: &'static str, position: MacroPosition, enum_name: &'static str) -> Self {
        Self {
            signature: MacroSignature::new(name, position, "[ ]"),
            enum_name,
        }
    }
}

impl SchemaMacroHandler for RootEnumMacro {
    fn name(&self) -> &str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
            && object.block().is_some_and(Block::is_square_bracket)
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
        let root_enum = RootEnumBlock::from_block(object, self.enum_name)?;
        let name = root_enum.name();
        let variants = root_enum.variants(registry, context)?;
        Ok(MacroOutput::RootEnum(EnumDeclaration::new(name, variants)))
    }
}

#[derive(Clone, Copy, Debug)]
struct RootEnumBlock<'schema> {
    variants: &'schema [Block],
    enum_name: &'static str,
}

impl<'schema> RootEnumBlock<'schema> {
    fn from_block(object: &'schema Block, enum_name: &'static str) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(object, Delimiter::SquareBracket, "root enum body")?;
        Ok(Self {
            variants: body.root_objects(),
            enum_name,
        })
    }

    fn name(&self) -> Name {
        Name::new(self.enum_name)
    }

    fn variants(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        AssembledVariants::new(self.variants).lower(registry, context)
    }
}
