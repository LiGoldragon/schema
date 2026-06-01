use std::{
    fs,
    path::{Path, PathBuf},
};

use nota_next::{AtomClassification, Block, Delimiter, Document, NotaEncode, NotaSource};

use crate::{
    Declaration, EnumDeclaration, EnumVariant, FieldDeclaration, MacroContext, MacroObject,
    MacroOutput, MacroPair, MacroPosition, MacroRegistry, Name, NewtypeDeclaration, SchemaError,
    SchemaMacro, StructDeclaration, TypeDeclaration, TypeReference, macros::SchemaBlockExt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarativeMacroLibrary {
    source_entries: Vec<MacroLibrarySourceEntry>,
}

impl DeclarativeMacroLibrary {
    pub fn builtin() -> Result<Self, SchemaError> {
        Ok(Self::from_data(MacroLibraryData::from_nota_source(
            include_str!("../schemas/builtin-macros.macro-library"),
        )?))
    }

    pub fn builtin_source() -> Result<Self, SchemaError> {
        Self::from_source(include_str!("../schemas/builtin-macros.schema"))
    }

    pub fn from_source(source: &str) -> Result<Self, SchemaError> {
        let document = Document::parse(source)?;
        let mut source_entries = Vec::new();
        for object in document.root_objects() {
            source_entries.push(MacroLibrarySourceEntry::from_block(object)?);
        }
        Ok(Self { source_entries })
    }

    pub fn from_data(data: MacroLibraryData) -> Self {
        Self {
            source_entries: data
                .into_source_entries()
                .into_iter()
                .map(MacroLibrarySourceEntry::from_data)
                .collect(),
        }
    }

    pub fn to_data(&self) -> MacroLibraryData {
        MacroLibraryData::new(
            self.source_entries
                .iter()
                .map(MacroLibrarySourceEntry::to_data)
                .collect(),
        )
    }

    pub fn source_entries(&self) -> &[MacroLibrarySourceEntry] {
        &self.source_entries
    }

    pub fn definitions(&self) -> Vec<&MacroDefinition> {
        self.source_entries
            .iter()
            .map(MacroLibrarySourceEntry::definition)
            .collect()
    }

    pub fn into_macros(self) -> Vec<Box<dyn SchemaMacro>> {
        self.source_entries
            .into_iter()
            .map(MacroLibrarySourceEntry::into_schema_macro)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroLibraryArtifact {
    data: MacroLibraryData,
}

impl MacroLibraryArtifact {
    pub fn new(data: MacroLibraryData) -> Self {
        Self { data }
    }

    pub fn data(&self) -> &MacroLibraryData {
        &self.data
    }

    pub fn into_data(self) -> MacroLibraryData {
        self.data
    }

    pub fn from_nota_source(source: &str) -> Result<Self, SchemaError> {
        MacroLibraryData::from_nota_source(source).map(Self::new)
    }

    pub fn to_nota_source(&self) -> String {
        self.data.to_nota_source()
    }

    pub fn from_binary_bytes(bytes: &[u8]) -> Result<Self, SchemaError> {
        MacroLibraryData::from_binary_bytes(bytes).map(Self::new)
    }

    pub fn to_binary_bytes(&self) -> Result<Vec<u8>, SchemaError> {
        self.data.to_binary_bytes()
    }

    pub fn read_nota_file(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let artifact_path = MacroLibraryArtifactPath::new(path.as_ref());
        let source = fs::read_to_string(artifact_path.path())
            .map_err(|error| artifact_path.io_error(error))?;
        Self::from_nota_source(&source)
    }

    pub fn write_nota_file(&self, path: impl AsRef<Path>) -> Result<(), SchemaError> {
        let artifact_path = MacroLibraryArtifactPath::new(path.as_ref());
        fs::write(artifact_path.path(), self.to_nota_source())
            .map_err(|error| artifact_path.io_error(error))
    }

    pub fn read_binary_file(path: impl AsRef<Path>) -> Result<Self, SchemaError> {
        let artifact_path = MacroLibraryArtifactPath::new(path.as_ref());
        let bytes =
            fs::read(artifact_path.path()).map_err(|error| artifact_path.io_error(error))?;
        Self::from_binary_bytes(&bytes)
    }

    pub fn write_binary_file(&self, path: impl AsRef<Path>) -> Result<(), SchemaError> {
        let artifact_path = MacroLibraryArtifactPath::new(path.as_ref());
        let bytes = self.to_binary_bytes()?;
        fs::write(artifact_path.path(), bytes).map_err(|error| artifact_path.io_error(error))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MacroLibraryArtifactPath {
    path: PathBuf,
}

impl MacroLibraryArtifactPath {
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
pub struct MacroLibraryData {
    source_entries: Vec<MacroLibrarySourceEntryData>,
}

impl MacroLibraryData {
    pub fn new(source_entries: Vec<MacroLibrarySourceEntryData>) -> Self {
        Self { source_entries }
    }

    pub fn source_entries(&self) -> &[MacroLibrarySourceEntryData] {
        &self.source_entries
    }

    pub fn definitions(&self) -> Vec<&MacroDefinitionData> {
        self.source_entries
            .iter()
            .map(MacroLibrarySourceEntryData::definition)
            .collect()
    }

    pub fn into_source_entries(self) -> Vec<MacroLibrarySourceEntryData> {
        self.source_entries
    }

    pub fn from_nota_source(source: &str) -> Result<Self, SchemaError> {
        NotaSource::new(source).parse::<Self>().map_err(Into::into)
    }

    pub fn to_nota_source(&self) -> String {
        NotaEncode::to_nota(self)
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
pub enum MacroLibrarySourceEntryData {
    SchemaMacro(MacroDefinitionData),
}

impl MacroLibrarySourceEntryData {
    pub fn definition(&self) -> &MacroDefinitionData {
        match self {
            Self::SchemaMacro(definition) => definition,
        }
    }

    pub fn into_definition(self) -> MacroDefinitionData {
        match self {
            Self::SchemaMacro(definition) => definition,
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::SchemaMacro(_) => "SchemaMacro",
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
pub struct MacroDefinitionData {
    name: Name,
    position: MacroPosition,
    pattern: MacroPatternData,
    template: MacroTemplateData,
}

impl MacroDefinitionData {
    pub fn new(
        name: Name,
        position: MacroPosition,
        pattern: MacroPatternData,
        template: MacroTemplateData,
    ) -> Self {
        Self {
            name,
            position,
            pattern,
            template,
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn position(&self) -> MacroPosition {
        self.position
    }

    pub fn pattern(&self) -> &MacroPatternData {
        &self.pattern
    }

    pub fn template(&self) -> &MacroTemplateData {
        &self.template
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
pub struct MacroPatternData {
    object: MacroPatternObjectData,
}

impl MacroPatternData {
    pub fn new(object: MacroPatternObjectData) -> Self {
        Self { object }
    }

    pub fn object(&self) -> &MacroPatternObjectData {
        &self.object
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
pub enum MacroPatternObjectData {
    Capture(String),
    RestCapture(String),
    Atom(String),
    Delimited(#[rkyv(omit_bounds)] Box<MacroPatternDelimitedData>),
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
pub struct MacroPatternDelimitedData {
    delimiter: MacroDelimiter,
    #[rkyv(omit_bounds)]
    children: Vec<MacroPatternObjectData>,
}

impl MacroPatternDelimitedData {
    pub fn new(delimiter: MacroDelimiter, children: Vec<MacroPatternObjectData>) -> Self {
        Self {
            delimiter,
            children,
        }
    }

    pub fn delimiter(&self) -> MacroDelimiter {
        self.delimiter
    }

    pub fn children(&self) -> &[MacroPatternObjectData] {
        &self.children
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
pub struct MacroTemplateData {
    object: MacroTemplateObjectData,
}

impl MacroTemplateData {
    pub fn new(object: MacroTemplateObjectData) -> Self {
        Self { object }
    }

    pub fn object(&self) -> &MacroTemplateObjectData {
        &self.object
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
pub enum MacroTemplateObjectData {
    Capture(String),
    RestCapture(String),
    Atom(String),
    Delimited(#[rkyv(omit_bounds)] Box<MacroTemplateDelimitedData>),
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
pub struct MacroTemplateDelimitedData {
    delimiter: MacroDelimiter,
    #[rkyv(omit_bounds)]
    children: Vec<MacroTemplateObjectData>,
}

impl MacroTemplateDelimitedData {
    pub fn new(delimiter: MacroDelimiter, children: Vec<MacroTemplateObjectData>) -> Self {
        Self {
            delimiter,
            children,
        }
    }

    pub fn delimiter(&self) -> MacroDelimiter {
        self.delimiter
    }

    pub fn children(&self) -> &[MacroTemplateObjectData] {
        &self.children
    }
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
pub enum MacroDelimiter {
    Parenthesis,
    SquareBracket,
    Brace,
    PipeParenthesis,
    PipeBrace,
}

impl MacroDelimiter {
    fn from_nota(delimiter: Delimiter) -> Self {
        match delimiter {
            Delimiter::Parenthesis => Self::Parenthesis,
            Delimiter::SquareBracket => Self::SquareBracket,
            Delimiter::Brace => Self::Brace,
            Delimiter::PipeParenthesis => Self::PipeParenthesis,
            Delimiter::PipeBrace => Self::PipeBrace,
        }
    }

    fn into_nota(self) -> Delimiter {
        match self {
            Self::Parenthesis => Delimiter::Parenthesis,
            Self::SquareBracket => Delimiter::SquareBracket,
            Self::Brace => Delimiter::Brace,
            Self::PipeParenthesis => Delimiter::PipeParenthesis,
            Self::PipeBrace => Delimiter::PipeBrace,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroLibrarySourceEntry {
    SchemaMacro(MacroDefinition),
}

impl MacroLibrarySourceEntry {
    pub fn from_block(object: &Block) -> Result<Self, SchemaError> {
        let record = MacroLibrarySourceEntryRecord::new(object)?;
        match record.variant_name()?.as_str() {
            "SchemaMacro" => Ok(Self::SchemaMacro(MacroDefinition::from_record(record)?)),
            _ => Err(record.expected_source_entry_error()),
        }
    }

    fn from_data(data: MacroLibrarySourceEntryData) -> Self {
        match data {
            MacroLibrarySourceEntryData::SchemaMacro(definition) => {
                Self::SchemaMacro(MacroDefinition::from_data(definition))
            }
        }
    }

    fn to_data(&self) -> MacroLibrarySourceEntryData {
        match self {
            Self::SchemaMacro(definition) => {
                MacroLibrarySourceEntryData::SchemaMacro(definition.to_data())
            }
        }
    }

    pub fn definition(&self) -> &MacroDefinition {
        match self {
            Self::SchemaMacro(definition) => definition,
        }
    }

    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::SchemaMacro(_) => "SchemaMacro",
        }
    }

    fn into_schema_macro(self) -> Box<dyn SchemaMacro> {
        match self {
            Self::SchemaMacro(definition) => {
                Box::new(DeclarativeSchemaMacro { definition }) as Box<dyn SchemaMacro>
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroDefinition {
    name: Name,
    position: MacroPosition,
    pattern: MacroPattern,
    template: MacroTemplate,
}

impl MacroDefinition {
    pub fn from_block(object: &Block) -> Result<Self, SchemaError> {
        match MacroLibrarySourceEntry::from_block(object)? {
            MacroLibrarySourceEntry::SchemaMacro(definition) => Ok(definition),
        }
    }

    fn from_record(record: MacroLibrarySourceEntryRecord<'_>) -> Result<Self, SchemaError> {
        Ok(Self {
            name: record.name()?,
            position: record.position()?,
            pattern: record.pattern()?,
            template: record.template()?,
        })
    }

    fn from_data(data: MacroDefinitionData) -> Self {
        Self {
            name: data.name,
            position: data.position,
            pattern: MacroPattern::from_data(data.pattern),
            template: MacroTemplate::from_data(data.template),
        }
    }

    fn to_data(&self) -> MacroDefinitionData {
        MacroDefinitionData::new(
            self.name.clone(),
            self.position,
            self.pattern.to_data(),
            self.template.to_data(),
        )
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn position(&self) -> MacroPosition {
        self.position
    }

    pub fn capture_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        for name in self.pattern.capture_names() {
            if !names.contains(&name) {
                names.push(name);
            }
        }
        names
    }
}

#[derive(Clone, Copy, Debug)]
struct MacroLibrarySourceEntryRecord<'schema> {
    object: &'schema Block,
}

impl<'schema> MacroLibrarySourceEntryRecord<'schema> {
    fn new(object: &'schema Block) -> Result<Self, SchemaError> {
        let record = Self { object };
        if !object.is_parenthesis() || object.holds_root_objects() != 5 {
            return Err(SchemaError::ExpectedMacroDefinition {
                found: NotationBlock::new(object).compact_notation(),
            });
        }
        Ok(record)
    }

    fn child(&self, index: usize) -> &'schema Block {
        self.object
            .root_object_at(index)
            .expect("macro definition shape checked")
    }

    fn variant_name(&self) -> Result<Name, SchemaError> {
        self.child(0).schema_name()
    }

    fn name(&self) -> Result<Name, SchemaError> {
        self.child(1).schema_name()
    }

    fn position(&self) -> Result<MacroPosition, SchemaError> {
        MacroPosition::from_name(&self.child(2).schema_name()?)
    }

    fn pattern(&self) -> Result<MacroPattern, SchemaError> {
        MacroPattern::from_block(self.child(3))
    }

    fn template(&self) -> Result<MacroTemplate, SchemaError> {
        MacroTemplate::from_block(self.child(4))
    }

    fn expected_source_entry_error(&self) -> SchemaError {
        SchemaError::ExpectedMacroDefinition {
            found: NotationBlock::new(self.object).compact_notation(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MacroPattern {
    object: PatternObject,
}

impl MacroPattern {
    fn from_block(object: &Block) -> Result<Self, SchemaError> {
        Ok(Self {
            object: PatternObject::from_block(object)?,
        })
    }

    fn from_data(data: MacroPatternData) -> Self {
        Self {
            object: PatternObject::from_data(data.object),
        }
    }

    fn to_data(&self) -> MacroPatternData {
        MacroPatternData::new(self.object.to_data())
    }

    fn captures(&self, object: MacroObject<'_>) -> Result<Option<MacroBindings>, SchemaError> {
        let mut bindings = MacroBindings::default();
        let matched = match object {
            MacroObject::Block(block) => self.object.matches_block(block, &mut bindings)?,
            MacroObject::Pair(pair) => self.object.matches_pair(pair, &mut bindings)?,
        };
        if matched {
            Ok(Some(bindings))
        } else {
            Ok(None)
        }
    }

    fn capture_names(&self) -> Vec<String> {
        let mut names = Vec::new();
        self.object.push_capture_names(&mut names);
        names
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum PatternObject {
    Capture(CaptureName),
    RestCapture(CaptureName),
    Atom(String),
    Delimited {
        delimiter: Delimiter,
        children: Vec<PatternObject>,
    },
}

impl PatternObject {
    fn from_block(object: &Block) -> Result<Self, SchemaError> {
        if let Some(text) = object.demote_to_string() {
            if let Some(capture) = CaptureName::from_token(text)? {
                if capture.rest {
                    return Ok(Self::RestCapture(capture));
                }
                return Ok(Self::Capture(capture));
            }
            return Ok(Self::Atom(text.to_owned()));
        }
        match object {
            Block::Delimited {
                delimiter,
                root_objects,
                ..
            } => {
                let mut children = Vec::new();
                for child in root_objects {
                    children.push(Self::from_block(child)?);
                }
                Ok(Self::Delimited {
                    delimiter: *delimiter,
                    children,
                })
            }
            Block::PipeText(_) => Ok(Self::Atom(NotationBlock::new(object).compact_notation())),
            Block::Atom(_) => unreachable!("atoms are handled by demote_to_string"),
        }
    }

    fn from_data(data: MacroPatternObjectData) -> Self {
        match data {
            MacroPatternObjectData::Capture(name) => Self::Capture(CaptureName::single(name)),
            MacroPatternObjectData::RestCapture(name) => Self::RestCapture(CaptureName::rest(name)),
            MacroPatternObjectData::Atom(text) => Self::Atom(text),
            MacroPatternObjectData::Delimited(data) => Self::Delimited {
                delimiter: data.delimiter().into_nota(),
                children: data.children.into_iter().map(Self::from_data).collect(),
            },
        }
    }

    fn to_data(&self) -> MacroPatternObjectData {
        match self {
            Self::Capture(capture) => MacroPatternObjectData::Capture(capture.name().to_owned()),
            Self::RestCapture(capture) => {
                MacroPatternObjectData::RestCapture(capture.name().to_owned())
            }
            Self::Atom(text) => MacroPatternObjectData::Atom(text.clone()),
            Self::Delimited {
                delimiter,
                children,
            } => MacroPatternObjectData::Delimited(Box::new(MacroPatternDelimitedData::new(
                MacroDelimiter::from_nota(*delimiter),
                children.iter().map(Self::to_data).collect(),
            ))),
        }
    }

    fn matches_pair(
        &self,
        pair: MacroPair<'_>,
        bindings: &mut MacroBindings,
    ) -> Result<bool, SchemaError> {
        let Self::Delimited {
            delimiter: Delimiter::Parenthesis,
            children,
        } = self
        else {
            return Ok(false);
        };
        if children.len() != 2 {
            return Ok(false);
        }
        Ok(children[0].matches_block(pair.name, bindings)?
            && children[1].matches_block(pair.definition, bindings)?)
    }

    fn matches_block(
        &self,
        object: &Block,
        bindings: &mut MacroBindings,
    ) -> Result<bool, SchemaError> {
        match self {
            Self::Capture(capture) => bindings.bind_single(capture.name(), object),
            Self::RestCapture(_) => Ok(false),
            Self::Atom(expected) => Ok(object.demote_to_string() == Some(expected.as_str())),
            Self::Delimited {
                delimiter,
                children,
            } => match object {
                Block::Delimited {
                    delimiter: found,
                    root_objects,
                    ..
                } if found == delimiter => {
                    PatternChildren::new(children).matches(root_objects, bindings)
                }
                _ => Ok(false),
            },
        }
    }

    fn push_capture_names(&self, names: &mut Vec<String>) {
        match self {
            Self::Capture(capture) | Self::RestCapture(capture) => {
                let prefix = if capture.rest { "$*" } else { "$" };
                names.push(format!("{prefix}{}", capture.name()));
            }
            Self::Delimited { children, .. } => {
                for child in children {
                    child.push_capture_names(names);
                }
            }
            Self::Atom(_) => {}
        }
    }

    fn as_rest_capture(&self) -> Option<&CaptureName> {
        match self {
            Self::RestCapture(capture) => Some(capture),
            Self::Capture(_) | Self::Atom(_) | Self::Delimited { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PatternChildren<'pattern> {
    children: &'pattern [PatternObject],
}

impl<'pattern> PatternChildren<'pattern> {
    fn new(children: &'pattern [PatternObject]) -> Self {
        Self { children }
    }

    fn matches(
        &self,
        objects: &[Block],
        bindings: &mut MacroBindings,
    ) -> Result<bool, SchemaError> {
        if let Some(rest_index) = self.rest_capture_index() {
            return self.matches_with_rest_capture(rest_index, objects, bindings);
        }
        if self.children.len() != objects.len() {
            return Ok(false);
        }
        for (pattern, object) in self.children.iter().zip(objects) {
            if !pattern.matches_block(object, bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn rest_capture_index(&self) -> Option<usize> {
        self.children
            .iter()
            .position(|child| child.as_rest_capture().is_some())
    }

    fn matches_with_rest_capture(
        &self,
        rest_index: usize,
        objects: &[Block],
        bindings: &mut MacroBindings,
    ) -> Result<bool, SchemaError> {
        let before = rest_index;
        let after = self.children.len() - rest_index - 1;
        if objects.len() < before + after {
            return Ok(false);
        }
        for (pattern, object) in self.children.iter().zip(objects).take(before) {
            if !pattern.matches_block(object, bindings)? {
                return Ok(false);
            }
        }
        let repeated_end = objects.len() - after;
        let capture = self.children[rest_index]
            .as_rest_capture()
            .expect("rest index came from rest capture");
        bindings.bind_repeated(capture.name(), &objects[before..repeated_end])?;
        for index in 0..after {
            let pattern_index = rest_index + 1 + index;
            let object_index = repeated_end + index;
            if !self.children[pattern_index].matches_block(&objects[object_index], bindings)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MacroTemplate {
    object: TemplateObject,
}

impl MacroTemplate {
    fn from_block(object: &Block) -> Result<Self, SchemaError> {
        Ok(Self {
            object: TemplateObject::from_block(object)?,
        })
    }

    fn from_data(data: MacroTemplateData) -> Self {
        Self {
            object: TemplateObject::from_data(data.object),
        }
    }

    fn to_data(&self) -> MacroTemplateData {
        MacroTemplateData::new(self.object.to_data())
    }

    fn expand(&self, bindings: &MacroBindings) -> Result<ExpandedTemplate, SchemaError> {
        let mut objects = self.object.expand_objects(bindings)?;
        let object = objects
            .pop()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: String::new(),
            })?;
        let source = object.compact_notation();
        if !objects.is_empty() {
            return Err(SchemaError::UnknownAssembledTemplate { found: source });
        }
        Ok(ExpandedTemplate { object, source })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TemplateObject {
    Capture(CaptureName),
    RestCapture(CaptureName),
    Atom(String),
    Delimited {
        delimiter: Delimiter,
        children: Vec<TemplateObject>,
    },
}

impl TemplateObject {
    fn from_block(object: &Block) -> Result<Self, SchemaError> {
        if let Some(text) = object.demote_to_string() {
            if let Some(capture) = CaptureName::from_token(text)? {
                if capture.rest {
                    return Ok(Self::RestCapture(capture));
                }
                return Ok(Self::Capture(capture));
            }
            return Ok(Self::Atom(text.to_owned()));
        }
        match object {
            Block::Delimited {
                delimiter,
                root_objects,
                ..
            } => {
                let mut children = Vec::new();
                for child in root_objects {
                    children.push(Self::from_block(child)?);
                }
                Ok(Self::Delimited {
                    delimiter: *delimiter,
                    children,
                })
            }
            Block::PipeText(_) => Ok(Self::Atom(NotationBlock::new(object).compact_notation())),
            Block::Atom(_) => unreachable!("atoms are handled by demote_to_string"),
        }
    }

    fn from_data(data: MacroTemplateObjectData) -> Self {
        match data {
            MacroTemplateObjectData::Capture(name) => Self::Capture(CaptureName::single(name)),
            MacroTemplateObjectData::RestCapture(name) => {
                Self::RestCapture(CaptureName::rest(name))
            }
            MacroTemplateObjectData::Atom(text) => Self::Atom(text),
            MacroTemplateObjectData::Delimited(data) => Self::Delimited {
                delimiter: data.delimiter().into_nota(),
                children: data.children.into_iter().map(Self::from_data).collect(),
            },
        }
    }

    fn to_data(&self) -> MacroTemplateObjectData {
        match self {
            Self::Capture(capture) => MacroTemplateObjectData::Capture(capture.name().to_owned()),
            Self::RestCapture(capture) => {
                MacroTemplateObjectData::RestCapture(capture.name().to_owned())
            }
            Self::Atom(text) => MacroTemplateObjectData::Atom(text.clone()),
            Self::Delimited {
                delimiter,
                children,
            } => MacroTemplateObjectData::Delimited(Box::new(MacroTemplateDelimitedData::new(
                MacroDelimiter::from_nota(*delimiter),
                children.iter().map(Self::to_data).collect(),
            ))),
        }
    }

    fn expand_objects(&self, bindings: &MacroBindings) -> Result<Vec<ExpandedObject>, SchemaError> {
        match self {
            Self::Capture(capture) => Ok(vec![ExpandedObject::Captured(
                bindings.single(capture.name())?.clone(),
            )]),
            Self::RestCapture(capture) => Ok(bindings
                .repeated(capture.name())?
                .iter()
                .cloned()
                .map(ExpandedObject::Captured)
                .collect()),
            Self::Atom(text) => Ok(vec![ExpandedObject::Atom(text.clone())]),
            Self::Delimited {
                delimiter,
                children,
            } => {
                let mut expanded_children = Vec::new();
                for child in children {
                    expanded_children.extend(child.expand_objects(bindings)?);
                }
                Ok(vec![ExpandedObject::Delimited {
                    delimiter: *delimiter,
                    children: expanded_children,
                }])
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CaptureName {
    name: String,
    rest: bool,
}

impl CaptureName {
    fn single(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rest: false,
        }
    }

    fn rest(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            rest: true,
        }
    }

    fn from_token(token: &str) -> Result<Option<Self>, SchemaError> {
        if !token.starts_with('$') {
            return Ok(None);
        }
        let (rest, name) = token
            .strip_prefix("$*")
            .map(|name| (true, name))
            .or_else(|| token.strip_prefix('$').map(|name| (false, name)))
            .expect("starts with dollar");
        if name.is_empty() {
            return Err(SchemaError::InvalidMacroCapture {
                found: token.to_owned(),
            });
        }
        Ok(Some(Self {
            name: name.to_owned(),
            rest,
        }))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MacroBindings {
    singles: Vec<SingleMacroBinding>,
    repeated: Vec<RepeatedMacroBinding>,
}

impl MacroBindings {
    fn bind_single(&mut self, name: &str, value: &Block) -> Result<bool, SchemaError> {
        if let Some(existing) = self.singles.iter().find(|binding| binding.name == name) {
            return Ok(existing.value == *value);
        }
        self.singles.push(SingleMacroBinding {
            name: name.to_owned(),
            value: value.clone(),
        });
        Ok(true)
    }

    fn bind_repeated(&mut self, name: &str, values: &[Block]) -> Result<(), SchemaError> {
        if let Some(existing) = self.repeated.iter().find(|binding| binding.name == name) {
            if existing.values == values {
                return Ok(());
            }
            return Err(SchemaError::ConflictingMacroBinding {
                name: name.to_owned(),
            });
        }
        self.repeated.push(RepeatedMacroBinding {
            name: name.to_owned(),
            values: values.to_vec(),
        });
        Ok(())
    }

    fn single(&self, name: &str) -> Result<&Block, SchemaError> {
        self.singles
            .iter()
            .find(|binding| binding.name == name)
            .map(|binding| &binding.value)
            .ok_or_else(|| SchemaError::MissingMacroBinding {
                name: name.to_owned(),
            })
    }

    fn repeated(&self, name: &str) -> Result<&[Block], SchemaError> {
        self.repeated
            .iter()
            .find(|binding| binding.name == name)
            .map(|binding| binding.values.as_slice())
            .ok_or_else(|| SchemaError::MissingMacroBinding {
                name: name.to_owned(),
            })
    }

    fn remember(&self, macro_name: &str, context: &mut MacroContext) {
        for binding in &self.singles {
            context.remember_binding(macro_name, &binding.name);
        }
        for binding in &self.repeated {
            context.remember_binding(macro_name, format!("*{}", binding.name));
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SingleMacroBinding {
    name: String,
    value: Block,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepeatedMacroBinding {
    name: String,
    values: Vec<Block>,
}

#[derive(Clone, Debug)]
struct DeclarativeSchemaMacro {
    definition: MacroDefinition,
}

impl SchemaMacro for DeclarativeSchemaMacro {
    fn name(&self) -> &str {
        self.definition.name.as_str()
    }

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool {
        position == self.definition.position
            && self
                .definition
                .pattern
                .captures(object)
                .is_ok_and(|captures| captures.is_some())
    }

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError> {
        if position != self.definition.position {
            return Err(SchemaError::MacroDidNotMatch {
                macro_name: self.name().to_owned(),
            });
        }
        let bindings = self.definition.pattern.captures(object)?.ok_or_else(|| {
            SchemaError::MacroDidNotMatch {
                macro_name: self.name().to_owned(),
            }
        })?;
        context.remember_macro(self.name());
        context.remember_position(position);
        bindings.remember(self.name(), context);
        let expanded = self.definition.template.expand(&bindings)?;
        context.remember_expanded_template(self.name(), expanded.source());
        expanded.lower_to_output(registry, context)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpandedTemplate {
    object: ExpandedObject,
    source: String,
}

impl ExpandedTemplate {
    fn source(&self) -> &str {
        &self.source
    }

    fn lower_to_output(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        AssembledTemplate::new(ObjectView::Expanded(&self.object)).lower(registry, context)
    }
}

#[derive(Clone, Copy, Debug)]
enum ObjectView<'object> {
    Block(&'object Block),
    Expanded(&'object ExpandedObject),
}

impl<'object> ObjectView<'object> {
    fn compact_notation(&self) -> String {
        match self {
            Self::Block(block) => NotationBlock::new(block).compact_notation(),
            Self::Expanded(object) => object.compact_notation(),
        }
    }

    fn demote_to_string(&self) -> Option<&'object str> {
        match self {
            Self::Block(block) => block.demote_to_string(),
            Self::Expanded(object) => object.demote_to_string(),
        }
    }

    fn schema_name(&self) -> Result<Name, SchemaError> {
        match self {
            Self::Block(block) => block.schema_name(),
            Self::Expanded(object) => object.schema_name(),
        }
    }

    fn is_parenthesis(&self) -> bool {
        self.delimited_children(Delimiter::Parenthesis).is_some()
    }

    fn holds_root_objects(&self) -> usize {
        match self {
            Self::Block(block) => block.holds_root_objects(),
            Self::Expanded(object) => object.holds_root_objects(),
        }
    }

    fn root_object_at(&self, index: usize) -> Option<Self> {
        match self {
            Self::Block(block) => block.root_object_at(index).map(Self::Block),
            Self::Expanded(ExpandedObject::Captured(block)) => {
                block.root_object_at(index).map(Self::Block)
            }
            Self::Expanded(object) => object.root_object_at(index).map(Self::Expanded),
        }
    }

    fn root_objects(&self) -> Vec<Self> {
        match self {
            Self::Block(block) => block.root_objects().iter().map(Self::Block).collect(),
            Self::Expanded(ExpandedObject::Captured(block)) => {
                block.root_objects().iter().map(Self::Block).collect()
            }
            Self::Expanded(object) => object.root_objects().iter().map(Self::Expanded).collect(),
        }
    }

    fn delimited_children(&self, expected: Delimiter) -> Option<Vec<Self>> {
        match self {
            Self::Block(Block::Delimited {
                delimiter,
                root_objects,
                ..
            }) if *delimiter == expected => Some(root_objects.iter().map(Self::Block).collect()),
            Self::Expanded(ExpandedObject::Delimited {
                delimiter,
                children,
            }) if *delimiter == expected => Some(children.iter().map(Self::Expanded).collect()),
            Self::Expanded(ExpandedObject::Captured(block)) => {
                ObjectView::Block(block).delimited_children(expected)
            }
            Self::Block(_) | Self::Expanded(_) => None,
        }
    }

    fn qualifies_as_pascal_case_symbol(&self) -> bool {
        match self {
            Self::Block(block) => block.qualifies_as_pascal_case_symbol(),
            Self::Expanded(object) => object.qualifies_as_pascal_case_symbol(),
        }
    }

    fn type_reference(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        match self {
            Self::Block(block) => TypeReference::from_block_with_registry(block, registry, context),
            Self::Expanded(object) => object.type_reference(registry, context),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ExpandedObject {
    Captured(Block),
    Atom(String),
    Delimited {
        delimiter: Delimiter,
        children: Vec<ExpandedObject>,
    },
}

impl ExpandedObject {
    fn compact_notation(&self) -> String {
        match self {
            Self::Captured(block) => NotationBlock::new(block).compact_notation(),
            Self::Atom(text) => text.clone(),
            Self::Delimited {
                delimiter,
                children,
            } => DelimitedNotation::new(*delimiter).wrap_children(
                &children
                    .iter()
                    .map(Self::compact_notation)
                    .collect::<Vec<_>>(),
            ),
        }
    }

    fn demote_to_string(&self) -> Option<&str> {
        match self {
            Self::Captured(block) => block.demote_to_string(),
            Self::Atom(text) => Some(text.as_str()),
            Self::Delimited { .. } => None,
        }
    }

    fn schema_name(&self) -> Result<Name, SchemaError> {
        match self {
            Self::Captured(block) => block.schema_name(),
            Self::Atom(text) => {
                let name = Name::new(text);
                if name.qualifies_as_symbol_name() {
                    Ok(name)
                } else {
                    Err(SchemaError::ExpectedSymbol {
                        found: text.clone(),
                    })
                }
            }
            Self::Delimited { .. } => Err(SchemaError::ExpectedSymbol {
                found: self.compact_notation(),
            }),
        }
    }

    fn holds_root_objects(&self) -> usize {
        match self {
            Self::Captured(block) => block.holds_root_objects(),
            Self::Delimited { children, .. } => children.len(),
            Self::Atom(_) => 0,
        }
    }

    fn root_object_at(&self, index: usize) -> Option<&ExpandedObject> {
        match self {
            Self::Delimited { children, .. } => children.get(index),
            Self::Captured(_) | Self::Atom(_) => None,
        }
    }

    fn root_objects(&self) -> &[ExpandedObject] {
        match self {
            Self::Delimited { children, .. } => children,
            Self::Captured(_) | Self::Atom(_) => &[],
        }
    }

    fn qualifies_as_pascal_case_symbol(&self) -> bool {
        match self {
            Self::Captured(block) => block.qualifies_as_pascal_case_symbol(),
            Self::Atom(text) => {
                AtomClassification::classify(text) == AtomClassification::SymbolCandidate
                    && text
                        .chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_uppercase())
                    && !text.contains('-')
            }
            Self::Delimited { .. } => false,
        }
    }

    fn type_reference(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        match self {
            Self::Captured(block) => {
                TypeReference::from_block_with_registry(block, registry, context)
            }
            Self::Atom(_) => Ok(TypeReference::from_name(self.schema_name()?)),
            Self::Delimited {
                delimiter: Delimiter::Parenthesis,
                children,
            } => ExpandedReference::new(children).type_reference(registry, context),
            Self::Delimited {
                delimiter: Delimiter::SquareBracket,
                children,
            } => Err(SchemaError::UnknownTypeReferenceForm {
                head: "SquareBracket".to_owned(),
                argument_count: children.len(),
            }),
            Self::Delimited {
                delimiter: Delimiter::Brace,
                children,
            } => Err(SchemaError::UnknownTypeReferenceForm {
                head: "Brace".to_owned(),
                argument_count: children.len(),
            }),
            Self::Delimited {
                delimiter: Delimiter::PipeBrace,
                children,
            } => ExpandedReference::new(children).inline_struct(registry, context),
            Self::Delimited {
                delimiter: Delimiter::PipeParenthesis,
                children,
            } => ExpandedReference::new(children).inline_enum(registry, context),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct ExpandedReference<'object> {
    children: &'object [ExpandedObject],
}

impl<'object> ExpandedReference<'object> {
    fn new(children: &'object [ExpandedObject]) -> Self {
        Self { children }
    }

    fn type_reference(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        if self.children.len() == 2 {
            if let Some(head) = self.children[0].demote_to_string() {
                match head {
                    "Vec" | "Vector" => {
                        return Ok(TypeReference::Vector(Box::new(
                            self.children[1].type_reference(registry, context)?,
                        )));
                    }
                    "Optional" | "Option" => {
                        return Ok(TypeReference::Optional(Box::new(
                            self.children[1].type_reference(registry, context)?,
                        )));
                    }
                    "Map" | "KeyValue" => {
                        return self.grouped_map_payload(&self.children[1], registry, context);
                    }
                    _ => {}
                }
            }
        }
        Err(SchemaError::UnknownTypeReferenceForm {
            head: self
                .children
                .first()
                .and_then(ExpandedObject::demote_to_string)
                .unwrap_or("<missing>")
                .to_owned(),
            argument_count: self.children.len().saturating_sub(1),
        })
    }

    fn grouped_map_payload(
        &self,
        payload: &'object ExpandedObject,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        let ExpandedObject::Delimited {
            delimiter: Delimiter::Parenthesis,
            children,
        } = payload
        else {
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: "Map".to_owned(),
                argument_count: 1,
            });
        };
        if children.len() != 2 {
            return Err(SchemaError::UnknownTypeReferenceForm {
                head: "Map".to_owned(),
                argument_count: children.len(),
            });
        }
        Ok(TypeReference::Map(
            Box::new(children[0].type_reference(registry, context)?),
            Box::new(children[1].type_reference(registry, context)?),
        ))
    }

    fn inline_struct(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        let name = self.inline_declaration_name("inline struct declaration")?;
        let fields = AssembledFields::from_objects(
            self.children[1..]
                .iter()
                .map(ObjectView::Expanded)
                .collect(),
        )
        .lower(registry, context)?;
        if fields.len() == 1 {
            let reference = fields.into_iter().next().expect("length checked").reference;
            context.remember_inline_declaration(crate::Declaration::private(
                TypeDeclaration::Newtype(NewtypeDeclaration::new(name.clone(), reference)),
            ));
        } else {
            context.remember_inline_declaration(crate::Declaration::private(
                TypeDeclaration::Struct(StructDeclaration::new(name.clone(), fields)),
            ));
        }
        Ok(TypeReference::Plain(name))
    }

    fn inline_enum(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        let name = self.inline_declaration_name("inline enum declaration")?;
        let variants = AssembledVariants::from_objects(
            self.children[1..]
                .iter()
                .map(ObjectView::Expanded)
                .collect(),
        )
        .lower(registry, context)?;
        context.remember_inline_declaration(crate::Declaration::private(TypeDeclaration::Enum(
            EnumDeclaration::new(name.clone(), variants),
        )));
        Ok(TypeReference::Plain(name))
    }

    fn inline_declaration_name(&self, form: &'static str) -> Result<Name, SchemaError> {
        let Some(name) = self.children.first() else {
            return Err(SchemaError::ExpectedSyntaxReferenceArity {
                form,
                expected: "declaration name plus body",
                found: 0,
            });
        };
        ObjectView::Expanded(name).schema_name()
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledTemplate<'template> {
    object: ObjectView<'template>,
}

impl<'template> AssembledTemplate<'template> {
    fn new(object: ObjectView<'template>) -> Self {
        Self { object }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        let children = self.parenthesized_children("assembled template")?;
        let head = children
            .first()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: self.object.compact_notation(),
            })?
            .schema_name()?;
        match head.as_str() {
            "Type" => AssembledType::new(self.child(&children, 1, "Type")?)
                .lower(registry, context)
                .map(MacroOutput::Type),
            "Fields" => AssembledFields::from_objects(children[1..].to_vec())
                .lower(registry, context)
                .map(MacroOutput::Fields),
            "Variants" => AssembledVariants::from_objects(children[1..].to_vec())
                .lower(registry, context)
                .map(MacroOutput::Variants),
            "Reference" => AssembledReference::new(children[1..].to_vec())
                .lower(registry, context)
                .map(MacroOutput::Reference),
            found => Err(SchemaError::UnknownAssembledTemplate {
                found: found.to_owned(),
            }),
        }
    }

    fn child(
        &self,
        children: &[ObjectView<'template>],
        index: usize,
        template_name: &'static str,
    ) -> Result<ObjectView<'template>, SchemaError> {
        children
            .get(index)
            .copied()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: template_name.to_owned(),
            })
    }

    fn parenthesized_children(
        &self,
        expected: &'static str,
    ) -> Result<Vec<ObjectView<'template>>, SchemaError> {
        self.object
            .delimited_children(Delimiter::Parenthesis)
            .ok_or(SchemaError::ExpectedDelimiter { expected })
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledType<'template> {
    object: ObjectView<'template>,
}

impl<'template> AssembledType<'template> {
    fn new(object: ObjectView<'template>) -> Self {
        Self { object }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let children =
            AssembledTemplate::new(self.object).parenthesized_children("assembled type")?;
        let kind = children
            .first()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: "Type".to_owned(),
            })?
            .schema_name()?;
        match kind.as_str() {
            "Struct" => self.lower_struct(&children, registry, context),
            "Enum" => self.lower_enum(&children, registry, context),
            "Newtype" => self.lower_newtype(&children, registry, context),
            found => Err(SchemaError::UnknownAssembledTemplate {
                found: found.to_owned(),
            }),
        }
    }

    fn lower_struct(
        &self,
        children: &[ObjectView<'template>],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.child(children, 1, "Struct")?.schema_name()?;
        let body = self.child(children, 2, "Struct")?;
        let fields = AssembledFields::from_objects(body.root_objects()).lower(registry, context)?;
        if fields.len() == 1 {
            let reference = fields.into_iter().next().expect("length checked").reference;
            Ok(TypeDeclaration::Newtype(NewtypeDeclaration::new(
                name, reference,
            )))
        } else {
            Ok(TypeDeclaration::Struct(StructDeclaration::new(
                name, fields,
            )))
        }
    }

    fn lower_enum(
        &self,
        children: &[ObjectView<'template>],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.child(children, 1, "Enum")?.schema_name()?;
        let body = self.child(children, 2, "Enum")?;
        let variants =
            AssembledVariants::from_objects(body.root_objects()).lower(registry, context)?;
        Ok(TypeDeclaration::Enum(EnumDeclaration::new(name, variants)))
    }

    fn lower_newtype(
        &self,
        children: &[ObjectView<'template>],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.child(children, 1, "Newtype")?.schema_name()?;
        let reference = self
            .child(children, 2, "Newtype")?
            .type_reference(registry, context)?;
        Ok(TypeDeclaration::Newtype(NewtypeDeclaration::new(
            name, reference,
        )))
    }

    fn child(
        &self,
        children: &[ObjectView<'template>],
        index: usize,
        template_name: &'static str,
    ) -> Result<ObjectView<'template>, SchemaError> {
        children
            .get(index)
            .copied()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: template_name.to_owned(),
            })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AssembledFields<'template> {
    objects: Vec<ObjectView<'template>>,
}

impl<'template> AssembledFields<'template> {
    pub(crate) fn new(objects: &'template [Block]) -> Self {
        Self {
            objects: objects.iter().map(ObjectView::Block).collect(),
        }
    }

    fn from_objects(objects: Vec<ObjectView<'template>>) -> Self {
        Self { objects }
    }

    pub(crate) fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<FieldDeclaration>, SchemaError> {
        let mut fields = Vec::new();
        let mut index = 0;
        while index < self.objects.len() {
            if self.starts_flat_field_pair(index) {
                let next_index = index + 1;
                if next_index >= self.objects.len() {
                    return Err(SchemaError::ExpectedSyntaxReferenceArity {
                        form: "flat struct field pair",
                        expected: "field name plus one type reference",
                        found: 1,
                    });
                }
                fields.push(
                    AssembledField::new_named_pair(self.objects[index], self.objects[next_index])
                        .lower(registry, context)?,
                );
                index += 2;
            } else if self.starts_ambiguous_pascal_pair(index) {
                fields.push(
                    AssembledField::new_named_pair(self.objects[index], self.objects[index + 1])
                        .lower(registry, context)?,
                );
                index += 2;
            } else {
                fields.push(AssembledField::new(self.objects[index]).lower(registry, context)?);
                index += 1;
            }
        }
        Ok(fields)
    }

    fn starts_flat_field_pair(&self, index: usize) -> bool {
        let Some(name) = self.objects[index].demote_to_string() else {
            return false;
        };
        if self
            .objects
            .get(index + 1)
            .and_then(ObjectView::demote_to_string)
            == Some("*")
        {
            return true;
        }
        !name.contains('@')
            && name
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_lowercase())
    }

    fn starts_ambiguous_pascal_pair(&self, index: usize) -> bool {
        if index + 1 >= self.objects.len() {
            return false;
        }
        self.objects[index].demote_to_string().is_some_and(|name| {
            !name.contains('@')
                && name
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_uppercase())
        })
    }
}

/// One field inside a struct body.
///
/// Strict struct bodies are key/value maps. `Topic *` derives the
/// field name from an already-declared type (`topic`) and creates a
/// `Plain` reference. Native NOTA type-reference objects can also sit
/// directly in a field position: `(Vec Topic)`,
/// `(Map (Topic RecordIdentifier))`, and `(Optional Topic)` lower to
/// vector, map, and optional references with names derived from the
/// reference shape. A parenthesised pair whose first object is a
/// lower-case field symbol remains the explicit escape hatch for
/// uncommon names.
#[derive(Clone, Copy, Debug)]
struct AssembledField<'template> {
    object: ObjectView<'template>,
    paired_reference: Option<ObjectView<'template>>,
}

impl<'template> AssembledField<'template> {
    fn new(object: ObjectView<'template>) -> Self {
        Self {
            object,
            paired_reference: None,
        }
    }

    fn new_named_pair(name: ObjectView<'template>, reference: ObjectView<'template>) -> Self {
        Self {
            object: name,
            paired_reference: Some(reference),
        }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<FieldDeclaration, SchemaError> {
        if let Some(reference_object) = self.paired_reference {
            let field_name = self.object.schema_name()?;
            let reference = if reference_object.demote_to_string() == Some("*") {
                TypeReference::from_name(field_name.clone())
            } else if Self::is_pascal_case_name(&field_name) {
                let declaration = self.inline_declaration(
                    field_name.clone(),
                    reference_object,
                    registry,
                    context,
                )?;
                context.remember_inline_declaration(Declaration::private(declaration));
                TypeReference::from_name(field_name.clone())
            } else {
                reference_object.type_reference(registry, context)?
            };
            return Ok(FieldDeclaration {
                name: Name::new(field_name.field_name()),
                reference,
            });
        }
        if self.is_explicit_field_pair() {
            let field_name = self
                .object
                .root_object_at(0)
                .expect("count checked")
                .schema_name()?;
            let reference = self
                .object
                .root_object_at(1)
                .expect("count checked")
                .type_reference(registry, context)?;
            return Ok(FieldDeclaration {
                name: Name::new(field_name.field_name()),
                reference,
            });
        }
        if self.object.demote_to_string().is_none() {
            let reference = self.object.type_reference(registry, context)?;
            return Ok(FieldDeclaration {
                name: self.derived_name_for_reference(&reference),
                reference,
            });
        }
        let name = self.object.schema_name()?;
        Ok(FieldDeclaration {
            name: Name::new(name.field_name()),
            reference: TypeReference::from_name(name),
        })
    }

    fn inline_declaration(
        &self,
        name: Name,
        object: ObjectView<'template>,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        if let Some(children) = object.delimited_children(Delimiter::Brace) {
            let fields = AssembledFields::from_objects(children).lower(registry, context)?;
            return Ok(TypeDeclaration::Struct(StructDeclaration::new(
                name, fields,
            )));
        }
        if let Some(children) = object.delimited_children(Delimiter::SquareBracket) {
            let variants = AssembledVariants::from_objects(children).lower(registry, context)?;
            return Ok(TypeDeclaration::Enum(EnumDeclaration::new(name, variants)));
        }
        Ok(TypeDeclaration::Newtype(NewtypeDeclaration::new(
            name,
            object.type_reference(registry, context)?,
        )))
    }

    fn is_pascal_case_name(name: &Name) -> bool {
        name.as_str()
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_uppercase())
    }

    fn is_explicit_field_pair(&self) -> bool {
        self.object.is_parenthesis()
            && self.object.holds_root_objects() == 2
            && self
                .object
                .root_object_at(0)
                .and_then(|object| object.demote_to_string())
                .is_some_and(|name| {
                    name.chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_lowercase())
                })
    }

    fn derived_name_for_reference(&self, reference: &TypeReference) -> Name {
        match reference {
            TypeReference::String => Name::new("string"),
            TypeReference::Integer => Name::new("integer"),
            TypeReference::Boolean => Name::new("boolean"),
            TypeReference::Path => Name::new("path"),
            TypeReference::Plain(name) => Name::new(name.field_name()),
            TypeReference::Vector(inner) => {
                Name::new(format!("{}_vector", self.derived_name_for_reference(inner)))
            }
            TypeReference::Map(key, value) => Name::new(format!(
                "{}_by_{}",
                self.derived_name_for_reference(value),
                self.derived_name_for_reference(key)
            )),
            TypeReference::Optional(inner) => Name::new(format!(
                "optional_{}",
                self.derived_name_for_reference(inner)
            )),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AssembledVariants<'template> {
    objects: Vec<ObjectView<'template>>,
}

impl<'template> AssembledVariants<'template> {
    pub(crate) fn new(objects: &'template [Block]) -> Self {
        Self {
            objects: objects.iter().map(ObjectView::Block).collect(),
        }
    }

    fn from_objects(objects: Vec<ObjectView<'template>>) -> Self {
        Self { objects }
    }

    pub(crate) fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        self.objects
            .iter()
            .map(|object| AssembledVariant::new(*object).lower(registry, context))
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledVariant<'template> {
    object: ObjectView<'template>,
}

impl<'template> AssembledVariant<'template> {
    fn new(object: ObjectView<'template>) -> Self {
        Self { object }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<EnumVariant, SchemaError> {
        if self.object.is_parenthesis() {
            self.lower_parenthesis(registry, context)
        } else if self.object.qualifies_as_pascal_case_symbol() {
            Ok(EnumVariant {
                name: self.object.schema_name()?,
                payload: None,
            })
        } else {
            Err(SchemaError::ExpectedEnumVariant)
        }
    }

    fn lower_parenthesis(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<EnumVariant, SchemaError> {
        match self.object.holds_root_objects() {
            2 => Ok(EnumVariant {
                name: self
                    .object
                    .root_object_at(0)
                    .expect("count checked")
                    .schema_name()?,
                payload: Some(
                    self.object
                        .root_object_at(1)
                        .expect("count checked")
                        .type_reference(registry, context)?,
                ),
            }),
            _ => Err(SchemaError::ExpectedEnumVariant),
        }
    }
}

#[derive(Clone, Debug)]
struct AssembledReference<'template> {
    objects: Vec<ObjectView<'template>>,
}

impl<'template> AssembledReference<'template> {
    fn new(objects: Vec<ObjectView<'template>>) -> Self {
        Self { objects }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        if self.objects.len() != 1 {
            return Err(SchemaError::UnknownAssembledTemplate {
                found: "Reference".to_owned(),
            });
        }
        Self::lower_object(self.objects[0], registry, context)
    }

    fn lower_object(
        object: ObjectView<'template>,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        if !object.is_parenthesis() {
            return object.type_reference(registry, context);
        }
        let children =
            AssembledTemplate::new(object).parenthesized_children("assembled reference")?;
        let head = children
            .first()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: "Reference".to_owned(),
            })?
            .schema_name()?;
        match head.as_str() {
            "Vector" if children.len() == 2 => Ok(TypeReference::Vector(Box::new(
                Self::lower_object(children[1], registry, context)?,
            ))),
            "Optional" if children.len() == 2 => Ok(TypeReference::Optional(Box::new(
                Self::lower_object(children[1], registry, context)?,
            ))),
            "Map" if children.len() == 3 => Ok(TypeReference::Map(
                Box::new(Self::lower_object(children[1], registry, context)?),
                Box::new(Self::lower_object(children[2], registry, context)?),
            )),
            _ => object.type_reference(registry, context),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct NotationBlock<'block> {
    block: &'block Block,
}

impl<'block> NotationBlock<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn compact_notation(&self) -> String {
        match self.block {
            Block::Delimited {
                delimiter,
                root_objects,
                ..
            } => DelimitedNotation::new(*delimiter).wrap_children(
                &root_objects
                    .iter()
                    .map(|object| NotationBlock::new(object).compact_notation())
                    .collect::<Vec<_>>(),
            ),
            Block::PipeText(pipe_text) => format!("[|{}|]", pipe_text.text),
            Block::Atom(atom) => atom.text().to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct DelimitedNotation {
    delimiter: Delimiter,
}

impl DelimitedNotation {
    fn new(delimiter: Delimiter) -> Self {
        Self { delimiter }
    }

    fn wrap_children(&self, children: &[String]) -> String {
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
