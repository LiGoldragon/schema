use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

use nota_next::{
    AtomClassification, Block, Delimiter, NotaBlock, NotaDecode, NotaDecodeError,
    NotaDocumentEncode, NotaEncode, NotaNamedDocumentFieldDecode, NotaNamedDocumentFieldEncode,
    NotaSource, NotaString,
};

use crate::{
    MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry, SchemaError,
    declarative::{AssembledFields, AssembledVariants},
    macros::{BlockDebug, SchemaBlockExt},
};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, Hash, PartialEq)]
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

    pub fn qualifies_as_symbol_name(&self) -> bool {
        AtomClassification::classify(self.as_str()) == AtomClassification::SymbolCandidate
    }
}

impl NotaDecode for Name {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        NotaBlock::new(block).parse_string().map(Self::new)
    }
}

impl NotaEncode for Name {
    fn to_nota(&self) -> String {
        if self.qualifies_as_symbol_name() {
            self.as_str().to_owned()
        } else {
            NotaString::new(self.as_str()).format()
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

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
#[nota(known_root)]
pub struct Asschema {
    identity: super::SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    resolved_imports: Vec<super::ResolvedImport>,
    #[nota(name = "Input")]
    input: EnumDeclaration,
    #[nota(name = "Output")]
    output: EnumDeclaration,
    namespace: Vec<Declaration>,
}

impl Asschema {
    pub(crate) fn new(
        identity: super::SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        resolved_imports: Vec<super::ResolvedImport>,
        input: EnumDeclaration,
        output: EnumDeclaration,
        namespace: Vec<Declaration>,
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
        [self.input(), self.output()]
    }

    pub fn root_named(&self, name: &str) -> Option<&EnumDeclaration> {
        self.input_and_output()
            .into_iter()
            .find(|declaration| declaration.name.as_str() == name)
    }

    pub fn namespace(&self) -> &[Declaration] {
        &self.namespace
    }

    pub fn type_named(&self, name: &str) -> Option<&TypeDeclaration> {
        self.namespace
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
            .map(Declaration::value)
    }

    pub fn from_nota_source(source: &str) -> Result<Self, SchemaError> {
        NotaSource::new(source)
            .parse_document_body()
            .map_err(SchemaError::from)
    }

    pub fn to_nota(&self) -> String {
        self.to_nota_document_body().to_nota()
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes).map_err(|_| SchemaError::ArchiveDecode)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|bytes| bytes.to_vec())
            .map_err(|_| SchemaError::ArchiveEncode)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsschemaArtifact {
    asschema: Asschema,
}

impl AsschemaArtifact {
    pub fn new(asschema: Asschema) -> Self {
        Self { asschema }
    }

    pub fn asschema(&self) -> &Asschema {
        &self.asschema
    }

    pub fn into_asschema(self) -> Asschema {
        self.asschema
    }

    pub fn from_nota_source(source: &str) -> Result<Self, SchemaError> {
        Asschema::from_nota_source(source).map(Self::new)
    }

    pub fn to_nota_source(&self) -> String {
        self.asschema.to_nota()
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        Asschema::from_binary_bytes(bytes).map(Self::new)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        self.asschema.to_binary_bytes()
    }

    pub fn read_nota_file(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let artifact_path = AsschemaArtifactPath::new(path.as_ref());
        let source = fs::read_to_string(artifact_path.path())
            .map_err(|error| artifact_path.io_error(error))?;
        Self::from_nota_source(&source)
    }

    pub fn write_nota_file(&self, path: impl AsRef<Path>) -> Result<(), SchemaError> {
        let artifact_path = AsschemaArtifactPath::new(path.as_ref());
        fs::write(artifact_path.path(), self.to_nota_source())
            .map_err(|error| artifact_path.io_error(error))
    }

    pub fn read_binary_file(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let artifact_path = AsschemaArtifactPath::new(path.as_ref());
        let bytes =
            fs::read(artifact_path.path()).map_err(|error| artifact_path.io_error(error))?;
        Self::from_binary_bytes(&bytes)
    }

    pub fn write_binary_file(&self, path: impl AsRef<Path>) -> Result<(), SchemaError> {
        let artifact_path = AsschemaArtifactPath::new(path.as_ref());
        let bytes = self.to_binary_bytes()?;
        fs::write(artifact_path.path(), bytes).map_err(|error| artifact_path.io_error(error))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AsschemaArtifactPath {
    path: PathBuf,
}

impl AsschemaArtifactPath {
    fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn io_error(&self, error: std::io::Error) -> SchemaError {
        SchemaError::Io {
            path: self.path.display().to_string(),
            reason: error.to_string(),
        }
    }
}

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
pub struct ImportDeclaration {
    pub local_name: Name,
    pub source: TypeReference,
}

#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::NotaDecode,
    nota_next::NotaEncode,
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
)]
pub enum Visibility {
    Public,
    Private,
}

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
pub struct Declaration {
    visibility: Visibility,
    name: Name,
    value: TypeDeclaration,
}

impl Declaration {
    pub fn public(value: TypeDeclaration) -> Self {
        Self::new(Visibility::Public, value)
    }

    pub fn private(value: TypeDeclaration) -> Self {
        Self::new(Visibility::Private, value)
    }

    fn new(visibility: Visibility, value: TypeDeclaration) -> Self {
        let name = value.name().clone();
        Self {
            visibility,
            name,
            value,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn is_private(&self) -> bool {
        self.visibility == Visibility::Private
    }

    pub fn value(&self) -> &TypeDeclaration {
        &self.value
    }
}

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
pub enum TypeDeclaration {
    Struct(StructDeclaration),
    Enum(EnumDeclaration),
    Newtype(NewtypeDeclaration),
}

impl TypeDeclaration {
    pub fn name(&self) -> &Name {
        match self {
            Self::Struct(declaration) => &declaration.name,
            Self::Newtype(declaration) => &declaration.name,
            Self::Enum(declaration) => &declaration.name,
        }
    }
}

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
pub struct NewtypeDeclaration {
    pub name: Name,
    pub reference: TypeReference,
}

impl NewtypeDeclaration {
    pub fn new(name: Name, reference: TypeReference) -> Self {
        Self { name, reference }
    }
}

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
pub struct StructDeclaration {
    pub name: Name,
    pub fields: StructFieldMap,
}

impl StructDeclaration {
    pub fn new(name: Name, fields: Vec<FieldDeclaration>) -> Self {
        Self {
            name,
            fields: StructFieldMap::new(fields),
        }
    }
}

/// Ordered key/value representation of a struct definition in asschema.
///
/// A struct declaration's long-form data is a brace-map shape:
/// each field name is the key and each `TypeReference` is the value.
/// The Rust storage preserves source order because rkyv layout and
/// generated struct field order are load-bearing, but the object is
/// semantically a field-name -> type-reference map.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct StructFieldMap {
    entries: Vec<FieldDeclaration>,
}

impl StructFieldMap {
    pub fn new(entries: Vec<FieldDeclaration>) -> Self {
        Self { entries }
    }

    pub fn entries(&self) -> &[FieldDeclaration] {
        &self.entries
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FieldDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn first(&self) -> Option<&FieldDeclaration> {
        self.entries.first()
    }
}

impl std::ops::Deref for StructFieldMap {
    type Target = [FieldDeclaration];

    fn deref(&self) -> &Self::Target {
        self.entries()
    }
}

impl<'fields> IntoIterator for &'fields StructFieldMap {
    type Item = &'fields FieldDeclaration;
    type IntoIter = std::slice::Iter<'fields, FieldDeclaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl NotaDecode for StructFieldMap {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let Block::Delimited {
            delimiter: Delimiter::Brace,
            root_objects,
            ..
        } = block
        else {
            return Err(NotaDecodeError::ExpectedDelimited {
                type_name: "StructFieldMap",
                delimiter: "brace",
            });
        };
        if root_objects.len() % 2 != 0 {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: "StructFieldMap",
                expected: root_objects.len() + 1,
                found: root_objects.len(),
            });
        }
        let mut entries = Vec::new();
        for chunk in root_objects.chunks_exact(2) {
            entries.push(FieldDeclaration {
                name: Name::from_nota_block(&chunk[0])?,
                reference: TypeReference::from_nota_block(&chunk[1])?,
            });
        }
        Ok(Self::new(entries))
    }
}

impl NotaEncode for StructFieldMap {
    fn to_nota(&self) -> String {
        let mut fields = Vec::new();
        for entry in self.entries() {
            fields.push(entry.name.to_nota());
            fields.push(entry.reference.to_nota());
        }
        format!("{{{}}}", fields.join(" "))
    }
}

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
pub struct FieldDeclaration {
    pub name: Name,
    pub reference: TypeReference,
}

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
pub struct EnumDeclaration {
    pub name: Name,
    pub variants: Vec<EnumVariant>,
}

impl EnumDeclaration {
    pub fn new(name: Name, variants: Vec<EnumVariant>) -> Self {
        Self { name, variants }
    }
}

impl NotaNamedDocumentFieldDecode for EnumDeclaration {
    fn from_nota_named_document_field(
        name: &'static str,
        block: &Block,
    ) -> Result<Self, NotaDecodeError> {
        Ok(Self::new(
            Name::new(name),
            Vec::<EnumVariant>::from_nota_block(block)?,
        ))
    }
}

impl NotaNamedDocumentFieldEncode for EnumDeclaration {
    fn to_nota_named_document_field_body(&self) -> String {
        self.variants.to_nota()
    }
}

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
pub struct EnumVariant {
    pub name: Name,
    pub payload: Option<TypeReference>,
}

/// A type at a reference position — a struct field's type, an enum
/// variant's payload, or an import source.
///
/// `String`, `Integer`, `Boolean`, and `Path` are reserved scalar leaves.
/// `Plain` is a declared-name leaf (`Topic`, `Magnitude`). `Vector`,
/// `Map`, and `Optional` carry inner references. These are Schema
/// type-reference objects read over nota-next's parsed structure:
/// `(Vec T)` lowers to `Vector<T>`,
/// `(Map (K V))` lowers to `Map<K, V>`, and `(Optional T)` lowers to
/// `Optional<T>`. Parentheses with other heads remain available for
/// user-declared type-reference macros.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext)),
    serialize_bounds(__S: rkyv::ser::Writer),
    deserialize_bounds(__D::Error: rkyv::rancor::Source)
)]
pub enum TypeReference {
    String,
    Integer,
    Boolean,
    Path,
    Plain(Name),
    Vector(#[rkyv(omit_bounds)] Box<TypeReference>),
    Map(
        #[rkyv(omit_bounds)] Box<TypeReference>,
        #[rkyv(omit_bounds)] Box<TypeReference>,
    ),
    Optional(#[rkyv(omit_bounds)] Box<TypeReference>),
}

impl NotaDecode for TypeReference {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        if let Some(name) = block.demote_to_string() {
            return match name {
                "String" => Ok(Self::String),
                "Integer" => Ok(Self::Integer),
                "Boolean" => Ok(Self::Boolean),
                "Path" => Ok(Self::Path),
                other => Err(NotaDecodeError::UnknownVariant {
                    enum_name: "TypeReference",
                    variant: other.to_owned(),
                }),
            };
        }
        let children = NotaBlock::new(block).expect_children(
            Delimiter::Parenthesis,
            "parenthesis",
            "TypeReference",
            2,
        )?;
        let variant = children[0]
            .demote_to_string()
            .ok_or(NotaDecodeError::ExpectedAtom {
                type_name: "TypeReference variant",
            })?;
        match variant {
            "Plain" => Ok(Self::Plain(Name::from_nota_block(&children[1])?)),
            "Vector" => Ok(Self::Vector(Box::new(Self::from_nota_block(&children[1])?))),
            "Optional" => Ok(Self::Optional(Box::new(Self::from_nota_block(
                &children[1],
            )?))),
            "Map" => Self::from_nota_map_payload(&children[1]),
            other => Err(NotaDecodeError::UnknownVariant {
                enum_name: "TypeReference",
                variant: other.to_owned(),
            }),
        }
    }
}

impl NotaEncode for TypeReference {
    fn to_nota(&self) -> String {
        match self {
            Self::String => "String".to_owned(),
            Self::Integer => "Integer".to_owned(),
            Self::Boolean => "Boolean".to_owned(),
            Self::Path => "Path".to_owned(),
            Self::Plain(name) => format!("(Plain {})", name.to_nota()),
            Self::Vector(reference) => format!("(Vector {})", reference.to_nota()),
            Self::Map(key, value) => format!("(Map ({} {}))", key.to_nota(), value.to_nota()),
            Self::Optional(reference) => format!("(Optional {})", reference.to_nota()),
        }
    }
}

impl TypeReference {
    /// Construct a reference from a schema name. Reserved scalar names
    /// become scalar leaves; every other name remains a declared-name
    /// leaf.
    pub fn new(name: impl Into<String>) -> Self {
        Self::from_name(Name::new(name))
    }

