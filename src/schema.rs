use std::fmt;

use nota_next::{
    AtomClassification, Block, Delimiter, NotaBlock, NotaBody, NotaDecode, NotaDecodeError,
    NotaEncode, NotaString, StructuralMacroError, StructuralMacroNode,
};

use crate::{
    MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry, SchemaError,
    declarative::{MacroExpansionFields, MacroExpansionVariants},
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

/// A `Name` decodes from a bare symbol atom and re-emits through its NOTA
/// codec, so the `TypeReference` derive can recurse into a `Plain` leaf as a
/// `#[shape(pascal_atom)]` field. The derive's pascal-case gate runs first, so
/// only a PascalCase atom reaches this decode in the reference grammar; the
/// symbol-case acceptance keeps the node usable wherever a qualified name is
/// already known to sit at the position.
impl nota_next::StructuralMacroNode for Name {
    type Error = SchemaError;

    fn structural_position() -> nota_next::PositionPredicate {
        nota_next::PositionPredicate::named("type name")
    }

    fn structural_variants() -> Vec<nota_next::StructuralVariant> {
        vec![
            nota_next::BlockShape::symbol(Some(nota_next::CaptureName::new("name")))
                .into_structural_variant("Name", "symbol atom"),
        ]
    }

    fn from_structural_block(
        block: &Block,
    ) -> Result<Self, nota_next::StructuralMacroError<Self::Error>> {
        block
            .schema_name()
            .map_err(nota_next::StructuralMacroError::MatchedNode)
    }

    fn from_structural_candidate(
        candidate: nota_next::MacroCandidate<'_>,
    ) -> Result<Self, nota_next::StructuralMacroError<Self::Error>> {
        match candidate.blocks() {
            [block] => Self::from_structural_block(block),
            blocks => Err(nota_next::StructuralMacroError::ExpectedSingleRoot {
                found: blocks.len(),
            }),
        }
    }

    fn to_structural_nota(&self) -> String {
        self.to_nota()
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, Hash, PartialEq)]
pub struct SymbolPath(Vec<Name>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolPathPosition<'path> {
    Type {
        type_name: &'path Name,
    },
    RootVariant {
        root_name: &'path Name,
        variant_name: &'path Name,
    },
    Field {
        type_name: &'path Name,
        field_name: &'path Name,
    },
    EnumVariant {
        enum_name: &'path Name,
        variant_name: &'path Name,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchemaDeclaredType<'schema> {
    Root(&'schema EnumDeclaration),
    Namespace(&'schema TypeDeclaration),
}

impl SymbolPath {
    pub fn new(segments: impl IntoIterator<Item = Name>) -> Self {
        Self(segments.into_iter().collect())
    }

    pub fn from_identity_and_segments(
        identity: &super::SchemaIdentity,
        segments: impl IntoIterator<Item = Name>,
    ) -> Self {
        let mut path_segments = vec![identity.component().clone()];
        path_segments.extend(segments);
        Self::new(path_segments)
    }

    pub fn segments(&self) -> &[Name] {
        &self.0
    }

    pub fn component(&self) -> Option<&Name> {
        self.0.first()
    }

    pub fn local_segments(&self) -> &[Name] {
        self.0.get(1..).unwrap_or(&[])
    }

    pub fn belongs_to(&self, identity: &super::SchemaIdentity) -> bool {
        self.component()
            .is_some_and(|component| component == identity.component())
    }

    pub fn type_path(identity: &super::SchemaIdentity, type_name: &Name) -> Self {
        Self::from_identity_and_segments(identity, [type_name.clone()])
    }

    pub fn root_variant_path(
        identity: &super::SchemaIdentity,
        root_name: &Name,
        variant_name: &Name,
    ) -> Self {
        Self::from_identity_and_segments(identity, [root_name.clone(), variant_name.clone()])
    }

    pub fn field_path(
        identity: &super::SchemaIdentity,
        type_name: &Name,
        field_name: &Name,
    ) -> Self {
        Self::from_identity_and_segments(identity, [type_name.clone(), field_name.clone()])
    }

    pub fn enum_variant_path(
        identity: &super::SchemaIdentity,
        enum_name: &Name,
        variant_name: &Name,
    ) -> Self {
        Self::from_identity_and_segments(identity, [enum_name.clone(), variant_name.clone()])
    }
}

impl NotaDecode for SymbolPath {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let children =
            NotaBlock::new(block).expect_children(Delimiter::Parenthesis, "SymbolPath", 2)?;
        let variant = children[0]
            .demote_to_string()
            .ok_or(NotaDecodeError::ExpectedAtom {
                type_name: "SymbolPath variant",
            })?;
        if variant != "SymbolPath" {
            return Err(NotaDecodeError::UnknownVariant {
                enum_name: "SymbolPath",
                variant: variant.to_owned(),
            });
        }
        Ok(Self(Vec::<Name>::from_nota_block(&children[1])?))
    }
}

impl NotaEncode for SymbolPath {
    fn to_nota(&self) -> String {
        format!("(SymbolPath {})", self.0.to_nota())
    }
}

impl fmt::Display for SymbolPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let joined = self
            .segments()
            .iter()
            .map(Name::as_str)
            .collect::<Vec<_>>()
            .join("/");
        formatter.write_str(&joined)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    identity: super::SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    resolved_imports: Vec<super::ResolvedImport>,
    input: EnumDeclaration,
    output: EnumDeclaration,
    namespace: Vec<Declaration>,
    streams: Vec<StreamDeclaration>,
    families: Vec<FamilyDeclaration>,
    relations: Vec<RelationDeclaration>,
}

impl Schema {
    pub(crate) fn new(
        identity: super::SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        resolved_imports: Vec<super::ResolvedImport>,
        input: EnumDeclaration,
        output: EnumDeclaration,
        namespace: Vec<Declaration>,
        streams: Vec<StreamDeclaration>,
        families: Vec<FamilyDeclaration>,
        relations: Vec<RelationDeclaration>,
    ) -> Self {
        Self {
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
            streams,
            families,
            relations,
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

    pub fn streams(&self) -> &[StreamDeclaration] {
        &self.streams
    }

    pub fn families(&self) -> &[FamilyDeclaration] {
        &self.families
    }

    pub fn relations(&self) -> &[RelationDeclaration] {
        &self.relations
    }

    /// Confirm every declared family's record type resolves to a
    /// namespace declaration, a root enum, or a declared import. Both
    /// lowering paths call this after assembly, so an unresolvable
    /// record name is a typed error rather than a silent dead family.
    pub(crate) fn families_verified(self) -> Result<Self, SchemaError> {
        for family in &self.families {
            if !self.family_record_resolves(&family.record) {
                return Err(SchemaError::FamilyRecordNotFound {
                    family: family.name.as_str().to_owned(),
                    record: family.record.as_str().to_owned(),
                });
            }
        }
        Ok(self)
    }

    fn family_record_resolves(&self, record: &Name) -> bool {
        self.type_named(record.as_str()).is_some()
            || self.root_named(record.as_str()).is_some()
            || self
                .imports
                .iter()
                .any(|import| &import.local_name == record)
    }

    pub fn type_named(&self, name: &str) -> Option<&TypeDeclaration> {
        self.namespace
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
            .map(Declaration::value)
    }

    pub fn declared_type_named(&self, name: &str) -> Option<SchemaDeclaredType<'_>> {
        self.type_named(name)
            .map(SchemaDeclaredType::Namespace)
            .or_else(|| self.root_named(name).map(SchemaDeclaredType::Root))
    }

    pub fn type_path(&self, type_name: &str) -> Option<SymbolPath> {
        self.type_named(type_name)
            .map(TypeDeclaration::name)
            .map(|name| SymbolPath::type_path(&self.identity, name))
    }

    pub fn root_variant_path(&self, root_name: &str, variant_name: &str) -> Option<SymbolPath> {
        self.root_named(root_name).and_then(|root| {
            root.variants
                .iter()
                .find(|variant| variant.name.as_str() == variant_name)
                .map(|variant| {
                    SymbolPath::root_variant_path(&self.identity, &root.name, &variant.name)
                })
        })
    }

    pub fn field_path(&self, type_name: &str, field_name: &str) -> Option<SymbolPath> {
        let TypeDeclaration::Struct(declaration) = self.type_named(type_name)? else {
            return None;
        };
        declaration
            .fields
            .iter()
            .find(|field| field.name.as_str() == field_name)
            .map(|field| SymbolPath::field_path(&self.identity, &declaration.name, &field.name))
    }

    pub fn enum_variant_path(&self, enum_name: &str, variant_name: &str) -> Option<SymbolPath> {
        let TypeDeclaration::Enum(declaration) = self.type_named(enum_name)? else {
            return None;
        };
        declaration
            .variants
            .iter()
            .find(|variant| variant.name.as_str() == variant_name)
            .map(|variant| {
                SymbolPath::enum_variant_path(&self.identity, &declaration.name, &variant.name)
            })
    }

    pub fn symbol_path_position<'path>(
        &self,
        path: &'path SymbolPath,
    ) -> Option<SymbolPathPosition<'path>> {
        if !path.belongs_to(&self.identity) {
            return None;
        }
        match path.local_segments() {
            [type_name] if self.type_named(type_name.as_str()).is_some() => {
                Some(SymbolPathPosition::Type { type_name })
            }
            [root_name, variant_name]
                if self
                    .root_named(root_name.as_str())
                    .is_some_and(|root| root.has_variant(variant_name)) =>
            {
                Some(SymbolPathPosition::RootVariant {
                    root_name,
                    variant_name,
                })
            }
            [type_name, field_name]
                if self
                    .type_named(type_name.as_str())
                    .is_some_and(|declaration| declaration.has_field_named(field_name)) =>
            {
                Some(SymbolPathPosition::Field {
                    type_name,
                    field_name,
                })
            }
            [enum_name, variant_name]
                if self
                    .type_named(enum_name.as_str())
                    .is_some_and(|declaration| declaration.has_variant_named(variant_name)) =>
            {
                Some(SymbolPathPosition::EnumVariant {
                    enum_name,
                    variant_name,
                })
            }
            _ => None,
        }
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
pub enum RelationDeclaration {
    Equivalence(Vec<RelationValue>),
}

impl RelationDeclaration {
    pub fn values(&self) -> &[RelationValue] {
        match self {
            Self::Equivalence(values) => values,
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
pub struct RelationValue {
    path: Vec<Name>,
}

impl RelationValue {
    pub fn new(path: Vec<Name>) -> Self {
        Self { path }
    }

    pub fn path(&self) -> &[Name] {
        &self.path
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

    pub fn has_field_named(&self, field_name: &Name) -> bool {
        let Self::Struct(declaration) = self else {
            return false;
        };
        declaration
            .fields
            .iter()
            .any(|field| &field.name == field_name)
    }

    pub fn has_variant_named(&self, variant_name: &Name) -> bool {
        let Self::Enum(declaration) = self else {
            return false;
        };
        declaration.has_variant(variant_name)
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

/// Ordered key/value representation of a struct definition in schema.
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
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "StructFieldMap")?;
        let root_objects = body.root_objects();
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

    pub fn has_variant(&self, variant_name: &Name) -> bool {
        self.variants
            .iter()
            .any(|variant| &variant.name == variant_name)
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
    pub stream_relation: Option<StreamRelation>,
}

impl EnumVariant {
    pub fn new(name: Name, payload: Option<TypeReference>) -> Self {
        Self {
            name,
            payload,
            stream_relation: None,
        }
    }

    pub fn with_stream_relation(mut self, stream_relation: StreamRelation) -> Self {
        self.stream_relation = Some(stream_relation);
        self
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
pub enum StreamRelation {
    Opens(Name),
    Belongs(Name),
}

impl StreamRelation {
    pub fn stream_name(&self) -> &Name {
        match self {
            Self::Opens(name) | Self::Belongs(name) => name,
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
pub struct StreamDeclaration {
    pub name: Name,
    pub token: TypeReference,
    pub opened: TypeReference,
    pub event: TypeReference,
    pub close: TypeReference,
}

impl StreamDeclaration {
    pub fn new(
        name: Name,
        token: TypeReference,
        opened: TypeReference,
        event: TypeReference,
        close: TypeReference,
    ) -> Self {
        Self {
            name,
            token,
            opened,
            event,
            close,
        }
    }
}

/// The current storage coordinate of a record family. A table name is
/// not a schema symbol: renaming the table moves only this coordinate,
/// never the family's semantic identity.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct TableName(String);

impl TableName {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl NotaDecode for TableName {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        NotaBlock::new(block).parse_string().map(Self::new)
    }
}

impl NotaEncode for TableName {
    fn to_nota(&self) -> String {
        if AtomClassification::classify(self.as_str()) == AtomClassification::SymbolCandidate {
            self.as_str().to_owned()
        } else {
            NotaString::new(self.as_str()).format()
        }
    }
}

impl fmt::Display for TableName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// How a stored record family is keyed: by a domain-supplied record
/// key, or by an engine-assigned record identifier. The two variants
/// mirror the two registration shapes a SEMA engine offers (a keyed
/// table descriptor versus an identified table descriptor).
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::NotaDecode,
    nota_next::NotaEncode,
    nota_next::StructuralMacroNode,
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
)]
pub enum FamilyKey {
    #[shape(keyword = "Domain")]
    Domain,
    #[shape(keyword = "Identified")]
    Identified,
}

/// A stored record family: schema metadata in the namespace map, on
/// the stream-declaration precedent. The family name is the stable
/// identity; the record type names the declaration whose closure is
/// the family's content identity; the table name is only the current
/// storage coordinate; the key kind selects the engine registration
/// shape.
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
pub struct FamilyDeclaration {
    pub name: Name,
    pub record: Name,
    pub table: TableName,
    pub key: FamilyKey,
}

impl FamilyDeclaration {
    pub fn new(name: Name, record: Name, table: TableName, key: FamilyKey) -> Self {
        Self {
            name,
            record,
            table,
            key,
        }
    }
}

/// A type at a reference position — a struct field's type, an enum
/// variant's payload, or an import source.
///
/// `String`, `Integer`, `Boolean`, and `Path` are reserved scalar leaves.
/// `Plain` is a declared-name leaf (`Topic`, `Magnitude`). `Vector`,
/// `Map`, `Optional`, and `ScopeOf` carry inner references.
///
/// The NOTA reference grammar is the [`StructuralMacroNode`] derive below: the
/// `#[shape(...)]` attribute on each variant is the single source of truth for
/// that variant's canonical head, and the derive *generates* the decode
/// dispatch and the encoder. There is exactly one head per variant —
/// `(Vec T)`, `(Optional T)`, `(Scope T)`, `(Map K V)`, `(Bytes N)`, the bare
/// `Bytes` leaf, the bare scalar leaves, and a bare PascalCase atom for
/// `Plain`. The earlier alias spellings (`Vector`, `Option`, `ScopeOf`,
/// `KeyValue`) are gone, per the no-aliases decision: each loses its safe bare
/// form and is no longer recognised. Round-trip is the witness — a
/// `TypeReference` NOTA form decodes to the node and re-encodes identically
/// through the derive.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::StructuralMacroNode,
    Clone,
    Debug,
    Eq,
    PartialEq,
)]
#[rkyv(
    bytecheck(bounds(__C: rkyv::validation::ArchiveContext)),
    serialize_bounds(__S: rkyv::ser::Writer),
    deserialize_bounds(__D::Error: rkyv::rancor::Source)
)]
pub enum TypeReference {
    #[shape(keyword = "String")]
    String,
    #[shape(keyword = "Integer")]
    Integer,
    #[shape(keyword = "Boolean")]
    Boolean,
    #[shape(keyword = "Path")]
    Path,
    #[shape(keyword = "Bytes")]
    Bytes,
    #[shape(head = "Bytes", atom)]
    FixedBytes(u64),
    #[shape(head = "Vec", arity = 2)]
    Vector(#[rkyv(omit_bounds)] Box<TypeReference>),
    #[shape(head = "Map", arity = 3)]
    Map(
        #[rkyv(omit_bounds)] Box<TypeReference>,
        #[rkyv(omit_bounds)] Box<TypeReference>,
    ),
    #[shape(head = "Optional", arity = 2)]
    Optional(#[rkyv(omit_bounds)] Box<TypeReference>),
    #[shape(head = "Scope", arity = 2)]
    ScopeOf(#[rkyv(omit_bounds)] Box<TypeReference>),
    #[shape(pascal_atom)]
    Plain(Name),
}

impl NotaDecode for TypeReference {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        Self::from_structural_block(block).map_err(|error| NotaDecodeError::UnknownVariant {
            enum_name: "TypeReference",
            variant: error.to_string(),
        })
    }
}

impl NotaEncode for TypeReference {
    fn to_nota(&self) -> String {
        self.to_structural_nota()
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
            "Bytes" => Self::Bytes,
            _ => Self::Plain(name),
        }
    }

    pub fn is_reserved_scalar_name(name: &Name) -> bool {
        matches!(
            name.as_str(),
            "String" | "Integer" | "Boolean" | "Path" | "Bytes"
        )
    }

    pub fn scalar_name(&self) -> Option<&'static str> {
        match self {
            Self::String => Some("String"),
            Self::Integer => Some("Integer"),
            Self::Boolean => Some("Boolean"),
            Self::Path => Some("Path"),
            Self::Bytes => Some("Bytes"),
            Self::FixedBytes(_)
            | Self::Plain(_)
            | Self::Vector(_)
            | Self::Map(..)
            | Self::Optional(_)
            | Self::ScopeOf(_) => None,
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
            | Self::Bytes
            | Self::FixedBytes(_)
            | Self::Vector(_)
            | Self::Map(..)
            | Self::Optional(_)
            | Self::ScopeOf(_) => None,
        }
    }

    /// Whether this reference is a declared-name leaf.
    pub fn is_plain(&self) -> bool {
        matches!(self, Self::Plain(_))
    }

    /// Lower an already-parsed NOTA block at a reference position into
    /// a `TypeReference`.
    ///
    /// A bare PascalCase symbol (`Topic`) lowers to `Plain`. Schema
    /// type-reference objects lower at this position through the
    /// `StructuralMacroNode` derive: `(Vec T)` -> `Vector`, `(Map K V)` ->
    /// `Map`, `(Optional T)` -> `Optional`, and `(Scope T)` -> `ScopeOf`.
    /// The inner positions recurse, so `(Vec (Optional Topic))` and
    /// `(Map NodeName (Vec Service))` nest. A head the derive does not
    /// recognise falls through to the declared-name macro-invocation path.
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
        let fields = MacroExpansionFields::new(&objects[1..]).lower(registry, context)?;
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
        let variants = MacroExpansionVariants::new(&objects[1..]).lower(registry, context)?;
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

    /// Lower a parenthesised reference. The canonical reference grammar —
    /// `(Vec T)`, `(Optional T)`, `(Scope T)`, `(Map K V)`, `(Bytes N)` — is
    /// the [`StructuralMacroNode`] derive, so the derived decode dispatches the
    /// head and recurses through nested grammar forms. A head the derive does
    /// not recognise is not a grammar form: it falls through to the
    /// declared-name macro-invocation path against the registry.
    fn from_parenthesis_objects(
        block: &Block,
        _objects: &[Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        match Self::from_structural_block(block) {
            Ok(reference) => Ok(reference),
            Err(StructuralMacroError::Dispatch(_)) => {
                Self::from_macro_invocation(block, registry, context)
            }
            Err(error) => Err(SchemaError::from(error)),
        }
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
