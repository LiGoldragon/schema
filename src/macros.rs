use nota_next::{Block, Delimiter, StructureHeader};

use crate::{
    Asschema, Declaration, EnumDeclaration, FieldDeclaration, ImportDeclaration, Name, SchemaError,
    TypeDeclaration, TypeReference,
};

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
pub enum MacroPosition {
    RootImports,
    RootInput,
    RootOutput,
    RootNamespace,
    NamespaceDeclaration,
    StructFields,
    EnumVariants,
    TypeReference,
}

impl MacroPosition {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RootImports => "RootImports",
            Self::RootInput => "RootInput",
            Self::RootOutput => "RootOutput",
            Self::RootNamespace => "RootNamespace",
            Self::NamespaceDeclaration => "NamespaceDeclaration",
            Self::StructFields => "StructFields",
            Self::EnumVariants => "EnumVariants",
            Self::TypeReference => "TypeReference",
        }
    }

    pub(crate) fn from_name(name: &Name) -> Result<Self, SchemaError> {
        match name.as_str() {
            "RootImports" => Ok(Self::RootImports),
            "RootInput" => Ok(Self::RootInput),
            "RootOutput" => Ok(Self::RootOutput),
            "RootNamespace" => Ok(Self::RootNamespace),
            "NamespaceDeclaration" => Ok(Self::NamespaceDeclaration),
            "StructFields" => Ok(Self::StructFields),
            "EnumVariants" => Ok(Self::EnumVariants),
            "TypeReference" => Ok(Self::TypeReference),
            found => Err(SchemaError::UnknownMacroPosition {
                found: found.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum MacroObject<'object> {
    Block(&'object Block),
    Pair(MacroPair<'object>),
}

impl<'object> MacroObject<'object> {
    pub fn block(self) -> Option<&'object Block> {
        match self {
            Self::Block(block) => Some(block),
            Self::Pair(_) => None,
        }
    }

    pub fn pair(self) -> Option<MacroPair<'object>> {
        match self {
            Self::Block(_) => None,
            Self::Pair(pair) => Some(pair),
        }
    }

    pub fn describe(self) -> String {
        match self {
            Self::Block(block) => format!("block({})", block.reemit_fallback()),
            Self::Pair(pair) => format!(
                "pair({} {})",
                pair.name.reemit_fallback(),
                pair.definition.reemit_fallback()
            ),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MacroPair<'object> {
    pub name: &'object Block,
    pub definition: &'object Block,
}

pub trait SchemaMacro {
    fn name(&self) -> &str;

    fn matches(&self, object: MacroObject<'_>, position: MacroPosition) -> bool;

    fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
        registry: &MacroRegistry,
    ) -> Result<MacroOutput, SchemaError>;
}

#[derive(Clone, Debug, Default)]
pub struct MacroContext {
    positions_seen: Vec<MacroPosition>,
    macros_applied: Vec<String>,
    bindings_seen: Vec<String>,
    expanded_templates: Vec<String>,
    structure_headers: Vec<StructureHeader>,
    inline_declarations: Vec<Declaration>,
}

impl MacroContext {
    pub fn remember_position(&mut self, position: MacroPosition) {
        self.positions_seen.push(position);
    }

    pub fn remember_macro(&mut self, macro_name: impl Into<String>) {
        self.macros_applied.push(macro_name.into());
    }

    pub fn remember_binding(&mut self, macro_name: impl AsRef<str>, binding_name: impl AsRef<str>) {
        self.bindings_seen.push(format!(
            "{}::{}",
            macro_name.as_ref(),
            binding_name.as_ref()
        ));
    }

    pub fn remember_expanded_template(
        &mut self,
        macro_name: impl AsRef<str>,
        template: impl AsRef<str>,
    ) {
        self.expanded_templates
            .push(format!("{} -> {}", macro_name.as_ref(), template.as_ref()));
    }

    pub fn remember_structure_header(&mut self, header: StructureHeader) {
        self.structure_headers.push(header);
    }

    pub(crate) fn remember_inline_declaration(&mut self, declaration: Declaration) {
        self.inline_declarations.push(declaration);
    }

    pub(crate) fn inline_declaration_count(&self) -> usize {
        self.inline_declarations.len()
    }

    pub(crate) fn drain_inline_declarations_from(&mut self, index: usize) -> Vec<Declaration> {
        self.inline_declarations.drain(index..).collect()
    }

    pub fn positions_seen(&self) -> &[MacroPosition] {
        &self.positions_seen
    }

    pub fn macros_applied(&self) -> &[String] {
        &self.macros_applied
    }

    pub fn bindings_seen(&self) -> &[String] {
        &self.bindings_seen
    }

    pub fn expanded_templates(&self) -> &[String] {
        &self.expanded_templates
    }

    pub fn structure_headers(&self) -> &[StructureHeader] {
        &self.structure_headers
    }

    pub fn inline_declarations(&self) -> &[Declaration] {
        &self.inline_declarations
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroOutput {
    Asschema(Asschema),
    Imports(Vec<ImportDeclaration>),
    RootEnum(EnumDeclaration),
    Types(Vec<Declaration>),
    Type(TypeDeclaration),
    Fields(Vec<FieldDeclaration>),
    Variants(Vec<crate::EnumVariant>),
    Reference(TypeReference),
    References(Vec<TypeReference>),
}

pub struct MacroRegistry {
    macros: Vec<Box<dyn SchemaMacro>>,
    node_definitions: Vec<MacroNodeDefinition>,
}

impl Default for MacroRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MacroRegistry {
    pub fn new() -> Self {
        Self {
            macros: Vec::new(),
            node_definitions: Vec::new(),
        }
    }

    pub fn register(&mut self, schema_macro: impl SchemaMacro + 'static) {
        self.macros.push(Box::new(schema_macro));
    }

    pub fn register_box(&mut self, schema_macro: Box<dyn SchemaMacro>) {
        self.macros.push(schema_macro);
    }

    pub fn register_node_definition(&mut self, definition: MacroNodeDefinition) {
        self.node_definitions.push(definition);
    }

    pub fn node_definition(&self, position: MacroPosition) -> Option<&MacroNodeDefinition> {
        self.node_definitions
            .iter()
            .find(|definition| definition.position == position)
    }

    pub fn node_definitions(&self) -> &[MacroNodeDefinition] {
        &self.node_definitions
    }

    pub fn lower(
        &self,
        object: MacroObject<'_>,
        position: MacroPosition,
        context: &mut MacroContext,
    ) -> Result<MacroOutput, SchemaError> {
        for schema_macro in &self.macros {
            if schema_macro.matches(object, position) {
                return schema_macro.lower(object, position, context, self);
            }
        }
        if position != MacroPosition::TypeReference
            && let Some(definition) = self.node_definition(position)
            && definition.has_cases()
        {
            return Err(definition.unsupported_structure_error(object));
        }
        Err(SchemaError::MacroDidNotMatch {
            macro_name: "registered macro".to_owned(),
        })
    }

    pub fn macro_names(&self) -> Vec<String> {
        self.macros
            .iter()
            .map(|schema_macro| schema_macro.name().to_owned())
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroNodeDefinition {
    position: MacroPosition,
    dispatch: MacroDispatch,
    cases: Vec<MacroNodeCase>,
}

impl MacroNodeDefinition {
    pub fn new(position: MacroPosition, dispatch: MacroDispatch) -> Self {
        Self {
            position,
            dispatch,
            cases: Vec::new(),
        }
    }

    pub fn with_cases(
        position: MacroPosition,
        dispatch: MacroDispatch,
        cases: Vec<MacroNodeCase>,
    ) -> Self {
        Self {
            position,
            dispatch,
            cases,
        }
    }

    pub fn root_imports() -> Self {
        Self::with_cases(
            MacroPosition::RootImports,
            MacroDispatch::RootPositional,
            vec![MacroNodeCase::block(
                "imports map",
                MacroNodeBlockConstraint::new(
                    Some(MacroNodeDelimiter::Brace),
                    MacroNodeObjectCount::Even,
                ),
            )],
        )
    }

    pub fn root_input() -> Self {
        Self::root_enum(MacroPosition::RootInput)
    }

    pub fn root_output() -> Self {
        Self::root_enum(MacroPosition::RootOutput)
    }

    pub fn root_namespace() -> Self {
        Self::with_cases(
            MacroPosition::RootNamespace,
            MacroDispatch::RootPositional,
            vec![MacroNodeCase::block(
                "namespace map",
                MacroNodeBlockConstraint::new(
                    Some(MacroNodeDelimiter::Brace),
                    MacroNodeObjectCount::Even,
                ),
            )],
        )
    }

    pub fn namespace_declaration() -> Self {
        Self::with_cases(
            MacroPosition::NamespaceDeclaration,
            MacroDispatch::Structural,
            vec![
                MacroNodeCase::pair(
                    "struct declaration",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::Symbol,
                        MacroNodeValueConstraint::Delimited(MacroNodeDelimiter::Brace),
                    ),
                ),
                MacroNodeCase::pair(
                    "enum declaration",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::Symbol,
                        MacroNodeValueConstraint::Delimited(MacroNodeDelimiter::SquareBracket),
                    ),
                ),
                MacroNodeCase::pair(
                    "newtype declaration",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::Symbol,
                        MacroNodeValueConstraint::TypeReferenceLike,
                    ),
                ),
            ],
        )
    }

    pub fn struct_fields() -> Self {
        Self::with_cases(
            MacroPosition::StructFields,
            MacroDispatch::Structural,
            vec![
                MacroNodeCase::pair(
                    "explicit field",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::CamelCaseSymbol,
                        MacroNodeValueConstraint::TypeReferenceLike,
                    ),
                ),
                MacroNodeCase::pair(
                    "derived field",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::PascalCaseSymbol,
                        MacroNodeValueConstraint::SameTypeMarker,
                    ),
                ),
            ],
        )
    }

    pub fn enum_variants() -> Self {
        Self::with_cases(
            MacroPosition::EnumVariants,
            MacroDispatch::Structural,
            vec![
                MacroNodeCase::block(
                    "unit variant",
                    MacroNodeBlockConstraint::new(None, MacroNodeObjectCount::Exact(0)),
                ),
                MacroNodeCase::pair(
                    "data variant",
                    MacroNodePairConstraint::new(
                        MacroNodeKeyConstraint::SigilSuffix('@'),
                        MacroNodeValueConstraint::TypeReferenceLike,
                    ),
                ),
            ],
        )
    }

    pub fn type_reference() -> Self {
        Self::with_cases(
            MacroPosition::TypeReference,
            MacroDispatch::StructuralOrTaggedInvocation,
            vec![
                MacroNodeCase::block(
                    "plain or scalar reference",
                    MacroNodeBlockConstraint::new(None, MacroNodeObjectCount::Exact(0)),
                ),
                MacroNodeCase::block(
                    "composite or tagged invocation",
                    MacroNodeBlockConstraint::new(
                        Some(MacroNodeDelimiter::Parenthesis),
                        MacroNodeObjectCount::Any,
                    ),
                ),
            ],
        )
    }

    fn root_enum(position: MacroPosition) -> Self {
        Self::with_cases(
            position,
            MacroDispatch::RootPositional,
            vec![
                MacroNodeCase::block(
                    "root enum body",
                    MacroNodeBlockConstraint::new(
                        Some(MacroNodeDelimiter::SquareBracket),
                        MacroNodeObjectCount::Any,
                    ),
                ),
                MacroNodeCase::block(
                    "legacy named root enum body",
                    MacroNodeBlockConstraint::new(
                        Some(MacroNodeDelimiter::PipeParenthesis),
                        MacroNodeObjectCount::Any,
                    ),
                ),
            ],
        )
    }

    pub fn position(&self) -> MacroPosition {
        self.position
    }

    pub fn dispatch(&self) -> MacroDispatch {
        self.dispatch
    }

    pub fn cases(&self) -> &[MacroNodeCase] {
        &self.cases
    }

    pub fn has_cases(&self) -> bool {
        !self.cases.is_empty()
    }

    pub fn matches(&self, object: MacroObject<'_>) -> bool {
        self.cases.iter().any(|case| case.matches(object))
    }

    pub fn unsupported_structure_error(&self, object: MacroObject<'_>) -> SchemaError {
        SchemaError::UnsupportedMacroNodeStructure {
            position: self.position.as_str().to_owned(),
            expected: self.cases.iter().map(MacroNodeCase::description).collect(),
            found: object.describe(),
        }
    }

    pub fn accepts_tagged_invocation(&self) -> bool {
        matches!(
            self.dispatch,
            MacroDispatch::TaggedInvocation | MacroDispatch::StructuralOrTaggedInvocation
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroDispatch {
    RootPositional,
    Structural,
    TaggedInvocation,
    StructuralOrTaggedInvocation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroNodeCase {
    name: String,
    object: MacroNodeObjectConstraint,
}

impl MacroNodeCase {
    pub fn block(name: impl Into<String>, block: MacroNodeBlockConstraint) -> Self {
        Self {
            name: name.into(),
            object: MacroNodeObjectConstraint::Block(block),
        }
    }

    pub fn pair(name: impl Into<String>, pair: MacroNodePairConstraint) -> Self {
        Self {
            name: name.into(),
            object: MacroNodeObjectConstraint::Pair(pair),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn matches(&self, object: MacroObject<'_>) -> bool {
        self.object.matches(object)
    }

    pub fn description(&self) -> String {
        format!("{}: {}", self.name, self.object.description())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MacroNodeObjectConstraint {
    Block(MacroNodeBlockConstraint),
    Pair(MacroNodePairConstraint),
}

impl MacroNodeObjectConstraint {
    pub fn matches(&self, object: MacroObject<'_>) -> bool {
        match self {
            Self::Block(constraint) => object
                .block()
                .is_some_and(|block| constraint.matches(block)),
            Self::Pair(constraint) => object.pair().is_some_and(|pair| constraint.matches(pair)),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Block(constraint) => constraint.description(),
            Self::Pair(constraint) => constraint.description(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroNodeBlockConstraint {
    delimiter: Option<MacroNodeDelimiter>,
    object_count: MacroNodeObjectCount,
}

impl MacroNodeBlockConstraint {
    pub fn new(delimiter: Option<MacroNodeDelimiter>, object_count: MacroNodeObjectCount) -> Self {
        Self {
            delimiter,
            object_count,
        }
    }

    pub fn matches(&self, block: &Block) -> bool {
        self.delimiter_matches(block) && self.object_count.matches(block.holds_root_objects())
    }

    pub fn description(&self) -> String {
        let delimiter = self
            .delimiter
            .map(|delimiter| delimiter.as_str())
            .unwrap_or("any delimiter or atom");
        format!(
            "block delimiter={delimiter} count={}",
            self.object_count.as_str()
        )
    }

    fn delimiter_matches(&self, block: &Block) -> bool {
        match self.delimiter {
            Some(expected) => MacroNodeDelimiter::from_block(block) == Some(expected),
            None => true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroNodeObjectCount {
    Any,
    Even,
    Exact(usize),
}

impl MacroNodeObjectCount {
    pub fn matches(&self, found: usize) -> bool {
        match self {
            Self::Any => true,
            Self::Even => found % 2 == 0,
            Self::Exact(expected) => found == *expected,
        }
    }

    pub fn as_str(&self) -> String {
        match self {
            Self::Any => "any".to_owned(),
            Self::Even => "even".to_owned(),
            Self::Exact(expected) => expected.to_string(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroNodePairConstraint {
    key: MacroNodeKeyConstraint,
    value: MacroNodeValueConstraint,
}

impl MacroNodePairConstraint {
    pub fn new(key: MacroNodeKeyConstraint, value: MacroNodeValueConstraint) -> Self {
        Self { key, value }
    }

    pub fn matches(&self, pair: MacroPair<'_>) -> bool {
        self.key.matches(pair.name) && self.value.matches(pair.definition)
    }

    pub fn description(&self) -> String {
        format!(
            "pair key={} value={}",
            self.key.description(),
            self.value.description()
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroNodeKeyConstraint {
    Symbol,
    PascalCaseSymbol,
    CamelCaseSymbol,
    SigilSuffix(char),
}

impl MacroNodeKeyConstraint {
    pub fn matches(&self, block: &Block) -> bool {
        let Some(text) = block.demote_to_string() else {
            return false;
        };
        match self {
            Self::Symbol => block.schema_name().is_ok(),
            Self::PascalCaseSymbol => Self::starts_with_case(text, char::is_ascii_uppercase),
            Self::CamelCaseSymbol => Self::starts_with_case(text, char::is_ascii_lowercase),
            Self::SigilSuffix(sigil) => text
                .strip_suffix(*sigil)
                .is_some_and(|name| !name.is_empty()),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Symbol => "symbol".to_owned(),
            Self::PascalCaseSymbol => "PascalCase symbol".to_owned(),
            Self::CamelCaseSymbol => "camelCase symbol".to_owned(),
            Self::SigilSuffix(sigil) => format!("symbol ending with {sigil}"),
        }
    }

    fn starts_with_case(text: &str, predicate: fn(&char) -> bool) -> bool {
        !text.contains('@')
            && text
                .chars()
                .next()
                .is_some_and(|character| predicate(&character))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroNodeValueConstraint {
    Delimited(MacroNodeDelimiter),
    TypeReferenceLike,
    SameTypeMarker,
}

impl MacroNodeValueConstraint {
    pub fn matches(&self, block: &Block) -> bool {
        match self {
            Self::Delimited(expected) => MacroNodeDelimiter::from_block(block) == Some(*expected),
            Self::TypeReferenceLike => Self::is_type_reference_like(block),
            Self::SameTypeMarker => block.demote_to_string() == Some("*"),
        }
    }

    pub fn description(&self) -> String {
        match self {
            Self::Delimited(delimiter) => delimiter.as_str().to_owned(),
            Self::TypeReferenceLike => "type reference".to_owned(),
            Self::SameTypeMarker => "*".to_owned(),
        }
    }

    fn is_type_reference_like(block: &Block) -> bool {
        match block {
            Block::Atom(atom) => atom.qualifies_as_symbol(),
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                ..
            } => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MacroNodeDelimiter {
    Parenthesis,
    SquareBracket,
    Brace,
    PipeParenthesis,
    PipeBrace,
}

impl MacroNodeDelimiter {
    pub fn from_block(block: &Block) -> Option<Self> {
        match block {
            Block::Delimited { delimiter, .. } => Some(Self::from_nota(*delimiter)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Parenthesis => "parenthesis",
            Self::SquareBracket => "square bracket",
            Self::Brace => "brace",
            Self::PipeParenthesis => "pipe parenthesis",
            Self::PipeBrace => "pipe brace",
        }
    }

    fn from_nota(delimiter: Delimiter) -> Self {
        match delimiter {
            Delimiter::Parenthesis => Self::Parenthesis,
            Delimiter::SquareBracket => Self::SquareBracket,
            Delimiter::Brace => Self::Brace,
            Delimiter::PipeParenthesis => Self::PipeParenthesis,
            Delimiter::PipeBrace => Self::PipeBrace,
        }
    }
}

pub(crate) trait BlockDebug {
    fn reemit_fallback(&self) -> String;
}

pub(crate) trait SchemaBlockExt {
    fn schema_name(&self) -> Result<Name, SchemaError>;
}

impl BlockDebug for Block {
    fn reemit_fallback(&self) -> String {
        self.demote_to_string()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{self:?}"))
    }
}

impl SchemaBlockExt for Block {
    fn schema_name(&self) -> Result<Name, SchemaError> {
        self.atom()
            .filter(|atom| atom.qualifies_as_symbol())
            .map(|atom| Name::new(atom.text()))
            .ok_or_else(|| SchemaError::ExpectedSymbol {
                found: self.reemit_fallback(),
            })
    }
}