    pub fn from_name(name: Name) -> Self {
        match name.as_str() {
            "String" => Self::String,
            "Integer" => Self::Integer,
            "Boolean" => Self::Boolean,
            "Path" => Self::Path,
            _ => Self::Plain(name),
        }
    }

    pub fn is_reserved_scalar_name(name: &Name) -> bool {
        matches!(name.as_str(), "String" | "Integer" | "Boolean" | "Path")
    }

    pub fn scalar_name(&self) -> Option<&'static str> {
        match self {
            Self::String => Some("String"),
            Self::Integer => Some("Integer"),
            Self::Boolean => Some("Boolean"),
            Self::Path => Some("Path"),
            Self::Plain(_) | Self::Vector(_) | Self::Map(..) | Self::Optional(_) => None,
        }
    }

    /// The plain name when this reference is a declared-name leaf.
    /// `None` for scalar, collection, or option references — those do
    /// not refer to a user-declared namespace type.
    pub fn plain_name(&self) -> Option<&Name> {
        match self {
            Self::Plain(name) => Some(name),
            Self::String
            | Self::Integer
            | Self::Boolean
            | Self::Path
            | Self::Vector(_)
            | Self::Map(..)
            | Self::Optional(_) => None,
        }
    }

    /// Whether this reference is a declared-name leaf.
    pub fn is_plain(&self) -> bool {
        matches!(self, Self::Plain(_))
    }

