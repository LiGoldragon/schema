use std::fmt;

use nota_next::{Block, Delimiter, Document};

use crate::{
    MacroContext, MacroObject, MacroOutput, MacroPosition, MacroRegistry, SchemaError,
    SchemaIdentity, SchemaMacro,
    macros::SchemaBlockExt,
    resolution::{ImportSource, ResolvedImport},
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

    pub fn to_nota(&self) -> String {
        AsschemaNotaWriter::new(self).render()
    }

    pub fn from_nota(source: &str) -> Result<Self, SchemaError> {
        let document = Document::parse(source)?;
        AsschemaNotaReader::new(&document).read()
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
/// the positions that previously only held a name. The authored macro
/// surface is a tagged/data-carrying variant: `(Vec [T])`,
/// `(KeyValue [K V])`, `(Option [T])`. The first object is the macro
/// tag; the second object is the macro input data. `Map` is the
/// schema-level ordered key-value collection; the concrete Rust
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
    /// lowers to `Plain`. A parenthesised tagged macro form lowers to
    /// a collection: `(Vec [T])` → `Vector`, `(KeyValue [K V])` →
    /// `Map`, `(Option [T])` → `Optional`. The inner positions
    /// recurse, so `(Vec [(Option [Topic])])` and
    /// `(KeyValue [NodeName (Vec [Service])])` nest. nota-next did
    /// the structural parse; this is pure semantic lowering over its
    /// `Block`s, not a hand-rolled text parser.
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

    fn argument_at(&self, index: usize) -> &Block {
        self.data.argument_at(index)
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

    fn argument_at(&self, index: usize) -> &'schema Block {
        match self {
            Self::Delimited(objects) => objects.get(index).expect("argument count checked"),
            Self::Single(object) if index == 0 => object,
            Self::Single(_) => panic!("argument count checked"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TypeReferenceMacroKind {
    Vector,
    Map,
    Optional,
}

/// Data representation of a schema-node object before macro execution.
///
/// A parenthesized schema node is a tagged/data-carrying variant:
/// `(Vec [Topic])` has tag `Vec` and data `[Topic]`. This type exists
/// so macro calls can be inspected, serialized through assembled
/// schema, and tested as data rather than disappearing into parser
/// control flow.
#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, Eq, PartialEq)]
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

struct AsschemaNotaWriter<'schema> {
    asschema: &'schema Asschema,
}

impl<'schema> AsschemaNotaWriter<'schema> {
    fn new(asschema: &'schema Asschema) -> Self {
        Self { asschema }
    }

    fn render(&self) -> String {
        NotaExpression::square([
            self.render_identity(),
            self.render_imports(),
            self.render_resolved_imports(),
            self.render_enum(self.asschema.input()),
            self.render_enum(self.asschema.output()),
            self.render_namespace(),
        ])
    }

    fn render_identity(&self) -> String {
        NotaExpression::square([
            self.render_name(self.asschema.identity().component()),
            NotaText::new(self.asschema.identity().version()).render(),
        ])
    }

    fn render_imports(&self) -> String {
        NotaExpression::square(self.asschema.imports().iter().map(|declaration| {
            NotaExpression::square([
                self.render_name(&declaration.local_name),
                self.render_reference(&declaration.source),
            ])
        }))
    }

    fn render_resolved_imports(&self) -> String {
        NotaExpression::square(self.asschema.resolved_imports().iter().map(|import| {
            NotaExpression::square([
                self.render_name(import.local_name()),
                self.render_name(import.source().crate_name()),
                self.render_name(import.source().module()),
                self.render_name(import.source().type_name()),
            ])
        }))
    }

    fn render_namespace(&self) -> String {
        NotaExpression::square(
            self.asschema
                .namespace()
                .iter()
                .map(|declaration| self.render_type_declaration(declaration)),
        )
    }

    fn render_type_declaration(&self, declaration: &TypeDeclaration) -> String {
        match declaration {
            TypeDeclaration::Struct(declaration) => {
                NotaExpression::parenthesis(["Struct".to_owned(), self.render_struct(declaration)])
            }
            TypeDeclaration::Newtype(declaration) => {
                NotaExpression::parenthesis(["Newtype".to_owned(), self.render_struct(declaration)])
            }
            TypeDeclaration::Enum(declaration) => {
                NotaExpression::parenthesis(["Enum".to_owned(), self.render_enum(declaration)])
            }
        }
    }

    fn render_struct(&self, declaration: &StructDeclaration) -> String {
        NotaExpression::square([
            self.render_name(&declaration.name),
            NotaExpression::square(
                declaration
                    .fields
                    .iter()
                    .map(|field| self.render_field(field)),
            ),
        ])
    }

    fn render_field(&self, declaration: &FieldDeclaration) -> String {
        NotaExpression::square([
            self.render_name(&declaration.name),
            self.render_reference(&declaration.reference),
        ])
    }

    fn render_enum(&self, declaration: &EnumDeclaration) -> String {
        NotaExpression::square([
            self.render_name(&declaration.name),
            NotaExpression::square(
                declaration
                    .variants
                    .iter()
                    .map(|variant| self.render_variant(variant)),
            ),
        ])
    }

    fn render_variant(&self, variant: &EnumVariant) -> String {
        let payload = match &variant.payload {
            Some(reference) => NotaExpression::parenthesis([
                "Carries".to_owned(),
                self.render_reference(reference),
            ]),
            None => "Unit".to_owned(),
        };
        NotaExpression::square([self.render_name(&variant.name), payload])
    }

    fn render_reference(&self, reference: &TypeReference) -> String {
        match reference {
            TypeReference::Plain(name) => {
                NotaExpression::parenthesis(["Plain".to_owned(), self.render_name(name)])
            }
            TypeReference::Vector(inner) => {
                NotaExpression::parenthesis(["Vector".to_owned(), self.render_reference(inner)])
            }
            TypeReference::Map(key, value) => NotaExpression::parenthesis([
                "Map".to_owned(),
                NotaExpression::square([self.render_reference(key), self.render_reference(value)]),
            ]),
            TypeReference::Optional(inner) => {
                NotaExpression::parenthesis(["Optional".to_owned(), self.render_reference(inner)])
            }
        }
    }

    fn render_name(&self, name: &Name) -> String {
        NotaText::new(name.as_str()).render()
    }
}

struct AsschemaNotaReader<'document> {
    document: &'document Document,
}

impl<'document> AsschemaNotaReader<'document> {
    fn new(document: &'document Document) -> Self {
        Self { document }
    }

    fn read(&self) -> Result<Asschema, SchemaError> {
        if self.document.holds_root_objects() != 1 {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: "one assembled-schema root object",
                found: self.document.holds_root_objects(),
            });
        }
        let root = AsschemaNotaBlock::new(
            self.document
                .root_object_at(0)
                .expect("root object count checked"),
        );
        let fields = root.square_children("Asschema", 6)?;
        let identity = AsschemaIdentityReader::new(&fields[0]).read()?;
        let imports = AsschemaImportReader::new(&fields[1]).read_imports()?;
        let resolved_imports = AsschemaResolvedImportReader::new(&fields[2]).read_imports()?;
        let input = AsschemaEnumReader::new(&fields[3]).read()?;
        let output = AsschemaEnumReader::new(&fields[4]).read()?;
        let namespace = AsschemaTypeReader::new(&fields[5]).read_namespace()?;
        Ok(Asschema::new(
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
        ))
    }
}

struct AsschemaIdentityReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaIdentityReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read(&self) -> Result<SchemaIdentity, SchemaError> {
        let fields = AsschemaNotaBlock::new(self.block).square_children("SchemaIdentity", 2)?;
        Ok(SchemaIdentity::new(
            AsschemaNotaBlock::new(&fields[0])
                .name()?
                .as_str()
                .to_owned(),
            AsschemaNotaBlock::new(&fields[1]).text()?.to_owned(),
        ))
    }
}

