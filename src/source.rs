use std::{
    fs,
    path::{Path, PathBuf},
};

use nota_next::{Block, Delimiter, Document, NotaBody, NotaEncode, NotaString};

use crate::{
    Name, RawDatatypeMap, RawNotaDatatype, RawNotaSequence, SchemaEngine, SchemaError,
    SchemaIdentity,
};

#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub fn to_schema_text(&self) -> String {
        [
            self.imports.to_schema_text(),
            self.input.body().to_schema_text(),
            self.output.body().to_schema_text(),
            self.namespace.to_schema_text(),
        ]
        .join("\n")
    }

    pub fn lower(
        &self,
        engine: &SchemaEngine,
        identity: SchemaIdentity,
    ) -> Result<crate::Asschema, SchemaError> {
        engine.lower_schema_source(self, identity)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceNamespace {
    entries: Vec<SourceNamespaceEntry>,
}

impl SourceNamespace {
    pub fn entries(&self) -> &[SourceNamespaceEntry] {
        &self.entries
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::Brace, "source namespace")?;
        let map = RawDatatypeMap::from_blocks(body.root_objects())?;
        let mut entries = Vec::new();
        for entry in map.entries() {
            entries.push(SourceNamespaceEntry {
                name: entry.name().clone(),
                value: SourceDeclarationValue::from_raw(entry.datatype())?,
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceDeclarationValue {
    Reference(SourceReference),
    Text(String),
    Struct(SourceStructBody),
    Enum(SourceEnumBody),
}

impl SourceDeclarationValue {
    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        match raw {
            RawNotaDatatype::Atom(_) | RawNotaDatatype::Record(_) => {
                Ok(Self::Reference(SourceReference::from_raw(raw)?))
            }
            RawNotaDatatype::Text(text) => Ok(Self::Text(text.clone())),
            RawNotaDatatype::KeyValue(map) => Ok(Self::Struct(SourceStructBody::from_map(map)?)),
            RawNotaDatatype::Vector(sequence) => {
                Ok(Self::Enum(SourceEnumBody::from_sequence(sequence)?))
            }
            RawNotaDatatype::PipeParenthesis(_) | RawNotaDatatype::PipeBrace(_) => {
                Err(SchemaError::ExpectedSyntaxDeclaration {
                    found: SourceRawNotation::new(raw).description(),
                })
            }
        }
    }

    fn to_schema_text(&self) -> String {
        match self {
            Self::Reference(reference) => reference.to_schema_text(),
            Self::Text(text) => NotaString::new(text).format(),
            Self::Struct(body) => body.to_schema_text(),
            Self::Enum(body) => body.to_schema_text(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceStructBody {
    fields: Vec<SourceField>,
}

impl SourceStructBody {
    pub fn fields(&self) -> &[SourceField] {
        &self.fields
    }

    fn from_map(map: &RawDatatypeMap) -> Result<Self, SchemaError> {
        let mut fields = Vec::new();
        for entry in map.entries() {
            fields.push(SourceField {
                name: entry.name().clone(),
                value: SourceFieldValue::from_raw(entry.datatype())?,
            });
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

#[derive(Clone, Debug, Eq, PartialEq)]
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceFieldValue {
    Derived,
    Reference(SourceReference),
}

impl SourceFieldValue {
    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        if raw.as_atom() == Some("*") {
            return Ok(Self::Derived);
        }
        SourceReference::from_raw(raw).map(Self::Reference)
    }

    fn to_schema_text(&self) -> String {
        match self {
            Self::Derived => "*".to_owned(),
            Self::Reference(reference) => reference.to_schema_text(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceEnumBody {
    variants: Vec<SourceVariantSignature>,
}

impl SourceEnumBody {
    pub fn variants(&self) -> &[SourceVariantSignature] {
        &self.variants
    }

    fn from_block(block: &Block) -> Result<Self, SchemaError> {
        let body = NotaBody::from_delimited(block, Delimiter::SquareBracket, "source enum body")?;
        let sequence = RawNotaSequence::from_blocks(body.root_objects())?;
        Self::from_sequence(&sequence)
    }

    fn from_sequence(sequence: &RawNotaSequence) -> Result<Self, SchemaError> {
        let mut variants = Vec::new();
        for item in sequence.items() {
            variants.push(SourceVariantSignature::from_raw(item)?);
        }
        Ok(Self { variants })
    }

    fn to_schema_text(&self) -> String {
        Delimiter::SquareBracket.wrap(
            self.variants
                .iter()
                .map(SourceVariantSignature::to_schema_text),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceVariantSignature {
    name: Name,
    payload: Option<SourceReference>,
}

impl SourceVariantSignature {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> Option<&SourceReference> {
        self.payload.as_ref()
    }

    fn from_raw(raw: &RawNotaDatatype) -> Result<Self, SchemaError> {
        if let Some(name) = raw.as_atom() {
            if SourceVariantName::new(name).is_valid() {
                return Ok(Self {
                    name: Name::new(name),
                    payload: None,
                });
            }
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: SourceRawNotation::new(raw).description(),
            });
        }

        let Some(sequence) = raw.as_record() else {
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: SourceRawNotation::new(raw).description(),
            });
        };
        if sequence.items().len() != 2 {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form: "data-carrying enum variant",
                expected: "tag plus one payload object",
                found: sequence.items().len(),
            });
        }
        let Some(name) = sequence.items()[0].as_atom() else {
            return Err(SchemaError::ExpectedSymbol {
                found: SourceRawNotation::new(&sequence.items()[0]).description(),
            });
        };
        if !SourceVariantName::new(name).is_valid() {
            return Err(SchemaError::ExpectedSyntaxEnumVariant {
                found: SourceRawNotation::new(&sequence.items()[0]).description(),
            });
        }
        Ok(Self {
            name: Name::new(name),
            payload: Some(SourceReference::from_raw(&sequence.items()[1])?),
        })
    }

    fn to_schema_text(&self) -> String {
        match &self.payload {
            Some(payload) => {
                Delimiter::Parenthesis.wrap([self.name.to_nota(), payload.to_schema_text()])
            }
            None => self.name.to_nota(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct SourceVariantName<'source>(&'source str);

impl<'source> SourceVariantName<'source> {
    fn new(value: &'source str) -> Self {
        Self(value)
    }

    fn is_valid(&self) -> bool {
        self.0
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
            && !self.0.contains('@')
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceReference {
    Plain(Name),
    Vector(Box<SourceReference>),
    Optional(Box<SourceReference>),
    Map(Box<SourceReference>, Box<SourceReference>),
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