    fn from_nota_map_payload(block: &Block) -> Result<Self, NotaDecodeError> {
        let children = NotaBlock::new(block).expect_children(
            Delimiter::Parenthesis,
            "parenthesis",
            "TypeReference::Map payload",
            2,
        )?;
        Ok(Self::Map(
            Box::new(Self::from_nota_block(&children[0])?),
            Box::new(Self::from_nota_block(&children[1])?),
        ))
    }

    /// Lower an already-parsed NOTA block at a reference position into
    /// a `TypeReference`.
    ///
    /// A bare PascalCase symbol (`Topic`, `schema-core:mail:Magnitude`)
    /// lowers to `Plain`. Schema type-reference objects lower at this
    /// position: `(Vec T)` -> `Vector`, `(Map (K V))` -> `Map`, and
    /// `(Optional T)` -> `Optional`. The inner positions recurse, so
    /// `(Vec (Optional Topic))` and `(Map (NodeName (Vec Service)))`
    /// nest. nota-next did the structural parse; this is pure semantic
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
        match block {
            Block::Atom(_) => Ok(Self::from_name(block.schema_name()?)),
            Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                root_objects,
                ..
            } => Err(SchemaError::UnknownTypeReferenceForm {
                head: "SquareBracket".to_owned(),
                argument_count: root_objects.len(),
            }),
            Block::Delimited {
                delimiter: Delimiter::Brace,
                root_objects,
                ..
            } => Err(SchemaError::UnknownTypeReferenceForm {
                head: "Brace".to_owned(),
                argument_count: root_objects.len(),
            }),
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                root_objects,
                ..
            } => Self::from_parenthesis_objects(block, root_objects, registry, context),
            Block::PipeText(_) => Err(SchemaError::ExpectedSymbol {
                found: block.reemit_fallback(),
            }),
            Block::Delimited {
                delimiter: Delimiter::PipeBrace,
                root_objects,
                ..
            } => Self::from_inline_struct(root_objects, registry, context),
            Block::Delimited {
                delimiter: Delimiter::PipeParenthesis,
                root_objects,
                ..
            } => Self::from_inline_enum(root_objects, registry, context),
        }
    }

    fn from_inline_struct(
        objects: &[Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        let name = Self::inline_declaration_name(objects, "inline struct declaration")?;
        let fields = AssembledFields::new(&objects[1..]).lower(registry, context)?;
        if fields.len() == 1 {
            let reference = fields.into_iter().next().expect("length checked").reference;
            context.remember_inline_declaration(Declaration::private(TypeDeclaration::Newtype(
                NewtypeDeclaration::new(name.clone(), reference),
            )));
        } else {
            let declaration = StructDeclaration::new(name.clone(), fields);
            context.remember_inline_declaration(Declaration::private(TypeDeclaration::Struct(
                declaration,
            )));
        }
        Ok(Self::Plain(name))
    }

    fn from_inline_enum(
        objects: &[Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        let name = Self::inline_declaration_name(objects, "inline enum declaration")?;
        let variants = AssembledVariants::new(&objects[1..]).lower(registry, context)?;
        context.remember_inline_declaration(Declaration::private(TypeDeclaration::Enum(
            EnumDeclaration::new(name.clone(), variants),
        )));
        Ok(Self::Plain(name))
    }

    fn inline_declaration_name(objects: &[Block], form: &'static str) -> Result<Name, SchemaError> {
        let Some(name) = objects.first() else {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form,
                expected: "declaration name plus body",
                found: 0,
            });
        };
        name.schema_name()
    }

    fn from_parenthesis_objects(
        block: &Block,
        objects: &[Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        if objects.len() == 2 {
            if let Some(head) = objects[0].demote_to_string() {
                match head {
                    "Vec" | "Vector" => {
                        return Ok(Self::Vector(Box::new(Self::from_block_with_registry(
                            &objects[1],
                            registry,
                            context,
                        )?)));
                    }
                    "Optional" | "Option" => {
                        return Ok(Self::Optional(Box::new(Self::from_block_with_registry(
                            &objects[1],
                            registry,
                            context,
                        )?)));
                    }
                    "Map" | "KeyValue" => {
                        return Self::from_grouped_map_payload(&objects[1], registry, context);
                    }
                    _ => {}
                }
            }
        }
        Self::from_macro_invocation(block, registry, context)
    }

    fn from_grouped_map_payload(
        block: &Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        let Block::Delimited {
            delimiter: Delimiter::Parenthesis,
            root_objects,
            ..
        } = block
        else {
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: "Map".to_owned(),
                argument_count: 1,
            });
        };
        if root_objects.len() != 2 {
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: "Map".to_owned(),
                argument_count: root_objects.len(),
            });
        }
        Ok(Self::Map(
            Box::new(Self::from_block_with_registry(
                &root_objects[0],
                registry,
                context,
            )?),
            Box::new(Self::from_block_with_registry(
                &root_objects[1],
                registry,
                context,
            )?),
        ))
    }

    fn from_macro_invocation(
        block: &Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        let invocation = TypeReferenceMacroInvocation::from_block(block)?;
        if !registry
            .node_definition(MacroPosition::TypeReference)
            .is_some_and(|definition| definition.accepts_tagged_invocation())
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
struct TypeReferenceMacroInvocation<'schema> {
    name: Name,
    data: MacroInvocationData<'schema>,
}