struct AsschemaImportReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaImportReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read_imports(&self) -> Result<Vec<ImportDeclaration>, SchemaError> {
        let mut imports = Vec::new();
        for object in
            AsschemaNotaBlock::new(self.block).square_any_children("ImportDeclarations")?
        {
            imports.push(self.read_import(object)?);
        }
        Ok(imports)
    }

    fn read_import(&self, object: &Block) -> Result<ImportDeclaration, SchemaError> {
        let fields = AsschemaNotaBlock::new(object).square_children("ImportDeclaration", 2)?;
        Ok(ImportDeclaration {
            local_name: AsschemaNotaBlock::new(&fields[0]).name()?,
            source: AsschemaReferenceReader::new(&fields[1]).read()?,
        })
    }
}

struct AsschemaResolvedImportReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaResolvedImportReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read_imports(&self) -> Result<Vec<ResolvedImport>, SchemaError> {
        let mut imports = Vec::new();
        for object in AsschemaNotaBlock::new(self.block).square_any_children("ResolvedImports")? {
            let fields = AsschemaNotaBlock::new(object).square_children("ResolvedImport", 4)?;
            imports.push(ResolvedImport::new(
                AsschemaNotaBlock::new(&fields[0]).name()?,
                ImportSource::new(
                    AsschemaNotaBlock::new(&fields[1]).name()?,
                    AsschemaNotaBlock::new(&fields[2]).name()?,
                    AsschemaNotaBlock::new(&fields[3]).name()?,
                ),
            ));
        }
        Ok(imports)
    }
}

