use std::fmt;

use nota_next::{
    AtomClassification, Block, Delimiter, NotaBlock, NotaBody, NotaDecode, NotaDecodeError,
    NotaEncode, NotaString, StructuralMacroNode,
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

    /// Whether this name is a PascalCase symbol — a symbol-shaped atom whose
    /// local part begins with an ASCII uppercase letter. This is the head
    /// gate for the generic-application form: only a PascalCase head can name
    /// a parameterized type at a reference position.
    pub fn qualifies_as_pascal_case(&self) -> bool {
        self.qualifies_as_symbol_name()
            && self
                .local_part()
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_uppercase())
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

/// A `Name` decodes from a bare symbol atom and re-emits through its NOTA
/// codec, so a structural-macro node can carry it as a head or leaf capture.
/// In the reference grammar the application form's `pascal_head` gate runs
/// first, so only a PascalCase atom reaches this decode there; the
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

impl fmt::Display for Name {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
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

/// A component-root Input/Output position. Today the position forces an
/// enum body `[Variant …]`, but a root may also be a typed sum applied
/// at the position directly — `(Work SignalInput SemaWriteOutput …)` — an
/// application of an imported or locally-declared parameterized head. The
/// closed sum names the two shapes a root can take; nothing else is a
/// legal root.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum Root {
    /// The enum-body root `[Variant …]` — the position lowers to a public
    /// enum declaration whose variants are the root's signatures.
    Enum(EnumDeclaration),
    /// The application-form root `(Head Arg …)` — the position is a typed
    /// sum produced by applying a parameterized head to its arguments. The
    /// application is boxed: an imported head carries a `ResolvedImport`, so
    /// an unboxed `RootApplication` would make `Root` (and every `Schema`
    /// holding two roots) carry that weight even for the common enum root.
    Application(Box<RootApplication>),
}

impl Root {
    /// Build an application root from its parts, boxing the application.
    pub fn application(application: RootApplication) -> Self {
        Self::Application(Box::new(application))
    }

    /// The root's identity name: an enum root carries its declaration name,
    /// an application root carries its position name (`Input` / `Output`).
    pub fn name(&self) -> &Name {
        match self {
            Self::Enum(declaration) => &declaration.name,
            Self::Application(application) => application.name(),
        }
    }

    /// The enum declaration when this root is the enum-body form; `None`
    /// for an application root. Callers that genuinely need the variant
    /// list (symbol-path resolution, variant lookup) read through this.
    pub fn as_enum(&self) -> Option<&EnumDeclaration> {
        match self {
            Self::Enum(declaration) => Some(declaration),
            Self::Application(_) => None,
        }
    }

    /// The application when this root is the application form; `None` for
    /// an enum root.
    pub fn as_application(&self) -> Option<&RootApplication> {
        match self {
            Self::Application(application) => Some(application.as_ref()),
            Self::Enum(_) => None,
        }
    }
}

/// A root in the application form `(Head Arg …)`: a parameterized head
/// applied to a tail of type-reference arguments, standing at a root
/// Input/Output position. It mirrors [`TypeReference::Application`]'s shape
/// but carries the position name the root is identified by, since an
/// application has no declaration name of its own. The content-address
/// closure reuses the field-position `Application` walk by projecting this
/// root back into a [`TypeReference::Application`] (see [`TypeReference`]'s
/// `From<&RootApplication>`).
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct RootApplication {
    name: Name,
    head: ApplicationHead,
    arguments: Vec<TypeReference>,
}

impl RootApplication {
    pub fn new(name: Name, head: ApplicationHead, arguments: Vec<TypeReference>) -> Self {
        Self {
            name,
            head,
            arguments,
        }
    }

    /// The position name this application root is identified by
    /// (`Input` / `Output`).
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn head(&self) -> &ApplicationHead {
        &self.head
    }

    pub fn arguments(&self) -> &[TypeReference] {
        &self.arguments
    }
}

impl From<&RootApplication> for TypeReference {
    /// Project the application root back into a field-position application
    /// reference, so the existing `Application` closure walk and arity
    /// validation cover it without a second code path.
    fn from(application: &RootApplication) -> Self {
        Self::Application {
            head: application.head.clone(),
            arguments: application.arguments.clone(),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Schema {
    identity: super::SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    resolved_imports: Vec<super::ResolvedImport>,
    input: Root,
    output: Root,
    namespace: Vec<Declaration>,
    streams: Vec<StreamDeclaration>,
    families: Vec<FamilyDeclaration>,
    relations: Vec<RelationDeclaration>,
}

impl Schema {
    // The schema's fields are each a distinct typed section of the model;
    // the constructor takes them as separate typed vectors rather than a
    // bag struct. (Newer clippy raises `too_many_arguments`; the repo's
    // pinned 1.85 toolchain does not.)
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        identity: super::SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        resolved_imports: Vec<super::ResolvedImport>,
        input: Root,
        output: Root,
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

    pub fn input(&self) -> &Root {
        &self.input
    }

    pub fn output(&self) -> &Root {
        &self.output
    }

    pub fn input_and_output(&self) -> [&Root; 2] {
        [self.input(), self.output()]
    }

    /// The root carrying the given position name. Either root shape
    /// answers — an enum root by its declaration name, an application root
    /// by its position name — so callers that only need the enum body
    /// chain `.and_then(Root::as_enum)`.
    pub fn root_named(&self, name: &str) -> Option<&Root> {
        self.input_and_output()
            .into_iter()
            .find(|root| root.name().as_str() == name)
    }

    /// The enum body of the root carrying the given position name; `None`
    /// when no such root exists or the root is an application form. Variant
    /// lookups (symbol paths, family records resolving to a root enum) go
    /// through this.
    pub fn root_enum_named(&self, name: &str) -> Option<&EnumDeclaration> {
        self.root_named(name).and_then(Root::as_enum)
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
            || self.root_enum_named(record.as_str()).is_some()
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
            .or_else(|| self.root_enum_named(name).map(SchemaDeclaredType::Root))
    }

    /// The namespace declaration carrying the given name, with its
    /// declared type parameters attached. Roots are not parameterizable,
    /// so this is the namespace declaration only.
    fn namespace_declaration_named(&self, name: &str) -> Option<&Declaration> {
        self.namespace
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }

    /// The declared generic arity of a named namespace type: the number
    /// of type parameters its declaration head introduced. `None` for a
    /// name that is not a namespace declaration (a root enum, an import,
    /// or an unknown name). A non-parameterized declaration reports
    /// `Some(0)`. The import resolver reads this across the crate
    /// boundary so a consumer can validate an imported head's arity.
    pub fn declared_parameter_count(&self, name: &str) -> Option<usize> {
        self.namespace_declaration_named(name)
            .map(|declaration| declaration.parameters().len())
    }

    /// The generic arity an `Application` head must supply when the head
    /// resolves to a declared parameterized type. A locally-declared head
    /// reports its declaration's parameter count; a resolved import head
    /// reports the parameter count carried across the crate boundary.
    /// `None` means the head is not a declared parameterized type in this
    /// schema, so no arity is fixed here.
    fn declared_head_arity(&self, head: &ApplicationHead) -> Option<usize> {
        match head {
            ApplicationHead::Local(name) => self
                .namespace_declaration_named(name.as_str())
                .map(|declaration| declaration.parameters().len()),
            ApplicationHead::Imported(import) => import.parameter_count(),
        }
    }

    /// Confirm every generic `Application` whose head resolves to a
    /// declared parameterized type supplies exactly that head's declared
    /// arity. This runs at lowering (decision O8), so a wrong argument
    /// count is a typed `GenericArityMismatch` rather than a deferred
    /// emitter failure. Heads that do not resolve to a declared
    /// parameterized type are left for the closure walk to judge.
    pub(crate) fn arities_verified(self) -> Result<Self, SchemaError> {
        for declaration in &self.namespace {
            self.verify_declaration_arities(declaration.value())?;
        }
        for root in self.input_and_output() {
            self.verify_root_arities(root)?;
        }
        Ok(self)
    }

    /// Arity-verify a root in either shape: an enum root verifies each
    /// variant payload; an application root verifies the application
    /// reference it projects to, so a wrong argument count against a
    /// declared parameterized head is the same typed error at the root
    /// position as at a field position.
    fn verify_root_arities(&self, root: &Root) -> Result<(), SchemaError> {
        match root {
            Root::Enum(declaration) => self.verify_enum_arities(declaration),
            Root::Application(application) => {
                self.verify_reference_arities(&TypeReference::from(application.as_ref()))
            }
        }
    }

    fn verify_declaration_arities(&self, declaration: &TypeDeclaration) -> Result<(), SchemaError> {
        match declaration {
            TypeDeclaration::Struct(body) => {
                for field in body.fields.iter() {
                    self.verify_reference_arities(&field.reference)?;
                }
                Ok(())
            }
            TypeDeclaration::Newtype(body) => self.verify_reference_arities(&body.reference),
            TypeDeclaration::Enum(body) => self.verify_enum_arities(body),
        }
    }

    fn verify_enum_arities(&self, declaration: &EnumDeclaration) -> Result<(), SchemaError> {
        for variant in &declaration.variants {
            if let Some(payload) = &variant.payload {
                self.verify_reference_arities(payload)?;
            }
        }
        Ok(())
    }

    fn verify_reference_arities(&self, reference: &TypeReference) -> Result<(), SchemaError> {
        match reference {
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::FixedBytes(_)
            | TypeReference::Plain(_) => Ok(()),
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => self.verify_reference_arities(inner),
            TypeReference::Map(key, value) => {
                self.verify_reference_arities(key)?;
                self.verify_reference_arities(value)
            }
            TypeReference::Application { head, arguments } => {
                if let Some(expected) = self.declared_head_arity(head)
                    && expected != arguments.len()
                {
                    return Err(SchemaError::GenericArityMismatch {
                        head: head.name().as_str().to_owned(),
                        expected,
                        found: arguments.len(),
                    });
                }
                for argument in arguments {
                    self.verify_reference_arities(argument)?;
                }
                Ok(())
            }
        }
    }

    pub fn type_path(&self, type_name: &str) -> Option<SymbolPath> {
        self.type_named(type_name)
            .map(TypeDeclaration::name)
            .map(|name| SymbolPath::type_path(&self.identity, name))
    }

    pub fn root_variant_path(&self, root_name: &str, variant_name: &str) -> Option<SymbolPath> {
        self.root_enum_named(root_name).and_then(|root| {
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
                    .root_enum_named(root_name.as_str())
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
    parameters: Vec<Name>,
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
            parameters: Vec::new(),
            value,
        }
    }

    /// Attach declared type parameters to this declaration. The
    /// parameter names are the binders the parameterized declaration
    /// head `(Name Param …)` introduces; references to them inside the
    /// body resolve as type-parameter binders, and their count is the
    /// declaration's generic arity that an `Application` must match.
    pub fn with_parameters(mut self, parameters: Vec<Name>) -> Self {
        self.parameters = parameters;
        self
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The declared type parameters of this declaration, in order. Empty
    /// for an ordinary (non-parameterized) declaration. The length is the
    /// declaration's generic arity.
    pub fn parameters(&self) -> &[Name] {
        &self.parameters
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

/// The head of a generic application `(Foo A B …)`.
///
/// A head is a typed sum: a generic head may name a locally-declared
/// parameterized type (`Local`) or a cross-crate imported one (`Imported`).
/// NOTA decode never resolves imports, so a freshly-decoded application
/// always carries `Local(Name)`; import resolution rewrites the head to
/// `Imported` once the closure walk proves the name is an import. The
/// canonical NOTA projection of either is the bare head name.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    bytecheck(bounds(
        __C: rkyv::validation::ArchiveContext,
        __C::Error: rkyv::rancor::Source
    )),
    serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source
    ),
    deserialize_bounds(__D::Error: rkyv::rancor::Source)
)]
pub enum ApplicationHead {
    Local(Name),
    Imported(super::ResolvedImport),
}

impl ApplicationHead {
    /// The head's local name — the name written at the application site,
    /// regardless of whether it has been resolved to an import yet.
    pub fn name(&self) -> &Name {
        match self {
            Self::Local(name) => name,
            Self::Imported(import) => import.local_name(),
        }
    }
}

/// The broad generic-application node `(Foo A B …)`, captured directly by
/// nota-next's `#[shape(pascal_head, body)]` derive: a PascalCase head atom
/// followed by a variable-arity tail of type-reference arguments. This is the
/// structural-macro seam for the application form — the head decodes as a
/// `Name` (always `Local` at decode time) and the tail decodes as a
/// `Vec<TypeReference>`. The derive is the single source of truth for matching
/// and re-emitting the form; this node lowers into [`TypeReference::Application`].
#[derive(Clone, Debug, Eq, PartialEq, nota_next::StructuralMacroNode)]
enum ApplicationNode {
    #[shape(pascal_head, body)]
    Application(Name, Vec<TypeReference>),
}

/// A declaration's type-name position: either a bare `Name` (the ordinary
/// declaration head) or a parameterized head `(Name Param Param …)` that
/// introduces type-parameter binders. The parameterized form is
/// structurally the same captured-head + variable-arity tail as the
/// generic-application form, so it decodes through the *same*
/// `#[shape(pascal_head, body)]` seam ([`ApplicationNode`]) — each tail
/// item must be a bare binder name (a `Plain` reference), since a
/// parameter is a binder, not an applied type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarationHead {
    name: Name,
    parameters: Vec<Name>,
}

impl DeclarationHead {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn parameters(&self) -> &[Name] {
        &self.parameters
    }

    pub fn into_parts(self) -> (Name, Vec<Name>) {
        (self.name, self.parameters)
    }

    /// Decode the declaration-name position from its block. A bare symbol
    /// atom is an ordinary head with no parameters; a parenthesized
    /// `(Name Param …)` reuses the application seam and lifts each binder
    /// out of the decoded tail.
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        match block {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                ..
            } => Self::from_parameterized(block),
            _ => Ok(Self {
                name: block.schema_name()?,
                parameters: Vec::new(),
            }),
        }
    }

    fn from_parameterized(block: &Block) -> Result<Self, SchemaError> {
        let ApplicationNode::Application(name, tail) =
            ApplicationNode::from_structural_block(block)?;
        let mut parameters = Vec::with_capacity(tail.len());
        for argument in tail {
            let TypeReference::Plain(parameter) = argument else {
                return Err(SchemaError::ExpectedTypeParameterName {
                    declaration: name.as_str().to_owned(),
                    found: argument.to_structural_nota(),
                });
            };
            if parameters.iter().any(|existing| existing == &parameter) {
                return Err(SchemaError::DuplicateTypeParameter {
                    declaration: name.as_str().to_owned(),
                    parameter: parameter.as_str().to_owned(),
                });
            }
            parameters.push(parameter);
        }
        Ok(Self { name, parameters })
    }
}

