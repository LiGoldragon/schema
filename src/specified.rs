use crate::{
    Declaration, EnumDeclaration, EnumVariant, FamilyDeclaration, ImplBlock, ImplCatalog,
    ImportDeclaration, Name, NewtypeDeclaration, RelationDeclaration, Root, RootApplication,
    Schema, SchemaIdentity, StreamDeclaration, StreamRelation, StructDeclaration, TypeDeclaration,
    TypeReference, Visibility,
};

/// The fully specified schema value: the semantic schema object after authored
/// `.schema` sugar has been decoded, resolved, and made explicit.
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
pub struct SpecifiedSchema {
    identity: SchemaIdentity,
    imports: Vec<ImportDeclaration>,
    resolved_imports: Vec<crate::ResolvedImport>,
    input: SpecifiedRoot,
    output: SpecifiedRoot,
    declarations: Vec<SpecifiedDeclaration>,
    streams: Vec<StreamDeclaration>,
    families: Vec<FamilyDeclaration>,
    relations: Vec<RelationDeclaration>,
    impl_blocks: Vec<ImplBlock>,
}

impl SpecifiedSchema {
    pub fn identity(&self) -> &SchemaIdentity {
        &self.identity
    }

    pub fn imports(&self) -> &[ImportDeclaration] {
        &self.imports
    }

    pub fn resolved_imports(&self) -> &[crate::ResolvedImport] {
        &self.resolved_imports
    }

    pub fn input(&self) -> &SpecifiedRoot {
        &self.input
    }

    pub fn output(&self) -> &SpecifiedRoot {
        &self.output
    }

    pub fn declarations(&self) -> &[SpecifiedDeclaration] {
        &self.declarations
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

    pub fn impl_blocks(&self) -> &[ImplBlock] {
        &self.impl_blocks
    }

    pub fn declaration_named(&self, name: &str) -> Option<&SpecifiedDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, crate::SchemaError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|_| crate::SchemaError::ArchiveDecode)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, crate::SchemaError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|bytes| bytes.to_vec())
            .map_err(|_| crate::SchemaError::ArchiveEncode)
    }
}

impl From<&Schema> for SpecifiedSchema {
    fn from(schema: &Schema) -> Self {
        SpecifiedSchemaBuilder::new(schema).into_schema()
    }
}

/// One fully specified root position.
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
pub enum SpecifiedRoot {
    Enum(SpecifiedRootEnum),
    Application(SpecifiedRootApplication),
}

impl SpecifiedRoot {
    pub fn name(&self) -> &Name {
        match self {
            Self::Enum(root) => root.name(),
            Self::Application(root) => root.name(),
        }
    }

    pub fn as_enum(&self) -> Option<&SpecifiedRootEnum> {
        match self {
            Self::Enum(root) => Some(root),
            Self::Application(_) => None,
        }
    }

    pub fn as_application(&self) -> Option<&SpecifiedRootApplication> {
        match self {
            Self::Application(root) => Some(root),
            Self::Enum(_) => None,
        }
    }
}

/// A concrete root enum with every variant payload made explicit.
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
pub struct SpecifiedRootEnum {
    name: Name,
    variants: Vec<SpecifiedVariant>,
}

impl SpecifiedRootEnum {
    pub fn new(name: Name, variants: Vec<SpecifiedVariant>) -> Self {
        Self { name, variants }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn variants(&self) -> &[SpecifiedVariant] {
        &self.variants
    }

    pub fn variant_named(&self, name: &str) -> Option<&SpecifiedVariant> {
        self.variants
            .iter()
            .find(|variant| variant.name().as_str() == name)
    }
}

/// A root application and the concrete enum body it denotes when the frame can
/// be expanded in this schema.
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
pub struct SpecifiedRootApplication {
    name: Name,
    reference: TypeReference,
    expanded: Option<SpecifiedRootEnum>,
}

impl SpecifiedRootApplication {
    pub fn new(name: Name, reference: TypeReference, expanded: Option<SpecifiedRootEnum>) -> Self {
        Self {
            name,
            reference,
            expanded,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }

    pub fn expanded(&self) -> Option<&SpecifiedRootEnum> {
        self.expanded.as_ref()
    }
}

/// A namespace declaration in fully specified form.
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
pub struct SpecifiedDeclaration {
    visibility: Visibility,
    name: Name,
    parameters: Vec<Name>,
    body: SpecifiedDeclarationBody,
    impls: ImplCatalog,
}

impl SpecifiedDeclaration {
    pub fn new(
        visibility: Visibility,
        name: Name,
        parameters: Vec<Name>,
        body: SpecifiedDeclarationBody,
        impls: ImplCatalog,
    ) -> Self {
        Self {
            visibility,
            name,
            parameters,
            body,
            impls,
        }
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn parameters(&self) -> &[Name] {
        &self.parameters
    }

    pub fn body(&self) -> &SpecifiedDeclarationBody {
        &self.body
    }

    pub fn impls(&self) -> &ImplCatalog {
        &self.impls
    }
}

/// The explicit body a declaration denotes.
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
pub enum SpecifiedDeclarationBody {
    Newtype(TypeReference),
    Struct(Vec<SpecifiedField>),
    Enum(Vec<SpecifiedVariant>),
}

impl SpecifiedDeclarationBody {
    pub fn as_struct(&self) -> Option<&[SpecifiedField]> {
        match self {
            Self::Struct(fields) => Some(fields),
            Self::Newtype(_) | Self::Enum(_) => None,
        }
    }