struct AsschemaTypeReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaTypeReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read_namespace(&self) -> Result<Vec<TypeDeclaration>, SchemaError> {
        let mut declarations = Vec::new();
        for object in AsschemaNotaBlock::new(self.block).square_any_children("Namespace")? {
            declarations.push(self.read_declaration(object)?);
        }
        Ok(declarations)
    }

    fn read_declaration(&self, object: &Block) -> Result<TypeDeclaration, SchemaError> {
        let fields = AsschemaNotaBlock::new(object).parenthesis_any_children("TypeDeclaration")?;
        let tag = AsschemaNotaBlock::new(fields.first().ok_or_else(|| {
            SchemaError::UnknownAssembledTemplate {
                found: "empty type declaration".to_owned(),
            }
        })?)
        .text()?;
        match tag {
            "Struct" if fields.len() == 2 => Ok(TypeDeclaration::Struct(
                AsschemaStructReader::new(&fields[1]).read()?,
            )),
            "Newtype" if fields.len() == 2 => Ok(TypeDeclaration::Newtype(
                AsschemaStructReader::new(&fields[1]).read()?,
            )),
            "Enum" if fields.len() == 2 => Ok(TypeDeclaration::Enum(
                AsschemaEnumReader::new(&fields[1]).read()?,
            )),
            _ => Err(SchemaError::UnknownAssembledTemplate {
                found: tag.to_owned(),
            }),
        }
    }
}

struct AsschemaStructReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaStructReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read(&self) -> Result<StructDeclaration, SchemaError> {
        let fields = AsschemaNotaBlock::new(self.block).square_children("StructDeclaration", 2)?;
        Ok(StructDeclaration {
            name: AsschemaNotaBlock::new(&fields[0]).name()?,
            fields: self.read_fields(&fields[1])?,
        })
    }

    fn read_fields(&self, object: &Block) -> Result<Vec<FieldDeclaration>, SchemaError> {
        let mut fields = Vec::new();
        for field in AsschemaNotaBlock::new(object).square_any_children("Fields")? {
            let values = AsschemaNotaBlock::new(field).square_children("FieldDeclaration", 2)?;
            fields.push(FieldDeclaration {
                name: AsschemaNotaBlock::new(&values[0]).name()?,
                reference: AsschemaReferenceReader::new(&values[1]).read()?,
            });
        }
        Ok(fields)
    }
}

struct AsschemaEnumReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaEnumReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read(&self) -> Result<EnumDeclaration, SchemaError> {
        let fields = AsschemaNotaBlock::new(self.block).square_children("EnumDeclaration", 2)?;
        Ok(EnumDeclaration {
            name: AsschemaNotaBlock::new(&fields[0]).name()?,
            variants: self.read_variants(&fields[1])?,
        })
    }

    fn read_variants(&self, object: &Block) -> Result<Vec<EnumVariant>, SchemaError> {
        let mut variants = Vec::new();
        for variant in AsschemaNotaBlock::new(object).square_any_children("Variants")? {
            let values = AsschemaNotaBlock::new(variant).square_children("EnumVariant", 2)?;
            variants.push(EnumVariant {
                name: AsschemaNotaBlock::new(&values[0]).name()?,
                payload: AsschemaPayloadReader::new(&values[1]).read()?,
            });
        }
        Ok(variants)
    }
}

struct AsschemaPayloadReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaPayloadReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read(&self) -> Result<Option<TypeReference>, SchemaError> {
        if self.block.demote_to_string() == Some("Unit") {
            return Ok(None);
        }
        let fields = AsschemaNotaBlock::new(self.block).parenthesis_any_children("Payload")?;
        if fields.len() == 2 && AsschemaNotaBlock::new(&fields[0]).text()? == "Carries" {
            return Ok(Some(AsschemaReferenceReader::new(&fields[1]).read()?));
        }
        Err(SchemaError::UnknownAssembledTemplate {
            found: "Payload".to_owned(),
        })
    }
}

struct AsschemaReferenceReader<'block> {
    block: &'block Block,
}