/// A type at a reference position — a struct field's type, an enum
/// variant's payload, or an import source.
///
/// `String`, `Integer`, `Boolean`, and `Path` are reserved scalar leaves.
/// `Plain` is a declared-name leaf (`Topic`, `Magnitude`). `Vector`,
/// `Map`, `Optional`, and `ScopeOf` carry inner references, lowered from the
/// single canonical head spelling each: `(Vector T)`, `(Map K V)`,
/// `(Optional T)`, `(ScopeOf T)` — the earlier aliases (`Vec`, `Option`,
/// `Scope`, `KeyValue`) are gone and no longer parse. `Application` is the
/// broad generic-application form `(Foo A B …)`: any other PascalCase head
/// carrying a tail of type-reference arguments, decoded through the
/// `#[shape(pascal_head, body)]` structural-macro seam. Built-in heads are
/// dispatched first; the application form is the fallback.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
#[rkyv(
    bytecheck(bounds(
        __C: rkyv::validation::ArchiveContext,
        __C::Error: rkyv::rancor::Source
    )),
    serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source
    ),
    deserialize_bounds(__D::Error: rkyv::rancor::Source)
)]
pub enum TypeReference {
    String,
    Integer,
    Boolean,
    Path,
    Bytes,
    FixedBytes(u64),
    Plain(Name),
    Vector(#[rkyv(omit_bounds)] Box<TypeReference>),
    Map(
        #[rkyv(omit_bounds)] Box<TypeReference>,
        #[rkyv(omit_bounds)] Box<TypeReference>,
    ),
    Optional(#[rkyv(omit_bounds)] Box<TypeReference>),
    ScopeOf(#[rkyv(omit_bounds)] Box<TypeReference>),
    Application {
        head: ApplicationHead,
        #[rkyv(omit_bounds)]
        arguments: Vec<TypeReference>,
    },
}

impl NotaDecode for TypeReference {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        if let Some(name) = block.demote_to_string() {
            return match name {
                "String" => Ok(Self::String),
                "Integer" => Ok(Self::Integer),
                "Boolean" => Ok(Self::Boolean),
                "Path" => Ok(Self::Path),
                "Bytes" => Ok(Self::Bytes),
                other => Err(NotaDecodeError::UnknownVariant {
                    enum_name: "TypeReference",
                    variant: other.to_owned(),
                }),
            };
        }
        let children = match block {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                root_objects,
                ..
            } => root_objects.as_slice(),
            _ => {
                return Err(NotaDecodeError::ExpectedDelimited {
                    type_name: "TypeReference",
                    delimiter: "(",
                });
            }
        };
        if children.is_empty() {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: "TypeReference",
                expected: 1,
                found: 0,
            });
        }
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
            "ScopeOf" => Ok(Self::ScopeOf(Box::new(Self::from_nota_block(
                &children[1],
            )?))),
            "Map" => Self::from_nota_map_payload(children),
            "FixedBytes" => Ok(Self::FixedBytes(
                children[1]
                    .demote_to_string()
                    .and_then(|text| text.parse::<u64>().ok())
                    .ok_or(NotaDecodeError::ExpectedAtom {
                        type_name: "FixedBytes width",
                    })?,
            )),
            "Application" => Self::from_nota_application_payload(&children[1]),
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
            Self::Bytes => "Bytes".to_owned(),
            Self::FixedBytes(width) => format!("(FixedBytes {width})"),
            Self::Plain(name) => format!("(Plain {})", name.to_nota()),
            Self::Vector(reference) => format!("(Vector {})", reference.to_nota()),
            Self::Map(key, value) => format!("(Map {} {})", key.to_nota(), value.to_nota()),
            Self::Optional(reference) => format!("(Optional {})", reference.to_nota()),
            Self::ScopeOf(reference) => format!("(ScopeOf {})", reference.to_nota()),
            Self::Application { head, arguments } => {
                let arguments = arguments
                    .iter()
                    .map(Self::to_nota)
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("(Application ({} ({arguments})))", head.name().to_nota())
            }
        }
    }
}