    pub fn as_enum(&self) -> Option<&[SpecifiedVariant]> {
        match self {
            Self::Enum(variants) => Some(variants),
            Self::Newtype(_) | Self::Struct(_) => None,
        }
    }

    pub fn as_newtype(&self) -> Option<&TypeReference> {
        match self {
            Self::Newtype(reference) => Some(reference),
            Self::Struct(_) | Self::Enum(_) => None,
        }
    }
}

/// A fully specified struct field: the role name and resolved reference are
/// both explicit.
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
pub struct SpecifiedField {
    name: Name,
    reference: TypeReference,
}

impl SpecifiedField {
    pub fn new(name: Name, reference: TypeReference) -> Self {
        Self { name, reference }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }
}

/// A fully specified enum variant. The variant's payload includes both the
/// immediate type reference and the data shape reached after transparent
/// schema newtypes are collapsed.
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
pub struct SpecifiedVariant {
    name: Name,
    payload: Option<SpecifiedPayload>,
    stream_relation: Option<StreamRelation>,
}

impl SpecifiedVariant {
    pub fn new(
        name: Name,
        payload: Option<SpecifiedPayload>,
        stream_relation: Option<StreamRelation>,
    ) -> Self {
        Self {
            name,
            payload,
            stream_relation,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> Option<&SpecifiedPayload> {
        self.payload.as_ref()
    }

    pub fn stream_relation(&self) -> Option<&StreamRelation> {
        self.stream_relation.as_ref()
    }
}

/// A variant payload made explicit for schema consumers.
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
pub struct SpecifiedPayload {
    reference: TypeReference,
    immediate_body: Option<SpecifiedPayloadBody>,
    shape: SpecifiedPayloadShape,
}

impl SpecifiedPayload {
    pub fn new(
        reference: TypeReference,
        immediate_body: Option<SpecifiedPayloadBody>,
        shape: SpecifiedPayloadShape,
    ) -> Self {
        Self {
            reference,
            immediate_body,
            shape,
        }
    }

    pub fn reference(&self) -> &TypeReference {
        &self.reference
    }

    pub fn immediate_body(&self) -> Option<&SpecifiedPayloadBody> {
        self.immediate_body.as_ref()
    }

    pub fn shape(&self) -> &SpecifiedPayloadShape {
        &self.shape
    }
}

/// The bounded declaration body directly named by a payload reference. Unlike
/// a namespace declaration, this summary does not recursively expand nested
/// enum payloads; those remain navigable references in the same schema value.
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
pub enum SpecifiedPayloadBody {
    Newtype(TypeReference),
    Struct(Vec<SpecifiedField>),
    Enum(Vec<SpecifiedVariantSummary>),
}

impl SpecifiedPayloadBody {
    pub fn as_struct(&self) -> Option<&[SpecifiedField]> {
        match self {
            Self::Struct(fields) => Some(fields),
            Self::Newtype(_) | Self::Enum(_) => None,
        }
    }

    pub fn as_enum(&self) -> Option<&[SpecifiedVariantSummary]> {
        match self {
            Self::Enum(variants) => Some(variants),
            Self::Newtype(_) | Self::Struct(_) => None,
        }
    }

    pub fn as_newtype(&self) -> Option<&TypeReference> {
        match self {
            Self::Newtype(reference) => Some(reference),
            Self::Struct(_) | Self::Enum(_) => None,
        }
    }
}

/// A bounded enum variant summary used inside payload bodies and shape
/// summaries. It names the payload reference but does not recursively expand
/// it, so the specified schema remains a finite first-class value.
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
pub struct SpecifiedVariantSummary {
    name: Name,
    payload: Option<TypeReference>,
    stream_relation: Option<StreamRelation>,
}

impl SpecifiedVariantSummary {
    pub fn new(
        name: Name,
        payload: Option<TypeReference>,
        stream_relation: Option<StreamRelation>,
    ) -> Self {
        Self {
            name,
            payload,
            stream_relation,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> Option<&TypeReference> {
        self.payload.as_ref()
    }

    pub fn stream_relation(&self) -> Option<&StreamRelation> {
        self.stream_relation.as_ref()
    }
}

/// The data shape a payload presents after transparent newtypes are followed.
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
pub enum SpecifiedPayloadShape {
    Scalar(TypeReference),
    Reference(TypeReference),
    Struct(Vec<SpecifiedField>),
    Enum(Vec<SpecifiedVariantSummary>),
}

impl SpecifiedPayloadShape {
    pub fn as_struct(&self) -> Option<&[SpecifiedField]> {
        match self {
            Self::Struct(fields) => Some(fields),
            Self::Scalar(_) | Self::Reference(_) | Self::Enum(_) => None,
        }
    }

    pub fn as_enum(&self) -> Option<&[SpecifiedVariantSummary]> {
        match self {
            Self::Enum(variants) => Some(variants),
            Self::Scalar(_) | Self::Reference(_) | Self::Struct(_) => None,
        }
    }
}

struct SpecifiedSchemaBuilder<'schema> {
    schema: &'schema Schema,
}

impl<'schema> SpecifiedSchemaBuilder<'schema> {
    fn new(schema: &'schema Schema) -> Self {
        Self { schema }
    }

    fn into_schema(self) -> SpecifiedSchema {
        SpecifiedSchema {
            identity: self.schema.identity().clone(),
            imports: self.schema.imports().to_vec(),
            resolved_imports: self.schema.resolved_imports().to_vec(),
            input: self.specified_root(self.schema.input()),
            output: self.specified_root(self.schema.output()),
            declarations: self
                .schema
                .namespace()
                .iter()
                .map(|declaration| self.specified_declaration(declaration))
                .collect(),
            streams: self.schema.streams().to_vec(),
            families: self.schema.families().to_vec(),
            relations: self.schema.relations().to_vec(),
            impl_blocks: self.schema.impl_blocks().to_vec(),
        }
    }

    fn specified_root(&self, root: &Root) -> SpecifiedRoot {
        match root {
            Root::Enum(declaration) => SpecifiedRoot::Enum(self.specified_root_enum(declaration)),
            Root::Application(application) => {
                SpecifiedRoot::Application(self.specified_application_root(application))
            }
        }
    }

    fn specified_application_root(
        &self,
        application: &RootApplication,
    ) -> SpecifiedRootApplication {
        let reference = TypeReference::from(application);
        let expanded = self
            .schema
            .expand_application_root(application)
            .map(|declaration| self.specified_root_enum(&declaration));
        SpecifiedRootApplication::new(application.name().clone(), reference, expanded)
    }

    fn specified_root_enum(&self, declaration: &EnumDeclaration) -> SpecifiedRootEnum {
        SpecifiedRootEnum::new(
            declaration.name.clone(),
            declaration
                .variants
                .iter()
                .map(|variant| self.specified_variant(variant))
                .collect(),
        )
    }

    fn specified_declaration(&self, declaration: &Declaration) -> SpecifiedDeclaration {
        SpecifiedDeclaration::new(
            declaration.visibility(),
            declaration.name().clone(),
            declaration.parameters().to_vec(),
            self.specified_body(declaration.value()),
            declaration.impls().clone(),
        )
    }

    fn specified_body(&self, declaration: &TypeDeclaration) -> SpecifiedDeclarationBody {
        match declaration {
            TypeDeclaration::Newtype(declaration) => self.specified_newtype(declaration),
            TypeDeclaration::Struct(declaration) => self.specified_struct(declaration),
            TypeDeclaration::Enum(declaration) => self.specified_enum(declaration),
        }
    }

    fn specified_newtype(&self, declaration: &NewtypeDeclaration) -> SpecifiedDeclarationBody {
        SpecifiedDeclarationBody::Newtype(declaration.reference.clone())
    }

    fn specified_struct(&self, declaration: &StructDeclaration) -> SpecifiedDeclarationBody {
        SpecifiedDeclarationBody::Struct(
            declaration
                .fields
                .iter()
                .map(|field| SpecifiedField::new(field.name.clone(), field.reference.clone()))
                .collect(),
        )
    }

    fn specified_enum(&self, declaration: &EnumDeclaration) -> SpecifiedDeclarationBody {
        SpecifiedDeclarationBody::Enum(
            declaration
                .variants
                .iter()
                .map(|variant| self.specified_variant(variant))
                .collect(),
        )
    }

    fn specified_variant(&self, variant: &EnumVariant) -> SpecifiedVariant {
        SpecifiedVariant::new(
            variant.name.clone(),
            variant
                .payload
                .as_ref()
                .map(|reference| self.specified_payload(reference)),
            variant.stream_relation.clone(),
        )
    }

    fn specified_payload(&self, reference: &TypeReference) -> SpecifiedPayload {
        SpecifiedPayload::new(
            reference.clone(),
            self.payload_body_for_reference(reference),
            self.payload_shape_for_reference(reference, Vec::new()),
        )
    }

    fn payload_body_for_reference(
        &self,
        reference: &TypeReference,
    ) -> Option<SpecifiedPayloadBody> {
        let TypeReference::Plain(name) = reference else {
            return None;
        };
        self.schema
            .type_named(name.as_str())
            .map(|declaration| self.specified_payload_body(declaration))
    }

    fn specified_payload_body(&self, declaration: &TypeDeclaration) -> SpecifiedPayloadBody {
        match declaration {
            TypeDeclaration::Newtype(declaration) => {
                SpecifiedPayloadBody::Newtype(declaration.reference.clone())
            }
            TypeDeclaration::Struct(declaration) => match self.specified_struct(declaration) {
                SpecifiedDeclarationBody::Struct(fields) => SpecifiedPayloadBody::Struct(fields),
                SpecifiedDeclarationBody::Newtype(_) | SpecifiedDeclarationBody::Enum(_) => {
                    unreachable!("specified_struct returns Struct")
                }
            },
            TypeDeclaration::Enum(declaration) => SpecifiedPayloadBody::Enum(
                declaration
                    .variants
                    .iter()
                    .map(SpecifiedVariantSummary::from)
                    .collect(),
            ),
        }
    }

    fn payload_shape_for_reference(
        &self,
        reference: &TypeReference,
        mut visited: Vec<Name>,
    ) -> SpecifiedPayloadShape {
        match reference {
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::FixedBytes(_) => SpecifiedPayloadShape::Scalar(reference.clone()),
            TypeReference::Plain(name) => {
                if visited.iter().any(|visited| visited == name) {
                    return SpecifiedPayloadShape::Reference(reference.clone());
                }
                visited.push(name.clone());
                match self.schema.type_named(name.as_str()) {
                    Some(TypeDeclaration::Newtype(declaration)) => {
                        self.payload_shape_for_reference(&declaration.reference, visited)
                    }
                    Some(TypeDeclaration::Struct(declaration)) => {
                        match self.specified_struct(declaration) {
                            SpecifiedDeclarationBody::Struct(fields) => {
                                SpecifiedPayloadShape::Struct(fields)
                            }
                            SpecifiedDeclarationBody::Newtype(_)
                            | SpecifiedDeclarationBody::Enum(_) => {
                                unreachable!("specified_struct returns Struct")
                            }
                        }
                    }
                    Some(TypeDeclaration::Enum(declaration)) => SpecifiedPayloadShape::Enum(
                        declaration
                            .variants
                            .iter()
                            .map(SpecifiedVariantSummary::from)
                            .collect(),
                    ),
                    None => SpecifiedPayloadShape::Reference(reference.clone()),
                }
            }
            TypeReference::Vector(_)
            | TypeReference::Map(..)
            | TypeReference::Optional(_)
            | TypeReference::ScopeOf(_)
            | TypeReference::Application { .. } => {
                SpecifiedPayloadShape::Reference(reference.clone())
            }
        }
    }
}

impl From<&EnumVariant> for SpecifiedVariantSummary {
    fn from(variant: &EnumVariant) -> Self {
        Self::new(
            variant.name.clone(),
            variant.payload.clone(),
            variant.stream_relation.clone(),
        )
    }
}
