use std::fmt;

use nota_next::Block;

use crate::{SchemaError, macros::SchemaBlockExt};

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
/// the positions that previously only held a name. The macro surface
/// forms are positional, collection-name-first — `(Vec T)`,
/// `(KeyValue K V)`, `(Option T)` — per psyche records 1034 / 1045.
/// `Map` is the schema-level ordered key-value collection; the
/// concrete Rust container is the emitter's concern, not this model's.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeReference {
    Plain(Name),
    Vector(Box<TypeReference>),
    Map(Box<TypeReference>, Box<TypeReference>),
    Optional(Box<TypeReference>),
}

impl TypeReference {
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
    /// lowers to `Plain`. A parenthesised head-symbol form lowers to a
    /// collection: `(Vec T)` → `Vector`, `(KeyValue K V)` → `Map`,
    /// `(Option T)` → `Optional`. The inner positions recurse, so
    /// `(Vec (Option Topic))` and `(KeyValue NodeName (Vec Service))`
    /// nest. The collection head is a positional symbol, name-first,
    /// per psyche record 1034 — it is NOT a keyword-tagged record; the
    /// head names the collection and the remaining positions are its
    /// element types. nota-next did the structural parse; this is pure
    /// semantic lowering over its `Block`s, not a hand-rolled text
    /// parser.
    pub fn from_block(block: &Block) -> Result<Self, SchemaError> {
        if block.is_parenthesis() {
            return Self::from_parenthesis(block);
        }
        Ok(Self::Plain(block.schema_name()?))
    }

    fn from_parenthesis(block: &Block) -> Result<Self, SchemaError> {
        let head = block
            .root_object_at(0)
            .ok_or(SchemaError::EmptyTypeReference)?
            .schema_name()?;
        let argument_count = block.holds_root_objects() - 1;
        match (head.as_str(), argument_count) {
            ("Vec", 1) => Ok(Self::Vector(Box::new(Self::from_block(
                block.root_object_at(1).expect("argument count checked"),
            )?))),
            ("Option", 1) => Ok(Self::Optional(Box::new(Self::from_block(
                block.root_object_at(1).expect("argument count checked"),
            )?))),
            ("KeyValue", 2) => Ok(Self::Map(
                Box::new(Self::from_block(
                    block.root_object_at(1).expect("argument count checked"),
                )?),
                Box::new(Self::from_block(
                    block.root_object_at(2).expect("argument count checked"),
                )?),
            )),
            (head, argument_count) => Err(SchemaError::UnknownTypeReferenceForm {
                head: head.to_owned(),
                argument_count,
            }),
        }
    }
}
