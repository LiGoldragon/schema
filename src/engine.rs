use nota_next::{Block, Document};

use crate::{
    asschema::{
        Asschema, Declaration, EnumDeclaration, EnumVariant, ImportDeclaration, Name, TypeReference,
    },
    declarative::{AssembledVariants, DeclarativeMacroLibrary},
    macros::{
        MacroContext, MacroDispatch, MacroNodeDefinition, MacroObject, MacroOutput, MacroPosition,
        MacroRegistry, SchemaBlockExt, SchemaMacro,
    },
    resolution::ImportResolver,
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
    RootEnumLabelForbidden {
        label: String,
    },
    RootEnumNameMismatch {
        expected: String,
        found: String,
    },
    MalformedSchemaNode {
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
    ) -> Result<Asschema, SchemaError> {
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

        Ok(Asschema::new(
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
        ))
    }

    pub fn lower_source_with_resolver(
        &self,
        source: &str,
        identity: SchemaIdentity,
        context: &mut MacroContext,
        resolver: &ImportResolver,
    ) -> Result<Asschema, SchemaError> {
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
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::RootImports,
            MacroDispatch::RootPositional,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::RootInput,
            MacroDispatch::RootPositional,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::RootOutput,
            MacroDispatch::RootPositional,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::RootNamespace,
            MacroDispatch::RootPositional,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::NamespaceDeclaration,
            MacroDispatch::Structural,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::StructFields,
            MacroDispatch::Structural,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::EnumVariants,
            MacroDispatch::Structural,
        ));
        registry.register_node_definition(MacroNodeDefinition::new(
            MacroPosition::TypeReference,
            MacroDispatch::StructuralOrTaggedInvocation,
        ));
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
    ) -> Result<Vec<Declaration>, SchemaError> {
        let mut declarations = Vec::new();
        for declaration_object in self.object.root_objects() {
            let name = NamespaceDeclarationBlock::new(declaration_object).name()?;
            if TypeReference::is_reserved_scalar_name(&name) {
                return Err(SchemaError::ReservedScalarTypeName {
                    name: name.as_str().to_owned(),
                });
            }
            self.push_declaration(
                MacroObject::Block(declaration_object),
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

#[derive(Clone, Copy, Debug)]
struct NamespaceDeclarationBlock<'schema> {
    object: &'schema Block,
}

impl<'schema> NamespaceDeclarationBlock<'schema> {
    fn new(object: &'schema Block) -> Self {
        Self { object }
    }

    fn name(&self) -> Result<Name, SchemaError> {
        self.object
            .root_object_at(0)
            .ok_or(SchemaError::ExpectedDelimiter {
                expected: "self-named namespace declaration",
            })?
            .schema_name()
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
            signature: MacroSignature::new(name, position, "Name@[ ]"),
            enum_name,
        }
    }
}

impl SchemaMacro for RootEnumMacro {
    fn name(&self) -> &str {
        self.signature.name()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        self.signature.accepts_position(position)
            && object
                .block()
                .is_some_and(|object| object.is_pipe_parenthesis())
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
        let root_enum = RootEnumBlock::new(object, self.enum_name);
        root_enum.require_named_root()?;
        let name = root_enum.name();
        let variants = root_enum.variants(registry, context)?;
        Ok(MacroOutput::RootEnum(EnumDeclaration::new(name, variants)))
    }
}

#[derive(Clone, Copy, Debug)]
struct RootEnumBlock<'schema> {
    object: &'schema Block,
    enum_name: &'static str,
}

impl<'schema> RootEnumBlock<'schema> {
    fn new(object: &'schema Block, enum_name: &'static str) -> Self {
        Self { object, enum_name }
    }

    fn name(&self) -> Name {
        Name::new(self.enum_name)
    }

    fn require_named_root(&self) -> Result<(), SchemaError> {
        let declared = self
            .object
            .root_object_at(0)
            .and_then(Block::demote_to_string)
            .ok_or_else(|| SchemaError::ExpectedDelimiter {
                expected: "root enum name",
            })?;
        if declared == self.enum_name {
            return Ok(());
        }
        Err(SchemaError::RootEnumNameMismatch {
            expected: self.enum_name.to_owned(),
            found: declared.to_owned(),
        })
    }

    fn variants(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        AssembledVariants::new(&self.object.root_objects()[1..]).lower(registry, context)
    }
}