impl<'schema> TypeReferenceMacroInvocation<'schema> {
    fn from_block(block: &'schema Block) -> Result<Self, SchemaError> {
        if !block.is_parenthesis() {
            return Err(SchemaError::ExpectedDelimiter {
                expected: "(Macro [input])",
            });
        }
        if block.holds_root_objects() != 2 {
            let head = block
                .root_object_at(0)
                .and_then(Block::demote_to_string)
                .unwrap_or("<missing>");
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: head.to_owned(),
                argument_count: block.holds_root_objects().saturating_sub(1),
            });
        }
        let name = block
            .root_object_at(0)
            .ok_or(SchemaError::EmptyTypeReference)?
            .schema_name()?;
        let data = MacroInvocationData::from_block(block.root_object_at(1).expect("count checked"));
        Ok(Self { name, data })
    }

    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn argument_count(&self) -> usize {
        self.data.argument_count()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MacroInvocationData<'schema> {
    Delimited(&'schema [Block]),
    Single(&'schema Block),
}

impl<'schema> MacroInvocationData<'schema> {
    fn from_block(block: &'schema Block) -> Self {
        match block {
            Block::Delimited { root_objects, .. } => Self::Delimited(root_objects),
            Block::PipeText(_) | Block::Atom(_) => Self::Single(block),
        }
    }

    fn argument_count(&self) -> usize {
        match self {
            Self::Delimited(objects) => objects.len(),
            Self::Single(_) => 1,
        }
    }
}

/// Data representation of a schema-node object before macro execution.
///
/// A parenthesized schema node is a tagged/data-carrying variant:
/// `(Normalize [Topic])` has tag `Normalize` and raw vector data
/// `[Topic]`. That vector is macro payload data, not the schema `Vec`
/// type constructor. This type exists so macro calls can be inspected,
/// serialized through assembled schema, and tested as data rather than
/// disappearing into parser control flow.
#[derive(nota_next::NotaDecode, nota_next::NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct SchemaNode {
    tag: Name,
    data: SchemaNodeData,
}

impl SchemaNode {
    pub fn new(tag: Name, data: SchemaNodeData) -> Self {
        Self { tag, data }
    }

    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let children = match block {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                root_objects,
                ..
            } => root_objects,
            _ => {
                return Err(SchemaError::MalformedSchemaNode {
                    found: SchemaNodeNotation::new(block).compact(),
                });
            }
        };
        let tag = children
            .first()
            .ok_or_else(|| SchemaError::MalformedSchemaNode {
                found: SchemaNodeNotation::new(block).compact(),
            })?
            .schema_name()?;
        let data = match children.len() {
            1 => SchemaNodeData::Unit,
            2 => SchemaNodeData::from_block(&children[1])?,
            _ => {
                return Err(SchemaError::MalformedSchemaNode {
                    found: SchemaNodeNotation::new(block).compact(),
                });
            }
        };
        Ok(Self { tag, data })
    }

    pub fn tag(&self) -> &Name {
        &self.tag
    }

    pub fn data(&self) -> &SchemaNodeData {
        &self.data
    }
}