impl<'block> AsschemaReferenceReader<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn read(&self) -> Result<TypeReference, SchemaError> {
        let fields =
            AsschemaNotaBlock::new(self.block).parenthesis_any_children("TypeReference")?;
        let tag = AsschemaNotaBlock::new(fields.first().ok_or_else(|| {
            SchemaError::UnknownAssembledTemplate {
                found: "empty type reference".to_owned(),
            }
        })?)
        .text()?;
        match tag {
            "Plain" if fields.len() == 2 => Ok(TypeReference::Plain(
                AsschemaNotaBlock::new(&fields[1]).name()?,
            )),
            "Vector" if fields.len() == 2 => Ok(TypeReference::Vector(Box::new(
                AsschemaReferenceReader::new(&fields[1]).read()?,
            ))),
            "Optional" if fields.len() == 2 => Ok(TypeReference::Optional(Box::new(
                AsschemaReferenceReader::new(&fields[1]).read()?,
            ))),
            "Map" if fields.len() == 2 => {
                let pair = AsschemaNotaBlock::new(&fields[1]).square_children("MapReference", 2)?;
                Ok(TypeReference::Map(
                    Box::new(AsschemaReferenceReader::new(&pair[0]).read()?),
                    Box::new(AsschemaReferenceReader::new(&pair[1]).read()?),
                ))
            }
            _ => Err(SchemaError::UnknownTypeReferenceForm {
                head: tag.to_owned(),
                argument_count: fields.len().saturating_sub(1),
            }),
        }
    }
}

struct AsschemaNotaBlock<'block> {
    block: &'block Block,
}

impl<'block> AsschemaNotaBlock<'block> {
    fn new(block: &'block Block) -> Self {
        Self { block }
    }

    fn square_children(
        &self,
        type_name: &'static str,
        expected: usize,
    ) -> Result<&'block [Block], SchemaError> {
        let children = self.square_any_children(type_name)?;
        if children.len() != expected {
            return Err(SchemaError::ExpectedRootObjectCount {
                expected: type_name,
                found: children.len(),
            });
        }
        Ok(children)
    }

    fn square_any_children(&self, type_name: &'static str) -> Result<&'block [Block], SchemaError> {
        self.children(nota_next::Delimiter::SquareBracket, type_name)
    }

    fn parenthesis_any_children(
        &self,
        type_name: &'static str,
    ) -> Result<&'block [Block], SchemaError> {
        self.children(nota_next::Delimiter::Parenthesis, type_name)
    }

    fn children(
        &self,
        delimiter: nota_next::Delimiter,
        type_name: &'static str,
    ) -> Result<&'block [Block], SchemaError> {
        match self.block {
            Block::Delimited {
                delimiter: found,
                root_objects,
                ..
            } if *found == delimiter => Ok(root_objects),
            _ => Err(SchemaError::ExpectedDelimiter {
                expected: type_name,
            }),
        }
    }

    fn name(&self) -> Result<Name, SchemaError> {
        Ok(Name::new(self.text()?))
    }

    fn text(&self) -> Result<&'block str, SchemaError> {
        self.block
            .demote_to_string()
            .ok_or_else(|| SchemaError::ExpectedSymbol {
                found: format!("{:?}", self.block),
            })
    }
}

struct NotaExpression {
    opening: &'static str,
    closing: &'static str,
}

impl NotaExpression {
    fn square(expressions: impl IntoIterator<Item = String>) -> String {
        Self::new("[", "]").render(expressions)
    }

    fn parenthesis(expressions: impl IntoIterator<Item = String>) -> String {
        Self::new("(", ")").render(expressions)
    }

    fn new(opening: &'static str, closing: &'static str) -> Self {
        Self { opening, closing }
    }

    fn render(&self, expressions: impl IntoIterator<Item = String>) -> String {
        let children = expressions.into_iter().collect::<Vec<_>>();
        if children.is_empty() {
            return format!("{}{}", self.opening, self.closing);
        }
        format!("{}{}{}", self.opening, children.join(" "), self.closing)
    }
}

struct NotaText<'text> {
    value: &'text str,
}

impl<'text> NotaText<'text> {
    fn new(value: &'text str) -> Self {
        Self { value }
    }

    fn render(&self) -> String {
        if self
            .value
            .chars()
            .all(|character| NotaTextCharacter::new(character).is_atom_safe())
            && self
                .value
                .chars()
                .next()
                .is_some_and(|character| !character.is_whitespace())
        {
            self.value.to_owned()
        } else {
            format!("[|{}|]", self.value.replace("|]", "| ]"))
        }
    }
}

struct NotaTextCharacter {
    character: char,
}

impl NotaTextCharacter {
    fn new(character: char) -> Self {
        Self { character }
    }

    fn is_atom_safe(&self) -> bool {
        !self.character.is_whitespace()
            && !matches!(self.character, ';' | '(' | ')' | '[' | ']' | '{' | '}')
    }
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
                expected: "(Macro [input])",
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
