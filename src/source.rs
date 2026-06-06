use std::{
    fs,
    path::{Path, PathBuf},
};

use nota_next::{
    Block, CaptureName, Delimiter, Document, MacroCandidate, NotaBody, NotaEncode, NotaString,
    StructuralMacroError, StructuralMacroNode, StructuralVariant,
};

use crate::{
    AliasDeclaration, Declaration, EnumDeclaration, EnumVariant, FieldDeclaration,
    ImportDeclaration, Name, NewtypeDeclaration, RawNotaDatatype, RawNotaSequence, ResolvedImport,
    Schema, SchemaEngine, SchemaError, SchemaIdentity, StreamDeclaration, StreamRelation,
    StructDeclaration, TypeDeclaration, TypeReference, macros::BlockDebug,
};

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SchemaSource {
    imports: SourceImports,
    input: SourceRootEnum,
    output: SourceRootEnum,
    namespace: SourceNamespace,
}

impl SchemaSource {
    pub fn from_schema_text(source: &str) -> Result<Self, SchemaError> {
        let document = Document::parse(source)?;
        Self::from_document(&document)
    }

    pub fn from_document(document: &Document) -> Result<Self, SchemaError> {
        if !matches!(document.holds_root_objects(), 3 | 4) {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: "3 root values (input output namespace) or 4 with leading imports",
                found: document.holds_root_objects(),
            });
        }

        let (imports, input_index, output_index, namespace_index) = if document.holds_root_objects()
            == 4
        {
            (
                SourceImports::from_block(document.root_object_at(0).expect("checked root count"))?,
                1,
                2,
                3,
            )
        } else {
            (SourceImports::empty(), 0, 1, 2)
        };

        Ok(Self {
            imports,
            input: SourceRootEnum::from_block(
                Name::new("Input"),
                document
                    .root_object_at(input_index)
                    .expect("checked root count"),
            )?,
            output: SourceRootEnum::from_block(
                Name::new("Output"),
                document
                    .root_object_at(output_index)
                    .expect("checked root count"),
            )?,
            namespace: SourceNamespace::from_block(
                document
                    .root_object_at(namespace_index)
                    .expect("checked root count"),
            )?,
        })
    }

    pub fn imports(&self) -> &SourceImports {
        &self.imports
    }

    pub fn input(&self) -> &SourceRootEnum {
        &self.input
    }

    pub fn output(&self) -> &SourceRootEnum {
        &self.output
    }

    pub fn namespace(&self) -> &SourceNamespace {
        &self.namespace
    }

    pub fn stream_declarations(&self) -> Result<Vec<StreamDeclaration>, SchemaError> {
        self.namespace.stream_declarations()
    }

    pub fn to_schema_text(&self) -> String {
        [
            self.imports.to_schema_text(),
            self.input.body().to_schema_text(),
            self.output.body().to_schema_text(),
            self.namespace.to_schema_text(),
        ]
        .join("\n")
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes).map_err(|_| SchemaError::ArchiveDecode)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|bytes| bytes.to_vec())
            .map_err(|_| SchemaError::ArchiveEncode)
    }

    pub fn lower(
        &self,
        engine: &SchemaEngine,
        identity: SchemaIdentity,
    ) -> Result<crate::Schema, SchemaError> {
        engine.lower_schema_source(self, identity)
    }

    pub(crate) fn to_schema(
        &self,
        identity: SchemaIdentity,
        imports: Vec<ImportDeclaration>,
        resolved_imports: Vec<ResolvedImport>,
    ) -> Result<Schema, SchemaError> {
        let resolver = SourceTypeResolver::from_source(self);
        let mut namespace = SourceLoweredNamespace::from_source(&self.namespace, &resolver)?;
        namespace.push_public_declarations(self.input.public_inline_declarations(&resolver)?)?;
        namespace.push_public_declarations(self.output.public_inline_declarations(&resolver)?)?;
        let streams = self.namespace.stream_declarations()?;
        let input = self.input.to_schema_enum(&namespace)?;
        let output = self.output.to_schema_enum(&namespace)?;
        Ok(Schema::new(
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace.into_declarations(),
            streams,
        ))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SchemaSourceArtifact(SchemaSource);

impl SchemaSourceArtifact {
    pub fn new(source: SchemaSource) -> Self {
        Self(source)
    }

    pub fn source(&self) -> &SchemaSource {
        &self.0
    }

    pub fn into_source(self) -> SchemaSource {
        self.0
    }

    pub fn from_schema_text(source: &str) -> Result<Self, SchemaError> {
        SchemaSource::from_schema_text(source).map(Self::new)
    }

    pub fn to_schema_text(&self) -> String {
        self.0.to_schema_text()
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        SchemaSource::from_binary_bytes(bytes).map(Self::new)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        self.0.to_binary_bytes()
    }

    pub fn read_schema_file(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let artifact_path = SchemaSourceArtifactPath::new(path.as_ref());
        let source = fs::read_to_string(artifact_path.path())
            .map_err(|error| artifact_path.io_error(error))?;
        Self::from_schema_text(&source)
    }

    pub fn write_schema_file(&self, path: impl AsRef<Path>) -> Result<(), SchemaError> {
        let artifact_path = SchemaSourceArtifactPath::new(path.as_ref());
        fs::write(artifact_path.path(), self.to_schema_text())
            .map_err(|error| artifact_path.io_error(error))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SchemaSourceArtifactPath(PathBuf);

impl SchemaSourceArtifactPath {
    fn new(path: &Path) -> Self {
        Self(path.to_path_buf())
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn io_error(&self, error: std::io::Error) -> SchemaError {
        SchemaError::Io {
            path: self.0.display().to_string(),
            reason: error.to_string(),
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceImports {
    entries: Vec<SourceImport>,
}

impl SourceImports {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn entries(&self) -> &[SourceImport] {
        &self.entries
    }

    pub(crate) fn to_schema_imports(&self) -> Result<Vec<ImportDeclaration>, SchemaError> {
        self.entries
            .iter()
            .map(SourceImport::to_schema_import)
            .collect()
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "source imports")?;
        if body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: body.root_objects().len(),
            });
        }

        let mut entries = Vec::new();
        for pair in body.root_objects().chunks_exact(2) {
            entries.push(SourceImport {
                local_name: SourceAtom::from_block(&pair[0])?.into_name(),
                source: SourceReference::from_block(&pair[1])?,
            });
        }
        Ok(Self { entries })
    }

    fn to_schema_text(&self) -> String {
        if self.entries.is_empty() {
            return "{}".to_owned();
        }
        let entries = self
            .entries
            .iter()
            .map(|entry| format!("  {}", entry.to_schema_text()))
            .collect::<Vec<_>>();
        format!("{{\n{}\n}}", entries.join("\n"))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceImport {
    local_name: Name,
    source: SourceReference,
}

impl SourceImport {
    pub fn local_name(&self) -> &Name {
        &self.local_name
    }

    pub fn source(&self) -> &SourceReference {
        &self.source
    }

    fn to_schema_text(&self) -> String {
        format!(
            "{} {}",
            self.local_name.to_nota(),
            self.source.to_schema_text()
        )
    }

    fn to_schema_import(&self) -> Result<ImportDeclaration, SchemaError> {
        Ok(ImportDeclaration {
            local_name: self.local_name.clone(),
            source: self.source.to_type_reference(),
        })
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceRootEnum {
    name: Name,
    body: SourceEnumBody,
}

impl SourceRootEnum {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn body(&self) -> &SourceEnumBody {
        &self.body
    }

    fn from_block(name: Name, block: &Block) -> Result<Self, SchemaError> {
        Ok(Self {
            name,
            body: SourceEnumBody::from_block(block)?,
        })
    }

    fn public_inline_declarations(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<Vec<Declaration>, SchemaError> {
        let mut declarations = Vec::new();
        for declaration in self.body.public_inline_declarations(resolver)? {
            declarations.push(declaration);
        }
        Ok(declarations)
    }

    fn to_schema_enum(
        &self,
        namespace: &SourceLoweredNamespace,
    ) -> Result<EnumDeclaration, SchemaError> {
        self.body.to_schema_enum(self.name.clone(), namespace)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceNamespace {
    entries: Vec<SourceNamespaceEntry>,
}

impl SourceNamespace {
    pub fn entries(&self) -> &[SourceNamespaceEntry] {
        &self.entries
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "source namespace")?;
        if body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: body.root_objects().len(),
            });
        }
        let mut entries = Vec::new();
        for pair in body.root_objects().chunks_exact(2) {
            entries.push(SourceNamespaceEntry {
                name: SourceAtom::from_block(&pair[0])?.into_name(),
                value: SourceDeclarationValue::from_block(&pair[1])?,
            });
        }
        Ok(Self { entries })
    }

    fn to_schema_text(&self) -> String {
        if self.entries.is_empty() {
            return "{}".to_owned();
        }
        let entries = self
            .entries
            .iter()
            .map(|entry| format!("  {}", entry.to_schema_text()))
            .collect::<Vec<_>>();
        format!("{{\n{}\n}}", entries.join("\n"))
    }

    fn stream_declarations(&self) -> Result<Vec<StreamDeclaration>, SchemaError> {
        let mut streams = Vec::new();
        for entry in &self.entries {
            if let Some(stream) = entry.to_stream_declaration() {
                if streams
                    .iter()
                    .any(|existing: &StreamDeclaration| existing.name == stream.name)
                {
                    return Err(SchemaError::DuplicateSourceDeclaration {
                        name: stream.name.as_str().to_owned(),
                    });
                }
                streams.push(stream);
            }
        }
        Ok(streams)
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceNamespaceEntry {
    name: Name,
    value: SourceDeclarationValue,
}

impl SourceNamespaceEntry {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn value(&self) -> &SourceDeclarationValue {
        &self.value
    }

    fn to_schema_text(&self) -> String {
        format!("{} {}", self.name.to_nota(), self.value.to_schema_text())
    }

    fn to_declaration_group(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceDeclarationGroup, SchemaError> {
        self.value.to_declaration_group(self.name.clone(), resolver)
    }

    fn to_stream_declaration(&self) -> Option<StreamDeclaration> {
        self.value.to_stream_declaration(self.name.clone())
    }

    fn is_type_declaration(&self) -> bool {
        self.value.is_type_declaration()
    }
}

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
pub enum SourceDeclarationValue {
    Reference(SourceReference),
    Text(String),
    Struct(#[rkyv(omit_bounds)] SourceStructBody),
    Enum(#[rkyv(omit_bounds)] SourceEnumBody),
    Stream(#[rkyv(omit_bounds)] SourceStreamBody),
}

impl SourceDeclarationValue {
    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        match block {
            Block::Atom(_) => Ok(Self::Reference(SourceReference::from_block(block)?)),
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                ..
            } => match Self::from_stream_block(block)? {
                Some(value) => Ok(value),
                None => Ok(Self::Reference(SourceReference::from_block(block)?)),
            },
            Block::PipeText(text) => Ok(Self::Text(text.text.clone())),
            Block::Delimited {
                delimiter: Delimiter::Brace,
                ..
            } => Ok(Self::Struct(SourceStructBody::from_block(block)?)),
            Block::Delimited {
                delimiter: Delimiter::SquareBracket,
                ..
            } => Ok(Self::Enum(SourceEnumBody::from_block(block)?)),
            Block::Delimited {
                delimiter: Delimiter::PipeParenthesis | Delimiter::PipeBrace,
                ..
            } => Err(SchemaError::ExpectedSyntaxDeclaration {
                found: block.reemit_fallback(),
            }),
        }
    }

    fn from_stream_block(block: &Block) -> Result<Option<Self>, SchemaError> {
        SourceStreamBody::from_block(block).map(|body| body.map(Self::Stream))
    }

    fn to_schema_text(&self) -> String {
        match self {
            Self::Reference(reference) => reference.to_schema_text(),
            Self::Text(text) => NotaString::new(text).format(),
            Self::Struct(body) => body.to_schema_text(),
            Self::Enum(body) => body.to_schema_text(),
            Self::Stream(body) => body.to_schema_text(),
        }
    }

    fn to_declaration_group(
        &self,
        name: Name,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceDeclarationGroup, SchemaError> {
        match self {
            Self::Reference(reference) => Ok(SourceDeclarationGroup::primary(
                TypeDeclaration::Alias(AliasDeclaration::new(name, reference.to_type_reference())),
            )),
            Self::Text(_) => Err(SchemaError::ExpectedSyntaxDeclaration {
                found: "text declaration".to_owned(),
            }),
            Self::Struct(body) => body.to_declaration_group(name, resolver),
            Self::Enum(body) => body.to_declaration_group(name, resolver),
            Self::Stream(_) => Ok(SourceDeclarationGroup::empty()),
        }
    }

    fn to_stream_declaration(&self, name: Name) -> Option<StreamDeclaration> {
        match self {
            Self::Stream(body) => Some(body.to_stream_declaration(name)),
            Self::Reference(_) | Self::Text(_) | Self::Struct(_) | Self::Enum(_) => None,
        }
    }

    fn is_type_declaration(&self) -> bool {
        !matches!(self, Self::Stream(_))
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceStreamBody {
    token: SourceReference,
    opened: SourceReference,
    event: SourceReference,
    close: SourceReference,
}

impl SourceStreamBody {
    pub fn token(&self) -> &SourceReference {
        &self.token
    }

    pub fn opened(&self) -> &SourceReference {
        &self.opened
    }

    pub fn event(&self) -> &SourceReference {
        &self.event
    }

    pub fn close(&self) -> &SourceReference {
        &self.close
    }

    fn from_block(block: &Block) -> Result<Option<Self>, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Parenthesis, "source stream body")?;
        let objects = body.root_objects();
        let Some(head) = objects.first().and_then(Block::demote_to_string) else {
            return Ok(None);
        };
        if head != "Stream" {
            return Ok(None);
        }
        if objects.len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "stream declaration",
                expected: "Stream plus one brace payload",
                found: objects.len(),
            });
        }
        let fields = SourceStreamFields::from_block(&objects[1])?;
        Ok(Some(fields.into_stream_body()?))
    }

    fn to_schema_text(&self) -> String {
        Delimiter::Parenthesis.wrap([
            "Stream".to_owned(),
            SourceDelimitedText::new(
                Delimiter::Brace,
                vec![
                    format!("token {}", self.token.to_schema_text()),
                    format!("opened {}", self.opened.to_schema_text()),
                    format!("event {}", self.event.to_schema_text()),
                    format!("close {}", self.close.to_schema_text()),
                ],
            )
            .inline(),
        ])
    }

    fn to_stream_declaration(&self, name: Name) -> StreamDeclaration {
        StreamDeclaration::new(
            name,
            self.token.to_type_reference(),
            self.opened.to_type_reference(),
            self.event.to_type_reference(),
            self.close.to_type_reference(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceStreamFields {
    token: Option<SourceReference>,
    opened: Option<SourceReference>,
    event: Option<SourceReference>,
    close: Option<SourceReference>,
}

impl SourceStreamFields {
    fn empty() -> Self {
        Self {
            token: None,
            opened: None,
            event: None,
            close: None,
        }
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "stream declaration fields")?;
        if body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: body.root_objects().len(),
            });
        }
        let mut fields = Self::empty();
        for pair in body.root_objects().chunks_exact(2) {
            let field = SourceAtom::from_block(&pair[0])?;
            let reference = SourceReference::from_block(&pair[1])?;
            fields.insert(field, reference)?;
        }
        Ok(fields)
    }

    fn insert(
        &mut self,
        field: SourceAtom<'_>,
        reference: SourceReference,
    ) -> Result<(), SchemaError> {
        match field.0 {
            "token" => self.token = Some(reference),
            "opened" => self.opened = Some(reference),
            "event" => self.event = Some(reference),
            "close" => self.close = Some(reference),
            other => {
                return Err(SchemaError::ExpectedSyntaxDeclaration {
                    found: format!("stream field {other}"),
                });
            }
        }
        Ok(())
    }

    fn into_stream_body(self) -> Result<SourceStreamBody, SchemaError> {
        Ok(SourceStreamBody {
            token: Self::required_field(self.token, "token")?,
            opened: Self::required_field(self.opened, "opened")?,
            event: Self::required_field(self.event, "event")?,
            close: Self::required_field(self.close, "close")?,
        })
    }

    fn required_field(
        field: Option<SourceReference>,
        field_name: &'static str,
    ) -> Result<SourceReference, SchemaError> {
        field.ok_or_else(|| SchemaError::ExpectedSyntaxDeclaration {
            found: format!("stream missing {field_name} field"),
        })
    }
}

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
pub struct SourceStructBody {
    #[rkyv(omit_bounds)]
    fields: Vec<SourceField>,
}

impl SourceStructBody {
    pub fn fields(&self) -> &[SourceField] {
        &self.fields
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "source struct body")?;
        if body.root_objects().len() % 2 != 0 {
            return Err(SchemaError::ExpectedEvenMapEntries {
                found: body.root_objects().len(),
            });
        }
        let mut fields = Vec::new();
        for pair in body.root_objects().chunks_exact(2) {
            fields.push(SourceField::from_pair(&pair[0], &pair[1])?);
        }
        Ok(Self { fields })
    }

    fn to_schema_text(&self) -> String {
        if self.fields.is_empty() {
            return "{}".to_owned();
        }
        let fields = self
            .fields
            .iter()
            .map(SourceField::to_schema_text)
            .collect::<Vec<_>>();
        SourceDelimitedText::new(Delimiter::Brace, fields).inline()
    }

    fn to_declaration_group(
        &self,
        name: Name,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceDeclarationGroup, SchemaError> {
        let mut private = Vec::new();
        let mut fields = Vec::new();
        for field in &self.fields {
            let lowered = field.to_lowered_field(resolver)?;
            private.extend(lowered.private_declarations);
            fields.push(lowered.field);
        }
        let primary = if fields.len() == 1 {
            TypeDeclaration::Newtype(NewtypeDeclaration::new(name, fields[0].reference.clone()))
        } else {
            TypeDeclaration::Struct(StructDeclaration::new(name, fields))
        };
        Ok(SourceDeclarationGroup::new(private, primary))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDelimitedText {
    delimiter: Delimiter,
    children: Vec<String>,
}

impl SourceDelimitedText {
    fn new(delimiter: Delimiter, children: Vec<String>) -> Self {
        Self {
            delimiter,
            children,
        }
    }

    fn inline(&self) -> String {
        if self.children.is_empty() {
            return format!(
                "{}{}",
                self.delimiter.opening_text(),
                self.delimiter.closing_text()
            );
        }
        format!(
            "{} {} {}",
            self.delimiter.opening_text(),
            self.children.join(" "),
            self.delimiter.closing_text()
        )
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceField {
    name: Name,
    value: SourceFieldValue,
}

impl SourceField {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn value(&self) -> &SourceFieldValue {
        &self.value
    }

    fn to_schema_text(&self) -> String {
        format!("{} {}", self.name.to_nota(), self.value.to_schema_text())
    }

    fn from_pair(name: &Block, value: &Block) -> Result<Self, SchemaError> {
        Ok(Self {
            name: SourceAtom::from_block(name)?.into_name(),
            value: SourceFieldValue::from_block(value)?,
        })
    }

    fn to_lowered_field(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceLoweredField, SchemaError> {
        match &self.value {
            SourceFieldValue::Derived => Ok(SourceLoweredField::new(
                Vec::new(),
                FieldDeclaration {
                    name: Name::new(self.name.field_name()),
                    reference: TypeReference::from_name(self.name.clone()),
                },
            )),
            SourceFieldValue::Reference(reference)
                if SourceIdentifierCase::new(&self.name).is_type() =>
            {
                Ok(SourceLoweredField::new(
                    vec![TypeDeclaration::Newtype(NewtypeDeclaration::new(
                        self.name.clone(),
                        reference.to_type_reference(),
                    ))],
                    FieldDeclaration {
                        name: Name::new(self.name.field_name()),
                        reference: TypeReference::from_name(self.name.clone()),
                    },
                ))
            }
            SourceFieldValue::Reference(reference) => Ok(SourceLoweredField::new(
                Vec::new(),
                FieldDeclaration {
                    name: Name::new(self.name.field_name()),
                    reference: reference.to_type_reference(),
                },
            )),
            SourceFieldValue::Declaration(value)
                if SourceIdentifierCase::new(&self.name).is_type() =>
            {
                let group = value.to_declaration_group(self.name.clone(), resolver)?;
                Ok(SourceLoweredField::new(
                    group.into_type_declarations(),
                    FieldDeclaration {
                        name: Name::new(self.name.field_name()),
                        reference: TypeReference::from_name(self.name.clone()),
                    },
                ))
            }
            SourceFieldValue::Declaration(_) => Err(SchemaError::ExpectedSyntaxDeclaration {
                found: format!("inline declaration field {}", self.name),
            }),
        }
    }
}

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
pub enum SourceFieldValue {
    Derived,
    Reference(SourceReference),
    Declaration(#[rkyv(omit_bounds)] SourceDeclarationValue),
}

impl SourceFieldValue {
    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        if block.demote_to_string() == Some("*") {
            return Ok(Self::Derived);
        }
        match SourceReference::from_block(block) {
            Ok(reference) => Ok(Self::Reference(reference)),
            Err(_) => SourceDeclarationValue::from_block(block).map(Self::Declaration),
        }
    }

    fn to_schema_text(&self) -> String {
        match self {
            Self::Derived => "*".to_owned(),
            Self::Reference(reference) => reference.to_schema_text(),
            Self::Declaration(value) => value.to_schema_text(),
        }
    }
}

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
pub struct SourceEnumBody {
    #[rkyv(omit_bounds)]
    variants: Vec<SourceVariantSignature>,
}

impl SourceEnumBody {
    pub fn variants(&self) -> &[SourceVariantSignature] {
        &self.variants
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::SquareBracket, "source enum body")?;
        Self::from_blocks(body.root_objects())
    }

    fn from_blocks(blocks: &[Block]) -> Result<Self, SchemaError> {
        let mut variants = Vec::new();
        for block in blocks {
            variants.push(
                SourceVariantSignature::from_structural_block(block).map_err(SchemaError::from)?,
            );
        }
        Ok(Self { variants })
    }

    fn to_schema_text(&self) -> String {
        Delimiter::SquareBracket.wrap(
            self.variants
                .iter()
                .map(SourceVariantSignature::to_structural_nota),
        )
    }

    fn to_declaration_group(
        &self,
        name: Name,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceDeclarationGroup, SchemaError> {
        let mut private = Vec::new();
        for variant in &self.variants {
            private.extend(variant.private_inline_declarations(resolver)?);
        }
        Ok(SourceDeclarationGroup::new(
            private,
            TypeDeclaration::Enum(
                self.to_schema_enum(name, &SourceVariantPayloadResolution::explicit_only())?,
            ),
        ))
    }

    fn public_inline_declarations(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<Vec<Declaration>, SchemaError> {
        let mut declarations = Vec::new();
        for variant in &self.variants {
            let group = variant.public_inline_declaration(resolver)?;
            declarations.extend(group.into_public_declarations());
        }
        Ok(declarations)
    }

    fn inline_declaration_names(&self) -> Vec<Name> {
        self.variants
            .iter()
            .filter_map(SourceVariantSignature::inline_declaration_name)
            .collect()
    }

    fn to_schema_enum(
        &self,
        name: Name,
        resolver: &impl SourceVariantResolver,
    ) -> Result<EnumDeclaration, SchemaError> {
        let variants = self
            .variants
            .iter()
            .map(|variant| variant.to_enum_variant(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EnumDeclaration::new(name, variants))
    }
}

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
pub enum SourceVariantSignature {
    #[shape(pascal_atom)]
    Unit(SourceVariantName),
    #[shape(pascal_head, arity = 2)]
    Data(SourceVariantName, #[rkyv(omit_bounds)] SourceVariantPayload),
    #[shape(pascal_head, arity = 4)]
    Streaming(
        SourceVariantName,
        #[rkyv(omit_bounds)] SourceVariantPayload,
        StreamRelationKeyword,
        SourceVariantName,
    ),
}

impl SourceVariantSignature {
    pub fn name(&self) -> &Name {
        match self {
            Self::Unit(name) | Self::Data(name, _) | Self::Streaming(name, ..) => name.name(),
        }
    }

    pub fn payload(&self) -> Option<&SourceReference> {
        match self.payload_value() {
            Some(SourceVariantPayload::Reference(reference)) => Some(reference),
            Some(SourceVariantPayload::Declaration(_)) | None => None,
        }
    }

    pub fn stream_relation(&self) -> Option<StreamRelation> {
        match self {
            Self::Streaming(_, _, keyword, stream_name) => {
                Some(keyword.into_stream_relation(stream_name.name().clone()))
            }
            Self::Unit(_) | Self::Data(_, _) => None,
        }
    }

    fn payload_value(&self) -> Option<&SourceVariantPayload> {
        match self {
            Self::Data(_, payload) | Self::Streaming(_, payload, _, _) => Some(payload),
            Self::Unit(_) => None,
        }
    }

    fn to_enum_variant(
        &self,
        resolver: &impl SourceVariantResolver,
    ) -> Result<EnumVariant, SchemaError> {
        let name = self.name().clone();
        let payload = match self.payload_value() {
            Some(SourceVariantPayload::Reference(reference)) => Some(reference.to_type_reference()),
            Some(SourceVariantPayload::Declaration(_)) => {
                Some(TypeReference::from_name(name.clone()))
            }
            None if resolver.resolves_variant_payload(&name) => {
                Some(TypeReference::from_name(name.clone()))
            }
            None => None,
        };
        let variant = EnumVariant::new(name, payload);
        Ok(match self.stream_relation() {
            Some(relation) => variant.with_stream_relation(relation),
            None => variant,
        })
    }

    fn public_inline_declaration(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<SourceDeclarationGroup, SchemaError> {
        match self.payload_value() {
            Some(SourceVariantPayload::Declaration(value)) => {
                value.to_declaration_group(self.name().clone(), resolver)
            }
            Some(SourceVariantPayload::Reference(_)) | None => Ok(SourceDeclarationGroup::empty()),
        }
    }

    fn private_inline_declarations(
        &self,
        resolver: &SourceTypeResolver,
    ) -> Result<Vec<TypeDeclaration>, SchemaError> {
        match self.payload_value() {
            Some(SourceVariantPayload::Declaration(value)) => Ok(value
                .to_declaration_group(self.name().clone(), resolver)?
                .into_type_declarations()),
            Some(SourceVariantPayload::Reference(_)) | None => Ok(Vec::new()),
        }
    }

    fn inline_declaration_name(&self) -> Option<Name> {
        match self.payload_value() {
            Some(SourceVariantPayload::Declaration(_)) => Some(self.name().clone()),
            Some(SourceVariantPayload::Reference(_)) | None => None,
        }
    }
}

/// A PascalCase schema symbol at a variant-name or stream-name position. It owns
/// the lowered `Name` and decodes itself from a bare PascalCase atom, so the
/// `SourceVariantSignature` derive can recurse into each name field.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SourceVariantName(Name);

impl SourceVariantName {
    fn name(&self) -> &Name {
        &self.0
    }

    fn qualifies(value: &str) -> bool {
        value
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
            && !value.contains('@')
    }
}

impl StructuralMacroNode for SourceVariantName {
    type Error = SchemaError;

    fn structural_position() -> nota_next::PositionPredicate {
        nota_next::PositionPredicate::named("variant name")
    }

    fn structural_variants() -> Vec<StructuralVariant> {
        vec![
            nota_next::BlockShape::pascal_atom(Some(CaptureName::new("name")))
                .into_structural_variant("symbol", "PascalCase atom"),
        ]
    }

    fn from_structural_block(block: &Block) -> Result<Self, StructuralMacroError<Self::Error>> {
        let Some(text) = block.demote_to_string() else {
            return Err(StructuralMacroError::MatchedNode(
                SchemaError::ExpectedSymbol {
                    found: block.reemit_fallback(),
                },
            ));
        };
        if !Self::qualifies(text) {
            return Err(StructuralMacroError::MatchedNode(
                SchemaError::ExpectedSyntaxEnumVariant {
                    found: block.reemit_fallback(),
                },
            ));
        }
        Ok(Self(Name::new(text)))
    }

    fn from_structural_candidate(
        candidate: MacroCandidate<'_>,
    ) -> Result<Self, StructuralMacroError<Self::Error>> {
        match candidate.blocks() {
            [block] => Self::from_structural_block(block),
            blocks => Err(StructuralMacroError::ExpectedSingleRoot {
                found: blocks.len(),
            }),
        }
    }

    fn to_structural_nota(&self) -> String {
        self.0.to_nota()
    }
}

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
pub enum SourceVariantPayload {
    Reference(SourceReference),
    Declaration(#[rkyv(omit_bounds)] SourceDeclarationValue),
}

impl SourceVariantPayload {
    fn to_schema_text(&self) -> String {
        match self {
            Self::Reference(reference) => reference.to_schema_text(),
            Self::Declaration(value) => value.to_schema_text(),
        }
    }
}

impl StructuralMacroNode for SourceVariantPayload {
    type Error = SchemaError;

    fn structural_position() -> nota_next::PositionPredicate {
        nota_next::PositionPredicate::named("variant payload")
    }

    fn structural_variants() -> Vec<StructuralVariant> {
        Vec::new()
    }

    fn from_structural_block(block: &Block) -> Result<Self, StructuralMacroError<Self::Error>> {
        let decoded = match SourceReference::from_block(block) {
            Ok(reference) => Self::Reference(reference),
            Err(_) => SourceDeclarationValue::from_block(block)
                .map(Self::Declaration)
                .map_err(StructuralMacroError::MatchedNode)?,
        };
        Ok(decoded)
    }

    fn from_structural_candidate(
        candidate: MacroCandidate<'_>,
    ) -> Result<Self, StructuralMacroError<Self::Error>> {
        match candidate.blocks() {
            [block] => Self::from_structural_block(block),
            blocks => Err(StructuralMacroError::ExpectedSingleRoot {
                found: blocks.len(),
            }),
        }
    }

    fn to_structural_nota(&self) -> String {
        self.to_schema_text()
    }
}

/// The `opens` / `belongs` discriminator that precedes a stream name in a
/// streaming variant signature. It is a keyword structural macro node so the
/// `SourceVariantSignature` derive decodes the marker recursively rather than
/// matching a literal string by hand.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::StructuralMacroNode,
    Clone,
    Copy,
    Debug,
    Eq,
    PartialEq,
)]
pub enum StreamRelationKeyword {
    #[shape(keyword = "opens")]
    Opens,
    #[shape(keyword = "belongs")]
    Belongs,
}

impl StreamRelationKeyword {
    fn into_stream_relation(self, stream_name: Name) -> StreamRelation {
        match self {
            Self::Opens => StreamRelation::Opens(stream_name),
            Self::Belongs => StreamRelation::Belongs(stream_name),
        }
    }
}

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
pub enum SourceReference {
    Plain(Name),
    Vector(#[rkyv(omit_bounds)] Box<SourceReference>),
    Optional(#[rkyv(omit_bounds)] Box<SourceReference>),
    Map(
        #[rkyv(omit_bounds)] Box<SourceReference>,
        #[rkyv(omit_bounds)] Box<SourceReference>,
    ),
}

impl SourceReference {
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        Self::from_raw(&RawNotaDatatype::from_block(block)?)
    }

    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        match raw {
            RawNotaDatatype::Atom(name) => Ok(Self::Plain(Name::new(name))),
            RawNotaDatatype::Record(sequence) => Self::from_record(sequence),
            RawNotaDatatype::Vector(_)
            | RawNotaDatatype::KeyValue(_)
            | RawNotaDatatype::PipeBrace(_)
            | RawNotaDatatype::PipeParenthesis(_)
            | RawNotaDatatype::Text(_) => Err(SchemaError::ExpectedSyntaxReference {
                found: SourceRawNotation::new(raw).description(),
            }),
        }
    }

    fn from_record(sequence: &RawNotaSequence) -> Result<Self, SchemaError> {
        let items = sequence.items();
        if items.len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "typed reference record",
                expected: "tag plus one grouped payload object",
                found: items.len(),
            });
        }
        let Some(head) = items[0].as_atom() else {
            return Err(SchemaError::ExpectedSymbol {
                found: SourceRawNotation::new(&items[0]).description(),
            });
        };
        match head {
            "Vec" | "Vector" => Ok(Self::Vector(Box::new(Self::from_raw(&items[1])?))),
            "Optional" | "Option" => Ok(Self::Optional(Box::new(Self::from_raw(&items[1])?))),
            "Map" | "KeyValue" => Self::from_map_record(&items[1]),
            _ => Err(SchemaError::ExpectedSyntaxReference {
                found: SourceSequenceNotation::new(sequence).description(),
            }),
        }
    }

    fn from_map_record(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        let Some(sequence) = raw.as_record() else {
            return Err(SchemaError::ExpectedSyntaxReference {
                found: SourceRawNotation::new(raw).description(),
            });
        };
        if sequence.items().len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "map reference payload",
                expected: "key type plus value type",
                found: sequence.items().len(),
            });
        }
        Ok(Self::Map(
            Box::new(Self::from_raw(&sequence.items()[0])?),
            Box::new(Self::from_raw(&sequence.items()[1])?),
        ))
    }

    fn to_schema_text(&self) -> String {
        match self {
            Self::Plain(name) => name.to_nota(),
            Self::Vector(reference) => {
                Delimiter::Parenthesis.wrap(["Vec".to_owned(), reference.to_schema_text()])
            }
            Self::Optional(reference) => {
                Delimiter::Parenthesis.wrap(["Optional".to_owned(), reference.to_schema_text()])
            }
            Self::Map(key, value) => Delimiter::Parenthesis.wrap([
                "Map".to_owned(),
                Delimiter::Parenthesis.wrap([key.to_schema_text(), value.to_schema_text()]),
            ]),
        }
    }

    fn to_type_reference(&self) -> TypeReference {
        match self {
            Self::Plain(name) => TypeReference::from_name(name.clone()),
            Self::Vector(reference) => {
                TypeReference::Vector(Box::new(reference.to_type_reference()))
            }
            Self::Optional(reference) => {
                TypeReference::Optional(Box::new(reference.to_type_reference()))
            }
            Self::Map(key, value) => TypeReference::Map(
                Box::new(key.to_type_reference()),
                Box::new(value.to_type_reference()),
            ),
        }
    }
}

trait SourceVariantResolver {
    fn resolves_variant_payload(&self, name: &Name) -> bool;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceVariantPayloadResolution {
    resolves_bare_names: bool,
}

impl SourceVariantPayloadResolution {
    fn explicit_only() -> Self {
        Self {
            resolves_bare_names: false,
        }
    }
}

impl SourceVariantResolver for SourceVariantPayloadResolution {
    fn resolves_variant_payload(&self, _name: &Name) -> bool {
        self.resolves_bare_names
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceTypeResolver {
    names: Vec<Name>,
}

impl SourceTypeResolver {
    fn from_source(source: &SchemaSource) -> Self {
        let mut names = source
            .namespace()
            .entries()
            .iter()
            .filter(|entry| entry.is_type_declaration())
            .map(|entry| entry.name().clone())
            .collect::<Vec<_>>();
        names.extend(source.input().body().inline_declaration_names());
        names.extend(source.output().body().inline_declaration_names());
        Self { names }
    }

    fn contains(&self, name: &Name) -> bool {
        self.names.iter().any(|candidate| candidate == name)
    }
}

impl SourceVariantResolver for SourceTypeResolver {
    fn resolves_variant_payload(&self, name: &Name) -> bool {
        self.contains(name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLoweredNamespace {
    declarations: Vec<Declaration>,
}

impl SourceLoweredNamespace {
    fn from_source(
        source: &SourceNamespace,
        resolver: &SourceTypeResolver,
    ) -> Result<Self, SchemaError> {
        let mut namespace = Self {
            declarations: Vec::new(),
        };
        for entry in source.entries() {
            namespace.push_public_group(entry.to_declaration_group(resolver)?)?;
        }
        Ok(namespace)
    }

    fn push_public_group(&mut self, group: SourceDeclarationGroup) -> Result<(), SchemaError> {
        self.push_public_declarations(group.into_public_declarations())
    }

    fn push_public_declarations(
        &mut self,
        declarations: Vec<Declaration>,
    ) -> Result<(), SchemaError> {
        for declaration in declarations {
            self.push_declaration(declaration)?;
        }
        Ok(())
    }

    fn push_declaration(&mut self, declaration: Declaration) -> Result<(), SchemaError> {
        if self
            .declarations
            .iter()
            .any(|existing| existing.name() == declaration.name())
        {
            return Err(SchemaError::DuplicateSourceDeclaration {
                name: declaration.name().as_str().to_owned(),
            });
        }
        self.declarations.push(declaration);
        Ok(())
    }

    fn into_declarations(self) -> Vec<Declaration> {
        self.declarations
    }
}

impl SourceVariantResolver for SourceLoweredNamespace {
    fn resolves_variant_payload(&self, name: &Name) -> bool {
        self.declarations
            .iter()
            .any(|declaration| declaration.name() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceDeclarationGroup {
    private: Vec<TypeDeclaration>,
    primary: Option<TypeDeclaration>,
}

impl SourceDeclarationGroup {
    fn empty() -> Self {
        Self {
            private: Vec::new(),
            primary: None,
        }
    }

    fn primary(primary: TypeDeclaration) -> Self {
        Self {
            private: Vec::new(),
            primary: Some(primary),
        }
    }

    fn new(private: Vec<TypeDeclaration>, primary: TypeDeclaration) -> Self {
        Self {
            private,
            primary: Some(primary),
        }
    }

    fn into_public_declarations(self) -> Vec<Declaration> {
        let mut declarations = self
            .private
            .into_iter()
            .map(Declaration::private)
            .collect::<Vec<_>>();
        if let Some(primary) = self.primary {
            declarations.push(Declaration::public(primary));
        }
        declarations
    }

    fn into_type_declarations(self) -> Vec<TypeDeclaration> {
        let mut declarations = self.private;
        if let Some(primary) = self.primary {
            declarations.push(primary);
        }
        declarations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceLoweredField {
    private_declarations: Vec<TypeDeclaration>,
    field: FieldDeclaration,
}

impl SourceLoweredField {
    fn new(private_declarations: Vec<TypeDeclaration>, field: FieldDeclaration) -> Self {
        Self {
            private_declarations,
            field,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceIdentifierCase<'name>(&'name Name);

impl<'name> SourceIdentifierCase<'name> {
    fn new(name: &'name Name) -> Self {
        Self(name)
    }

    fn is_type(&self) -> bool {
        self.0
            .as_str()
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceAtom<'source>(&'source str);

impl<'source> SourceAtom<'source> {
    fn from_block(block: &'source Block) -> Result<Self, SchemaError> {
        let Block::Atom(atom) = block else {
            return Err(SchemaError::ExpectedSymbol {
                found: SourceBlockNotation::new(block).description(),
            });
        };
        Ok(Self(atom.text()))
    }

    fn into_name(self) -> Name {
        Name::new(self.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceBlockNotation<'source>(&'source Block);

impl<'source> SourceBlockNotation<'source> {
    fn new(block: &'source Block) -> Self {
        Self(block)
    }

    fn description(&self) -> String {
        match self.0 {
            Block::Delimited { delimiter, .. } => {
                format!("{} block", delimiter.description())
            }
            Block::PipeText(_) => "pipe text".to_owned(),
            Block::Atom(atom) => format!("atom {}", atom.text()),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceRawNotation<'source>(&'source RawNotaDatatype);

impl<'source> SourceRawNotation<'source> {
    fn new(raw: &'source RawNotaDatatype) -> Self {
        Self(raw)
    }

    fn description(&self) -> String {
        match self.0 {
            RawNotaDatatype::Atom(text) => format!("atom {text}"),
            RawNotaDatatype::Text(_) => "text".to_owned(),
            RawNotaDatatype::Record(_) => "parenthesis record".to_owned(),
            RawNotaDatatype::Vector(_) => "square-bracket vector".to_owned(),
            RawNotaDatatype::KeyValue(_) => "brace key-value map".to_owned(),
            RawNotaDatatype::PipeParenthesis(_) => "pipe-parenthesis declaration".to_owned(),
            RawNotaDatatype::PipeBrace(_) => "pipe-brace declaration".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceSequenceNotation<'source>(&'source RawNotaSequence);

impl<'source> SourceSequenceNotation<'source> {
    fn new(sequence: &'source RawNotaSequence) -> Self {
        Self(sequence)
    }

    fn description(&self) -> String {
        format!("parenthesis record with {} objects", self.0.items().len())
    }
}