#[derive(nota_next::NotaDecode, nota_next::NotaEncode, Clone, Debug, Eq, PartialEq)]
pub enum SchemaNodeData {
    Unit,
    Value(SchemaNodeValue),
    Vector(Vec<SchemaNodeValue>),
    Map(Vec<SchemaNodePair>),
}

impl SchemaNodeData {
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        match block {
            Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                root_objects,
                ..
            } => Ok(Self::Vector(SchemaNodeValues::new(root_objects).read()?)),
            Block::Delimited {
                delimiter: Delimiter::Brace,
                root_objects,
                ..
            } => Ok(Self::Map(SchemaNodeMapEntries::new(root_objects).read()?)),
            _ => Ok(Self::Value(SchemaNodeValue::from_block(block)?)),
        }
    }
}

#[derive(nota_next::NotaDecode, nota_next::NotaEncode, Clone, Debug, Eq, PartialEq)]
pub enum SchemaNodeValue {
    Symbol(Name),
    Text(String),
    Node(Box<SchemaNode>),
    Vector(Vec<SchemaNodeValue>),
    Map(Vec<SchemaNodePair>),
}

impl SchemaNodeValue {
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        match block {
            Block::Atom(_) => block.schema_name().map(Self::Symbol),
            Block::PipeText(text) => Ok(Self::Text(text.text.clone())),
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                ..
            } => Ok(Self::Node(Box::new(SchemaNode::from_block(block)?))),
            Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                root_objects,
                ..
            } => Ok(Self::Vector(SchemaNodeValues::new(root_objects).read()?)),
            Block::Delimited {
                delimiter: Delimiter::Brace,
                root_objects,
                ..
            } => Ok(Self::Map(SchemaNodeMapEntries::new(root_objects).read()?)),
            Block::Delimited {
                delimiter: Delimiter::PipeParenthesis,
                ..
            }
            | Block::Delimited {
                delimiter: Delimiter::PipeBrace,
                ..
            } => Err(SchemaError::MalformedSchemaNode {
                found: SchemaNodeNotation::new(block).compact(),
            }),
        }
    }
}

