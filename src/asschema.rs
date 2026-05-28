use std::fmt;

use nota_next::Block;

use crate::{
    MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry, SchemaError, SchemaMacro,
    macros::SchemaBlockExt,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Name(String);

impl Name {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn namespace_segments(&self) -> Vec<&str> {
        self.0.split(':').collect()
    }

    pub fn local_part(&self) -> &str {
        self.namespace_segments()
            .into_iter()
            .last()
            .expect("split always yields at least one segment")
    }

    pub fn field_name(&self) -> String {
        let mut output = String::new();
        for (index, character) in self.local_part().chars().enumerate() {
            if character.is_ascii_uppercase() {
                if index > 0 {
                    output.push('_');
                }
                output.push(character.to_ascii_lowercase());
            } else if character == '-' {
                output.push('_');
            } else {
                output.push(character);
            }
        }
        output
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asschema {
    identity: super::SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    resolved_imports: Vec<super::ResolvedImport>,
    input: EnumDeclaration,
    output: EnumDeclaration,
    namespace: Vec<TypeDeclaration>,
}

impl Asschema {
    pub(crate) fn new(
        identity: super::SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        resolved_imports: Vec<super::ResolvedImport>,
        input: EnumDeclaration,
        output: EnumDeclaration,
        namespace: Vec<TypeDeclaration>,
    ) -> Self {
        Self {
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
        }
    }

    pub fn identity(&self) -> &super::SchemaIdentity {
        &self.identity
    }

    pub fn imports(&self) -> &[ImportDeclaration] {
        &self.imports
    }

    /// The imports resolved against dependency crate schemas. Empty
    /// when the schema was lowered without an import resolver or when
    /// the schema declares no imports. The Rust emitter reads these to
    /// reference dependency-emitted types instead of re-declaring them.
    pub fn resolved_imports(&self) -> &[super::ResolvedImport] {
        &self.resolved_imports
    }

    pub fn input(&self) -> &EnumDeclaration {
        &self.input
    }

    pub fn output(&self) -> &EnumDeclaration {
        &self.output
    }

    pub fn input_and_output(&self) -> [&EnumDeclaration; 2] {
        [&self.input, &self.output]
    }

    pub fn namespace(&self) -> &[TypeDeclaration] {
        &self.namespace
    }

    pub fn type_named(&self, name: &str) -> Option<&TypeDeclaration> {
        self.namespace
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImportDeclaration {
    pub local_name: Name,
    pub source: TypeReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeDeclaration {
    Struct(StructDeclaration),
    Enum(EnumDeclaration),
    Newtype(StructDeclaration),
}

impl TypeDeclaration {
    pub fn name(&self) -> &Name {
        match self {
            Self::Struct(declaration) | Self::Newtype(declaration) => &declaration.name,
            Self::Enum(declaration) => &declaration.name,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructDeclaration {
    pub name: Name,
    pub fields: Vec<FieldDeclaration>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDeclaration {
    pub name: Name,
    pub reference: TypeReference,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumDeclaration {
    pub name: Name,
    pub variants: Vec<EnumVariant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumVariant {
    pub name: Name,
    pub payload: Option<TypeReference>,
}

/// A type at a reference position — a struct field's type, an enum
/// variant's payload, or an import source.
///
/// A reference is no longer just a bare name: it can wrap that name
/// in a collection or option. `Plain` is the leaf (`Topic`,
/// `Magnitude`); `Vector`, `Map`, and `Optional` carry inner
/// references so the schema can express a vector of proposals, an
/// ordered key-value map of node to config, and an optional config at
/// the positions that previously only held a name. The macro surface
/// forms have an explicit marker head and one grouped input object:
/// `(@Vec (T))`, `(@KeyValue (K V))`, `(@Option (T))`. `@Vec` is a
/// macro marker atom, not a symbol candidate or type name. `Map` is
/// the schema-level ordered key-value collection; the concrete Rust
/// container is the emitter's concern, not this model's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeReference {
    Plain(Name),
    Vector(Box<TypeReference>),
    Map(Box<TypeReference>, Box<TypeReference>),
    Optional(Box<TypeReference>),
}

impl TypeReference {
    pub(crate) fn register_builtin_macros(registry: &mut MacroRegistry) {
        registry.register(TypeReferenceMacro::new(
            "Vec",
            TypeReferenceMacroKind::Vector,
        ));
        registry.register(TypeReferenceMacro::new(
            "Option",
            TypeReferenceMacroKind::Optional,
        ));
        registry.register(TypeReferenceMacro::new(
            "KeyValue",
            TypeReferenceMacroKind::Map,
        ));
    }

    /// Construct a plain (leaf) reference to a named type. This is the
    /// legacy shape every non-collection reference still uses.
    pub fn new(name: impl Into<String>) -> Self {
        Self::Plain(Name::new(name))
    }

    /// The plain name when this reference is a leaf. `None` for a
    /// collection or option reference — those have no single name.
    /// Call sites that structurally know a reference is plain (import
    /// sources, scalar fields in legacy tests) use this.
    pub fn plain_name(&self) -> Option<&Name> {
        match self {
            Self::Plain(name) => Some(name),
            Self::Vector(_) | Self::Map(..) | Self::Optional(_) => None,
        }
    }

    /// Whether this reference is a plain leaf (not a collection).
    pub fn is_plain(&self) -> bool {
        matches!(self, Self::Plain(_))
    }

    /// Lower an already-parsed NOTA block at a reference position into
    /// a `TypeReference`.
    ///
    /// A bare PascalCase symbol (`Topic`, `schema-core:mail:Magnitude`)
    /// lowers to `Plain`. A parenthesised macro marker form lowers to
    /// a collection: `(@Vec (T))` → `Vector`, `(@KeyValue (K V))` →
    /// `Map`, `(@Option (T))` → `Optional`. The inner positions
    /// recurse, so `(@Vec ((@Option (Topic))))` and
    /// `(@KeyValue (NodeName (@Vec (Service))))` nest. The collection
    /// head is explicitly marked with `@`, so it is not interpreted
    /// through `schema_name()` and cannot collide with user types.
    /// nota-next did the structural parse; this is pure semantic
    /// lowering over its `Block`s, not a hand-rolled text parser.
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let mut context = MacroContext::default();
        Self::from_block_with_registry(block, &MacroRegistry::with_schema_defaults(), &mut context)
    }

    pub(crate) fn from_block_with_registry(
        block: &Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        if block.is_parenthesis() {
            return Self::from_macro_invocation(block, registry, context);
        }
        Ok(Self::Plain(block.schema_name()?))
    }

    fn from_macro_invocation(
        block: &Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        let invocation = TypeReferenceMacroInvocation::from_block(block)?;
        if !registry
            .node_definition(MacroPosition::TypeReference)
            .is_some_and(|definition| definition.accepts_named_invocation())
        {
            return Err(SchemaError::MacroDidNotMatch {
                macro_name: invocation.name().to_owned(),
            });
        }
        match registry.lower(
            MacroObject::Block(block),
            MacroPosition::TypeReference,
            context,
        ) {
            Ok(MacroOutput::Reference(reference)) => Ok(reference),
            Ok(_) => Err(SchemaError::UnexpectedMacroOutput {
                macro_name: invocation.name().to_owned(),
                expected: "type reference",
            }),
            Err(SchemaError::MacroDidNotMatch { .. }) => {
                Err(SchemaError::UnknownTypeReferenceForm {
                    head: invocation.name().to_owned(),
                    argument_count: invocation.argument_count(),
                })
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MacroMarker(String);

impl MacroMarker {
    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let found = block
            .demote_to_string()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{block:?}"));
        let Some(name) = found.strip_prefix('@') else {
            return Err(SchemaError::ExpectedMacroMarker { found });
        };
        if name.is_empty() {
            return Err(SchemaError::ExpectedMacroMarker { found });
        }
        Ok(Self(name.to_owned()))
    }

    fn name(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug)]
struct TypeReferenceMacroInvocation<'schema> {
    marker: MacroMarker,
    arguments: &'schema Block,
}

impl<'schema> TypeReferenceMacroInvocation<'schema> {
    fn from_block(block: &'schema Block) -> Result<Self, SchemaError> {
        if !block.is_parenthesis() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: "(@Macro (...))",
            });
        }
        if block.holds_root_objects() != 2 {
            let head = block
                .root_object_at(0)
                .and_then(Block::demote_to_string)
                .unwrap_or("<missing>");
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: head.trim_start_matches('@').to_owned(),
                argument_count: block.holds_root_objects().saturating_sub(1),
            });
        }
        let marker = MacroMarker::from_block(
            block
                .root_object_at(0)
                .ok_or(SchemaError::EmptyTypeReference)?,
        )?;
        let arguments = block.root_object_at(1).expect("root count checked");
        if !arguments.is_parenthesis() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: "( ) grouped macro input",
            });
        }
        Ok(Self { marker, arguments })
    }

    fn name(&self) -> &str {
        self.marker.name()
    }

    fn argument_count(&self) -> usize {
        self.arguments.holds_root_objects()
    }

    fn argument_at(&self, index: usize) -> &Block {
        self.arguments
            .root_object_at(index)
            .expect("argument count checked")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeReferenceMacroKind {
    Vector,
    Map,
    Optional,
}

#[derive(Clone, Debug)]
struct TypeReferenceMacro {
    name: &'static str,
    kind: TypeReferenceMacroKind,
}

impl TypeReferenceMacro {
    fn new(name: &'static str, kind: TypeReferenceMacroKind) -> Self {
        Self { name, kind }
    }

    fn expected_arguments(&self) -> usize {
        match self.kind {
            TypeReferenceMacroKind::Vector | TypeReferenceMacroKind::Optional => 1,
            TypeReferenceMacroKind::Map => 2,
        }
    }
}

impl SchemaMacro for TypeReferenceMacro {
    fn name(&self) -> &str {
        self.name
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        if position != MacroPosition::TypeReference {
            return false;
        }
        let Some(block) = object.block() else {
            return false;
        };
        TypeReferenceMacroInvocation::from_block(block)
            .is_ok_and(|invocation| invocation.name() == self.name)
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        if position != MacroPosition::TypeReference {
            return Err(SchemaError::MacroDidNotMatch {
                macro_name: self.name.to_owned(),
            });
        }
        let invocation = TypeReferenceMacroInvocation::from_block(object.block().ok_or(
            SchemaError::ExpectedDelimiter {
                expected: "(@Macro (...))",
            },
        )?)?;
        if invocation.argument_count() != self.expected_arguments() {
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: invocation.name().to_owned(),
                argument_count: invocation.argument_count(),
            });
        }
        context.remember_macro(self.name);
        context.remember_position(position);
        let reference = match self.kind {
            TypeReferenceMacroKind::Vector => {
                TypeReference::Vector(Box::new(TypeReference::from_block_with_registry(
                    invocation.argument_at(0),
                    registry,
                    context,
                )?))
            }
            TypeReferenceMacroKind::Optional => {
                TypeReference::Optional(Box::new(TypeReference::from_block_with_registry(
                    invocation.argument_at(0),
                    registry,
                    context,
                )?))
            }
            TypeReferenceMacroKind::Map => TypeReference::Map(
                Box::new(TypeReference::from_block_with_registry(
                    invocation.argument_at(0),
                    registry,
                    context,
                )?),
                Box::new(TypeReference::from_block_with_registry(
                    invocation.argument_at(1),
                    registry,
                    context,
                )?),
            ),
        };
        Ok(MacroOutput::Reference(reference))
    }
}
