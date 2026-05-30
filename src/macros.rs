use nota_next::{
    AtomShape, Block, CaptureName, DelimitedShape, MacroCandidate, MacroDelimiter,
    MacroNodeDefinition as NotaMacroNodeDefinition, MacroObjectCount as NotaMacroObjectCount,
    MacroRegistry as NotaMacroRegistry, Pattern, PatternElement, PositionPredicate, SigilSpec,
    StructureHeader,
};

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

    pub fn position_predicate(&self) -> PositionPredicate {
        PositionPredicate::named(self.as_str())
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

    pub fn macro_candidate(self, position: MacroPosition) -> MacroCandidate<'object> {
        match self {
            Self::Block(block) => MacroCandidate::from_block(position.position_predicate(), block),
            Self::Pair(pair) => {
                MacroCandidate::from_pair(position.position_predicate(), pair.name, pair.definition)
            }
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
    cases: Vec<NotaMacroNodeDefinition>,
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
        cases: Vec<NotaMacroNodeDefinition>,
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
            vec![Self::block_case(
                MacroPosition::RootImports,
                "imports map",
                MacroDelimiter::Brace,
                NotaMacroObjectCount::Even,
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
            vec![Self::block_case(
                MacroPosition::RootNamespace,
                "namespace map",
                MacroDelimiter::Brace,
                NotaMacroObjectCount::Even,
            )],
        )
    }

    pub fn namespace_declaration() -> Self {
        Self::with_cases(
            MacroPosition::NamespaceDeclaration,
            MacroDispatch::Structural,
            vec![
                Self::pair_case(
                    MacroPosition::NamespaceDeclaration,
                    "struct declaration",
                    PatternElement::atom(AtomShape::symbol(Some(CaptureName::new("type_name")))),
                    PatternElement::delimited(DelimitedShape::new(
                        MacroDelimiter::Brace,
                        NotaMacroObjectCount::Any,
                        Some(CaptureName::new("body")),
                    )),
                    "symbol key followed by brace value",
                ),
                Self::pair_case(
                    MacroPosition::NamespaceDeclaration,
                    "enum declaration",
                    PatternElement::atom(AtomShape::symbol(Some(CaptureName::new("type_name")))),
                    PatternElement::delimited(DelimitedShape::new(
                        MacroDelimiter::SquareBracket,
                        NotaMacroObjectCount::Any,
                        Some(CaptureName::new("body")),
                    )),
                    "symbol key followed by square bracket value",
                ),
                Self::pair_case(
                    MacroPosition::NamespaceDeclaration,
                    "newtype declaration",
                    PatternElement::atom(AtomShape::symbol(Some(CaptureName::new("type_name")))),
                    PatternElement::any(Some(CaptureName::new("reference"))),
                    "symbol key followed by type reference value",
                ),
            ],
        )
    }

    pub fn struct_fields() -> Self {
        Self::with_cases(
            MacroPosition::StructFields,
            MacroDispatch::Structural,
            vec![
                Self::pair_case(
                    MacroPosition::StructFields,
                    "explicit field",
                    PatternElement::atom(AtomShape::camel_case(Some(CaptureName::new(
                        "field_name",
                    )))),
                    PatternElement::any(Some(CaptureName::new("reference"))),
                    "camelCase field key followed by type reference value",
                ),
                Self::pair_case(
                    MacroPosition::StructFields,
                    "derived field",
                    PatternElement::atom(AtomShape::pascal_case(Some(CaptureName::new(
                        "type_name",
                    )))),
                    PatternElement::literal("*"),
                    "PascalCase type key followed by * marker",
                ),
            ],
        )
    }

    pub fn enum_variants() -> Self {
        Self::with_cases(
            MacroPosition::EnumVariants,
            MacroDispatch::Structural,
            vec![
                NotaMacroNodeDefinition::new(
                    "unit variant",
                    MacroPosition::EnumVariants.position_predicate(),
                    Pattern::new(vec![PatternElement::atom(AtomShape::pascal_case(Some(
                        CaptureName::new("variant_name"),
                    )))]),
                    "PascalCase variant atom",
                ),
                Self::pair_case(
                    MacroPosition::EnumVariants,
                    "data variant",
                    PatternElement::atom(
                        AtomShape::pascal_case(Some(CaptureName::new("variant_name")))
                            .with_sigil(SigilSpec::suffix("@")),
                    ),
                    PatternElement::any(Some(CaptureName::new("payload"))),
                    "PascalCase@ variant key followed by payload type",
                ),
            ],
        )
    }

    pub fn type_reference() -> Self {
        Self::with_cases(
            MacroPosition::TypeReference,
            MacroDispatch::StructuralOrTaggedInvocation,
            vec![
                NotaMacroNodeDefinition::new(
                    "plain or scalar reference",
                    MacroPosition::TypeReference.position_predicate(),
                    Pattern::new(vec![PatternElement::atom(AtomShape::symbol(Some(
                        CaptureName::new("reference"),
                    )))]),
                    "symbol reference atom",
                ),
                Self::block_case(
                    MacroPosition::TypeReference,
                    "composite or tagged invocation",
                    MacroDelimiter::Parenthesis,
                    NotaMacroObjectCount::Any,
                ),
            ],
        )
    }

    fn root_enum(position: MacroPosition) -> Self {
        Self::with_cases(
            position,
            MacroDispatch::RootPositional,
            vec![
                Self::block_case(
                    position,
                    "root enum body",
                    MacroDelimiter::SquareBracket,
                    NotaMacroObjectCount::Any,
                ),
                Self::block_case(
                    position,
                    "legacy named root enum body",
                    MacroDelimiter::PipeParenthesis,
                    NotaMacroObjectCount::Any,
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

    pub fn cases(&self) -> &[NotaMacroNodeDefinition] {
        &self.cases
    }

    pub fn has_cases(&self) -> bool {
        !self.cases.is_empty()
    }

    pub fn matches(&self, object: MacroObject<'_>) -> bool {
        NotaMacroRegistry::unchecked(self.cases.clone())
            .dispatch(&object.macro_candidate(self.position))
            .is_ok()
    }

    pub fn unsupported_structure_error(&self, object: MacroObject<'_>) -> SchemaError {
        let error = NotaMacroRegistry::unchecked(self.cases.clone())
            .dispatch(&object.macro_candidate(self.position))
            .expect_err("unsupported structure checked after no schema macro matched");
        match error {
            nota_next::MacroError::NoMatch {
                expected, found, ..
            } => SchemaError::UnsupportedMacroNodeStructure {
                position: self.position.as_str().to_owned(),
                expected,
                found,
            },
            nota_next::MacroError::Conflict(conflict) => {
                SchemaError::UnsupportedMacroNodeStructure {
                    position: self.position.as_str().to_owned(),
                    expected: vec![format!(
                        "non-conflicting macro cases, found conflict between {} and {}",
                        conflict.first(),
                        conflict.second()
                    )],
                    found: object.describe(),
                }
            }
        }
    }

    pub fn accepts_tagged_invocation(&self) -> bool {
        matches!(
            self.dispatch,
            MacroDispatch::TaggedInvocation | MacroDispatch::StructuralOrTaggedInvocation
        )
    }

    fn block_case(
        position: MacroPosition,
        name: impl Into<String>,
        delimiter: MacroDelimiter,
        object_count: NotaMacroObjectCount,
    ) -> NotaMacroNodeDefinition {
        let delimiter_name = delimiter.as_str();
        NotaMacroNodeDefinition::new(
            name,
            position.position_predicate(),
            Pattern::new(vec![PatternElement::delimited(DelimitedShape::new(
                delimiter,
                object_count,
                Some(CaptureName::new("body")),
            ))]),
            format!("{delimiter_name} block"),
        )
    }

    fn pair_case(
        position: MacroPosition,
        name: impl Into<String>,
        key: PatternElement,
        value: PatternElement,
        expected: impl Into<String>,
    ) -> NotaMacroNodeDefinition {
        NotaMacroNodeDefinition::new(
            name,
            position.position_predicate(),
            Pattern::new(vec![key, value]),
            expected,
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
