use nota_next::{Block, Delimiter, Document};

use crate::{
    EnumDeclaration, EnumVariant, FieldDeclaration, MacroContext, MacroObject, MacroOutput,
    MacroPair, MacroPosition, MacroRegistry, Name, SchemaError, SchemaMacro, StructDeclaration,
    TypeDeclaration, TypeReference, macros::SchemaBlockExt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclarativeMacroLibrary {
    definitions: Vec<MacroDefinition>,
}

impl DeclarativeMacroLibrary {
    pub fn builtin() -> Result<Self, SchemaError> {
        Self::from_source(include_str!("../schemas/builtin-macros.schema"))
    }

    pub fn from_source(source: &str) -> Result<Self, SchemaError> {
        let document = Document::parse(source)?;
        let mut definitions = Vec::new();
        for object in document.root_objects() {
            definitions.push(MacroDefinition::from_block(object)?);
        }
        Ok(Self { definitions })
    }

    pub fn definitions(&self) -> &[MacroDefinition] {
        &self.definitions
    }

    pub fn into_macros(self) -> Vec<Box<dyn SchemaMacro>> {
        self.definitions
            .into_iter()
            .map(|definition| {
                Box::new(DeclarativeSchemaMacro { definition }) as Box<dyn SchemaMacro>
            })
            .collect()
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
        let record = MacroDefinitionRecord::new(object)?;
        Ok(Self {
            name: record.name()?,
            position: record.position()?,
            pattern: record.pattern()?,
            template: record.template()?,
        })
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn position(&self) -> MacroPosition {
        self.position
    }

    pub fn capture_names(&self) -> Vec<String> {
        self.pattern.capture_names()
    }
}

#[derive(Clone, Copy, Debug)]
struct MacroDefinitionRecord<'schema> {
    object: &'schema Block,
}

impl<'schema> MacroDefinitionRecord<'schema> {
    fn new(object: &'schema Block) -> Result<Self, SchemaError> {
        let record = Self { object };
        if !object.is_parenthesis() || object.holds_root_objects() != 5 {
            return Err(SchemaError::ExpectedMacroDefinition {
                found: NotationBlock::new(object).compact_notation(),
            });
        }
        if record.child(0).schema_name()?.as_str() != "SchemaMacro" {
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
            Self::Capture(capture) => bindings.bind_single(
                capture.name(),
                NotationBlock::new(object).compact_notation(),
            ),
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
        bindings.bind_repeated(
            capture.name(),
            objects[before..repeated_end]
                .iter()
                .map(|object| NotationBlock::new(object).compact_notation())
                .collect(),
        )?;
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

    fn expand(&self, bindings: &MacroBindings) -> Result<ExpandedTemplate, SchemaError> {
        let mut pieces = self.object.expand_notations(bindings)?;
        let source = pieces
            .pop()
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: String::new(),
            })?;
        if !pieces.is_empty() {
            return Err(SchemaError::UnknownAssembledTemplate { found: source });
        }
        Ok(ExpandedTemplate { source })
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

    fn expand_notations(&self, bindings: &MacroBindings) -> Result<Vec<String>, SchemaError> {
        match self {
            Self::Capture(capture) => Ok(vec![bindings.single(capture.name())?.to_owned()]),
            Self::RestCapture(capture) => Ok(bindings.repeated(capture.name())?.to_vec()),
            Self::Atom(text) => Ok(vec![text.clone()]),
            Self::Delimited {
                delimiter,
                children,
            } => {
                let mut child_notations = Vec::new();
                for child in children {
                    child_notations.extend(child.expand_notations(bindings)?);
                }
                Ok(vec![
                    DelimitedNotation::new(*delimiter).wrap_children(&child_notations),
                ])
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
    fn bind_single(&mut self, name: &str, value: String) -> Result<bool, SchemaError> {
        if let Some(existing) = self.singles.iter().find(|binding| binding.name == name) {
            return Ok(existing.value == value);
        }
        self.singles.push(SingleMacroBinding {
            name: name.to_owned(),
            value,
        });
        Ok(true)
    }

    fn bind_repeated(&mut self, name: &str, values: Vec<String>) -> Result<(), SchemaError> {
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
            values,
        });
        Ok(())
    }

    fn single(&self, name: &str) -> Result<&str, SchemaError> {
        self.singles
            .iter()
            .find(|binding| binding.name == name)
            .map(|binding| binding.value.as_str())
            .ok_or_else(|| SchemaError::MissingMacroBinding {
                name: name.to_owned(),
            })
    }

    fn repeated(&self, name: &str) -> Result<&[String], SchemaError> {
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
    value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RepeatedMacroBinding {
    name: String,
    values: Vec<String>,
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
        let document = Document::parse(&self.source)?;
        if document.holds_root_objects() != 1 {
            return Err(SchemaError::UnknownAssembledTemplate {
                found: self.source.clone(),
            });
        }
        AssembledTemplate::new(
            document
                .root_object_at(0)
                .expect("expanded template root count checked"),
        )
        .lower(registry, context)
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledTemplate<'template> {
    object: &'template Block,
}

impl<'template> AssembledTemplate<'template> {
    fn new(object: &'template Block) -> Self {
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
                found: NotationBlock::new(self.object).compact_notation(),
            })?
            .schema_name()?;
        match head.as_str() {
            "Type" => AssembledType::new(self.child(children, 1, "Type")?)
                .lower(registry, context)
                .map(MacroOutput::Type),
            "Fields" => AssembledFields::new(&children[1..])
                .lower(registry, context)
                .map(MacroOutput::Fields),
            "Variants" => AssembledVariants::new(&children[1..])
                .lower(registry, context)
                .map(MacroOutput::Variants),
            "Reference" => AssembledReference::new(&children[1..])
                .lower(registry, context)
                .map(MacroOutput::Reference),
            found => Err(SchemaError::UnknownAssembledTemplate {
                found: found.to_owned(),
            }),
        }
    }

    fn child(
        &self,
        children: &'template [Block],
        index: usize,
        template_name: &'static str,
    ) -> Result<&'template Block, SchemaError> {
        children
            .get(index)
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: template_name.to_owned(),
            })
    }

    fn parenthesized_children(
        &self,
        expected: &'static str,
    ) -> Result<&'template [Block], SchemaError> {
        match self.object {
            Block::Delimited {
                delimiter: Delimiter::Parenthesis,
                root_objects,
                ..
            } => Ok(root_objects),
            _ => Err(SchemaError::ExpectedDelimiter { expected }),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledType<'template> {
    object: &'template Block,
}

impl<'template> AssembledType<'template> {
    fn new(object: &'template Block) -> Self {
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
            "Struct" => self.lower_struct(children, registry, context),
            "Enum" => self.lower_enum(children, registry, context),
            found => Err(SchemaError::UnknownAssembledTemplate {
                found: found.to_owned(),
            }),
        }
    }

    fn lower_struct(
        &self,
        children: &'template [Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.child(children, 1, "Struct")?.schema_name()?;
        let body = self.child(children, 2, "Struct")?;
        let fields = match registry.lower(
            MacroObject::Block(body),
            MacroPosition::StructFields,
            context,
        )? {
            MacroOutput::Fields(fields) => fields,
            _ => {
                return Err(SchemaError::UnexpectedMacroOutput {
                    macro_name: "StructFields".to_owned(),
                    expected: "fields",
                });
            }
        };
        let declaration = StructDeclaration { name, fields };
        if declaration.fields.len() == 1 {
            Ok(TypeDeclaration::Newtype(declaration))
        } else {
            Ok(TypeDeclaration::Struct(declaration))
        }
    }

    fn lower_enum(
        &self,
        children: &'template [Block],
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeDeclaration, SchemaError> {
        let name = self.child(children, 1, "Enum")?.schema_name()?;
        let body = self.child(children, 2, "Enum")?;
        let variants = match registry.lower(
            MacroObject::Block(body),
            MacroPosition::EnumVariants,
            context,
        )? {
            MacroOutput::Variants(variants) => variants,
            _ => {
                return Err(SchemaError::UnexpectedMacroOutput {
                    macro_name: "EnumVariants".to_owned(),
                    expected: "variants",
                });
            }
        };
        Ok(TypeDeclaration::Enum(EnumDeclaration { name, variants }))
    }

    fn child(
        &self,
        children: &'template [Block],
        index: usize,
        template_name: &'static str,
    ) -> Result<&'template Block, SchemaError> {
        children
            .get(index)
            .ok_or_else(|| SchemaError::UnknownAssembledTemplate {
                found: template_name.to_owned(),
            })
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledFields<'template> {
    objects: &'template [Block],
}

impl<'template> AssembledFields<'template> {
    fn new(objects: &'template [Block]) -> Self {
        Self { objects }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<FieldDeclaration>, SchemaError> {
        let mut fields = Vec::new();
        for object in self.objects {
            fields.push(AssembledField::new(object).lower(registry, context)?);
        }
        Ok(fields)
    }
}

/// One field inside a struct body.
///
/// A bare PascalCase symbol (`Topic`) derives the field name from the
/// type name (`topic`) and creates a `Plain` reference. Native NOTA
/// type-reference objects can also sit directly in a field position:
/// `(Vec Topic)`, `(Map (Topic RecordIdentifier))`, and
/// `(Optional Topic)` lower to vector, map, and optional references
/// with names derived from the reference shape. A parenthesised pair
/// whose first object is a
/// lower-case field symbol remains the explicit escape hatch for
/// uncommon names.
#[derive(Clone, Copy, Debug)]
struct AssembledField<'template> {
    object: &'template Block,
}

impl<'template> AssembledField<'template> {
    fn new(object: &'template Block) -> Self {
        Self { object }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<FieldDeclaration, SchemaError> {
        if self.is_explicit_field_pair() {
            let field_name = self
                .object
                .root_object_at(0)
                .expect("count checked")
                .schema_name()?;
            let reference = TypeReference::from_block_with_registry(
                self.object.root_object_at(1).expect("count checked"),
                registry,
                context,
            )?;
            return Ok(FieldDeclaration {
                name: Name::new(field_name.field_name()),
                reference,
            });
        }
        if !matches!(self.object, Block::Atom(_)) {
            let reference =
                TypeReference::from_block_with_registry(self.object, registry, context)?;
            return Ok(FieldDeclaration {
                name: self.derived_name_for_reference(&reference),
                reference,
            });
        }
        let name = self.object.schema_name()?;
        Ok(FieldDeclaration {
            name: Name::new(name.field_name()),
            reference: TypeReference::Plain(name),
        })
    }

    fn is_explicit_field_pair(&self) -> bool {
        self.object.is_parenthesis()
            && self.object.holds_root_objects() == 2
            && self
                .object
                .root_object_at(0)
                .and_then(Block::demote_to_string)
                .is_some_and(|name| {
                    name.chars()
                        .next()
                        .is_some_and(|character| character.is_ascii_lowercase())
                })
    }

    fn derived_name_for_reference(&self, reference: &TypeReference) -> Name {
        match reference {
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

#[derive(Clone, Copy, Debug)]
struct AssembledVariants<'template> {
    objects: &'template [Block],
}

impl<'template> AssembledVariants<'template> {
    fn new(objects: &'template [Block]) -> Self {
        Self { objects }
    }

    fn lower(
        &self,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<Vec<EnumVariant>, SchemaError> {
        let mut variants = Vec::new();
        for object in self.objects {
            variants.push(AssembledVariant::new(object).lower(registry, context)?);
        }
        Ok(variants)
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledVariant<'template> {
    object: &'template Block,
}

impl<'template> AssembledVariant<'template> {
    fn new(object: &'template Block) -> Self {
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
            1 => Ok(EnumVariant {
                name: self
                    .object
                    .root_object_at(0)
                    .expect("count checked")
                    .schema_name()?,
                payload: None,
            }),
            2 => Ok(EnumVariant {
                name: self
                    .object
                    .root_object_at(0)
                    .expect("count checked")
                    .schema_name()?,
                payload: Some(TypeReference::from_block_with_registry(
                    self.object.root_object_at(1).expect("count checked"),
                    registry,
                    context,
                )?),
            }),
            _ => Err(SchemaError::ExpectedEnumVariant),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct AssembledReference<'template> {
    objects: &'template [Block],
}

impl<'template> AssembledReference<'template> {
    fn new(objects: &'template [Block]) -> Self {
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
        Self::lower_object(&self.objects[0], registry, context)
    }

    fn lower_object(
        object: &'template Block,
        registry: &MacroRegistry,
        context: &mut MacroContext,
    ) -> Result<TypeReference, SchemaError> {
        if !object.is_parenthesis() {
            return TypeReference::from_block_with_registry(object, registry, context);
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
                Self::lower_object(&children[1], registry, context)?,
            ))),
            "Optional" if children.len() == 2 => Ok(TypeReference::Optional(Box::new(
                Self::lower_object(&children[1], registry, context)?,
            ))),
            "Map" if children.len() == 3 => Ok(TypeReference::Map(
                Box::new(Self::lower_object(&children[1], registry, context)?),
                Box::new(Self::lower_object(&children[2], registry, context)?),
            )),
            _ => TypeReference::from_block_with_registry(object, registry, context),
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