#[derive(nota_next::NotaDecode, nota_next::NotaEncode, Clone, Debug, Eq, PartialEq)]
pub struct SchemaNodePair {
    key: Name,
    value: SchemaNodeValue,
}

impl SchemaNodePair {
    pub fn new(key: Name, value: SchemaNodeValue) -> Self {
        Self { key, value }
    }

    pub fn key(&self) -> &Name {
        &self.key
    }

    pub fn value(&self) -> &SchemaNodeValue {
        &self.value
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaNodeValues<'schema> {
    objects: &'schema [Block],
}

impl<'schema> SchemaNodeValues<'schema> {
    fn new(objects: &'schema [Block]) -> Self {
        Self { objects }
    }

    fn read(&self) -> Result<Vec<SchemaNodeValue>, SchemaError> {
        let mut values = Vec::new();
        for object in self.objects {
            values.push(SchemaNodeValue::from_block(object)?);
        }
        Ok(values)
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaNodeMapEntries<'schema> {
    objects: &'schema [Block],
}

impl<'schema> SchemaNodeMapEntries<'schema> {
    fn new(objects: &'schema [Block]) -> Self {
        Self { objects }
    }

    fn read(&self) -> Result<Vec<SchemaNodePair>, SchemaError> {
        if self.objects.len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: self.objects.len(),
            });
        }
        let mut pairs = Vec::new();
        for chunk in self.objects.chunks_exact(2) {
            pairs.push(SchemaNodePair::new(
                chunk[0].schema_name()?,
                SchemaNodeValue::from_block(&chunk[1])?,
            ));
        }
        Ok(pairs)
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaNodeNotation<'schema> {
    block: &'schema Block,
}