/// `TypeReference` is itself a structural-macro node so the application
/// form's variable-arity tail (`Vec<TypeReference>`, via nota-next's blanket
/// `StructuralMacroNode for Vec<Item>`) can decode each argument back through
/// the full reference grammar. Decode delegates to [`Self::from_block`] (which
/// owns the built-in-head fast path and the application seam), and encode is
/// the source-grammar projection — a bare PascalCase atom for a leaf, a
/// headed parenthesis for every composite. This is the source-facing grammar
/// projection, distinct from the canonical-only `NotaEncode`/`NotaDecode`
/// machine codec above.
impl nota_next::StructuralMacroNode for TypeReference {
    type Error = SchemaError;

    fn structural_position() -> nota_next::PositionPredicate {
        nota_next::PositionPredicate::named("TypeReference")
    }

    fn structural_variants() -> Vec<nota_next::StructuralVariant> {
        vec![
            nota_next::BlockShape::symbol(Some(nota_next::CaptureName::new("reference")))
                .into_structural_variant("TypeReference", "symbol reference atom"),
        ]
    }

    fn from_structural_block(
        block: &Block,
    ) -> Result<Self, nota_next::StructuralMacroError<Self::Error>> {
        Self::from_block(block).map_err(nota_next::StructuralMacroError::MatchedNode)
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
        match self {
            Self::String => "String".to_owned(),
            Self::Integer => "Integer".to_owned(),
            Self::Boolean => "Boolean".to_owned(),
            Self::Path => "Path".to_owned(),
            Self::Bytes => "Bytes".to_owned(),
            Self::FixedBytes(width) => format!("(Bytes {width})"),
            Self::Plain(name) => name.to_nota(),
            Self::Vector(reference) => {
                format!("(Vector {})", reference.to_structural_nota())
            }
            Self::Map(key, value) => {
                format!(
                    "(Map {} {})",
                    key.to_structural_nota(),
                    value.to_structural_nota()
                )
            }
            Self::Optional(reference) => {
                format!("(Optional {})", reference.to_structural_nota())
            }
            Self::ScopeOf(reference) => {
                format!("(ScopeOf {})", reference.to_structural_nota())
            }
            Self::Application { head, arguments } => {
                let tail = arguments
                    .iter()
                    .map(Self::to_structural_nota)
                    .collect::<Vec<_>>()
                    .join(" ");
                if tail.is_empty() {
                    format!("({})", head.name().to_nota())
                } else {
                    format!("({} {tail})", head.name().to_nota())
                }
            }
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
            | Self::ScopeOf(_)
            | Self::Application { .. } => None,
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
            | Self::ScopeOf(_)
            | Self::Application { .. } => None,
        }
    }