impl<'schema> SchemaNodeNotation<'schema> {
    fn new(block: &'schema Block) -> Self {
        Self { block }
    }

    fn compact(&self) -> String {
        match self.block {
            Block::Delimited {
                delimiter,
                root_objects,
                ..
            } => {
                let children = root_objects
                    .iter()
                    .map(|child| Self::new(child).compact())
                    .collect::<Vec<_>>();
                SchemaNodeDelimitedNotation::new(*delimiter).wrap(&children)
            }
            Block::PipeText(text) => format!("[|{}|]", text.text),
            Block::Atom(atom) => atom.text().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SchemaNodeDelimitedNotation {
    delimiter: Delimiter,
}

impl SchemaNodeDelimitedNotation {
    fn new(delimiter: Delimiter) -> Self {
        Self { delimiter }
    }

    fn wrap(&self, children: &[String]) -> String {
        if children.is_empty() {
            return format!("{}{}", self.opening(), self.closing());
        }
        format!("{}{}{}", self.opening(), children.join(" "), self.closing())
    }

    fn opening(&self) -> &'static str {
        match self.delimiter {
            Delimiter::Parenthesis => "(",
            Delimiter::SquareBracket => "[",
            Delimiter::Brace => "{",
            Delimiter::PipeParenthesis => "(|",
            Delimiter::PipeBrace => "{|",
        }
    }

    fn closing(&self) -> &'static str {
        match self.delimiter {
            Delimiter::Parenthesis => ")",
            Delimiter::SquareBracket => "]",
            Delimiter::Brace => "}",
            Delimiter::PipeParenthesis => "|)",
            Delimiter::PipeBrace => "|}",
        }
    }
}