    /// Whether this reference is a declared-name leaf.
    pub fn is_plain(&self) -> bool {
        matches!(self, Self::Plain(_))
    }

    fn from_nota_map_payload(children: &[Block]) -> Result<Self, NotaDecodeError> {
        if children.len() != 3 {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: "TypeReference::Map",
                expected: 3,
                found: children.len(),
            });
        }
        Ok(Self::Map(
            Box::new(Self::from_nota_block(&children[1])?),
            Box::new(Self::from_nota_block(&children[2])?),
        ))
    }

    /// Decode the grouped payload of the canonical `Application` machine
    /// projection — `(head (arg0 arg1 …))`. The head always decodes as
    /// `Local`; import resolution rewrites it to `Imported` later.
    fn from_nota_application_payload(block: &Block) -> Result<Self, NotaDecodeError> {
        let children = NotaBlock::new(block).expect_children(
            Delimiter::Parenthesis,
            "TypeReference::Application payload",
            2,
        )?;
        let head = Name::from_nota_block(&children[0])?;
        let argument_blocks = match &children[1] {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                root_objects,
                ..
            } => root_objects.as_slice(),
            _ => {
                return Err(NotaDecodeError::ExpectedDelimited {
                    type_name: "TypeReference::Application arguments",
                    delimiter: "(",
                });
            }
        };
        let arguments = argument_blocks
            .iter()
            .map(Self::from_nota_block)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::Application {
            head: ApplicationHead::Local(head),
            arguments,
        })
    }

    /// Lower an already-parsed NOTA block at a reference position into
    /// a `TypeReference`.
    ///
    /// A bare PascalCase symbol (`Topic`, `schema-core:mail:Magnitude`)
    /// lowers to `Plain`. Schema type-reference objects lower at this
    /// position: `(Vector T)` -> `Vector`, `(Map K V)` -> `Map`,
    /// `(Optional T)` -> `Optional`, and `(ScopeOf T)` -> `ScopeOf`.
    /// The inner positions recurse, so
    /// `(Vector (Optional Topic))` and `(Map NodeName (Vector Service))`
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

    /// Lower a parenthesised reference. Dispatch ORDER is the grammar's
    /// disambiguation — it is deliberately not compiler-checked (the
    /// application form would otherwise conflict with every built-in and
    /// declared head), so the ordering is stated here and pinned by tests:
    ///
    /// 1. The canonical built-in heads — `(Vector T)`, `(Optional T)`,
    ///    `(ScopeOf T)`, `(Map K V)`, `(Bytes N)` — are the direct fast
    ///    path. Each has exactly one canonical spelling; the dropped aliases
    ///    (`Vec`, `Option`, `Scope`, `KeyValue`) no longer parse.
    /// 2. A DECLARED head — a registered user TypeReference macro (e.g.
    ///    `(Bag $Type)`) — is consulted next; a declared head wins over the
    ///    broad application form.
    /// 3. The broad generic-application form `(Foo A B …)` is the
    ///    structural-macro seam ([`ApplicationNode`] via the
    ///    `#[shape(pascal_head, body)]` derive) — the fallback for any other
    ///    PascalCase head whose tail decodes as type-reference arguments.
    fn from_parenthesis_objects(
        block: &Block,
        objects: &[Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        if let Some(head) = objects.first().and_then(Block::demote_to_string) {
            match (head, objects.len()) {
                ("Vector", 2) => {
                    return Ok(Self::Vector(Box::new(Self::from_block_with_registry(
                        &objects[1],
                        registry,
                        context,
                    )?)));
                }
                ("Optional", 2) => {
                    return Ok(Self::Optional(Box::new(Self::from_block_with_registry(
                        &objects[1],
                        registry,
                        context,
                    )?)));
                }
                ("ScopeOf", 2) => {
                    return Ok(Self::ScopeOf(Box::new(Self::from_block_with_registry(
                        &objects[1],
                        registry,
                        context,
                    )?)));
                }
                ("Map", 3) => {
                    return Ok(Self::Map(
                        Box::new(Self::from_block_with_registry(
                            &objects[1],
                            registry,
                            context,
                        )?),
                        Box::new(Self::from_block_with_registry(
                            &objects[2],
                            registry,
                            context,
                        )?),
                    ));
                }
                ("Bytes", 2) => {
                    return Self::from_fixed_bytes_width(&objects[1]);
                }
                ("Vector" | "Optional" | "ScopeOf" | "Map" | "Bytes", found) => {
                    return Err(SchemaError::UnknownTypeReferenceForm {
                        head: head.to_owned(),
                        argument_count: found.saturating_sub(1),
                    });
                }
                _ => {}
            }
        }
        Self::from_macro_or_application(block, registry, context)
    }

    /// The seam between a DECLARED head (a registered user macro) and the
    /// broad generic-application form. A registered TypeReference macro is a
    /// declared head and wins over the application fallback, so the registry
    /// is consulted first; only when no macro matches does the broad
    /// `(Foo A B …)` form decode through the structural-macro seam. This
    /// ordering is the design's disambiguation and is NOT compiler-checked
    /// (the application form structurally overlaps every PascalCase head), so
    /// it is pinned by tests.
    fn from_macro_or_application(
        block: &Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Self, SchemaError> {
        match Self::from_macro_invocation(block, registry, context) {
            Ok(reference) => Ok(reference),
            Err(SchemaError::MacroDidNotMatch { .. })
            | Err(SchemaError::UnknownTypeReferenceForm { .. }) => Self::from_application(block),
            Err(error) => Err(error),
        }
    }

    /// Decode the broad generic-application form `(Foo A B …)` through the
    /// `#[shape(pascal_head, body)]` structural-macro seam ([`ApplicationNode`]).
    /// The head is always `Local` at decode time; import resolution rewrites
    /// it to `Imported` later.
    fn from_application(block: &Block) -> Result<Self, SchemaError> {
        match ApplicationNode::from_structural_block(block)? {
            ApplicationNode::Application(head, arguments) => Ok(Self::Application {
                head: ApplicationHead::Local(head),
                arguments,
            }),
        }
    }

    /// Lower the numeric width of a fixed-size byte reference `(Bytes N)`
    /// into `TypeReference::FixedBytes(N)` — the grammar's one numeric
    /// type-argument.
    fn from_fixed_bytes_width(block: &Block) -> Result<Self, SchemaError> {
        let width = block
            .demote_to_string()
            .and_then(|text| text.parse::<u64>().ok())
            .ok_or_else(|| SchemaError::UnknownTypeReferenceForm {
                head: "Bytes".to_owned(),
                argument_count: 1,
            })?;
        Ok(Self::FixedBytes(width))
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
