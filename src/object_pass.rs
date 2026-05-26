use std::{fs, path::Path};

use nota_codec::{NotaValue, NotaValueKind, parse_sequence};

use crate::{Error, ModuleName, Result};

/// First-pass schema object reader.
///
/// This pass intentionally knows only NOTA object shape and the schema file's
/// namespace prefix. It does not assume the old six-position schema shape, and
/// it does not lower features. Later schema macro passes interpret these roots
/// according to their delimiter, position, and namespace context.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaObjectPass {
    namespace_prefix: ModuleName,
    roots: Vec<SchemaRootObject>,
}

impl SchemaObjectPass {
    pub fn parse_text(namespace_prefix: ModuleName, text: &str) -> Result<Self> {
        let values = parse_sequence(text).map_err(|error| Error::InvalidSchemaText {
            context: "schema object pass",
            message: error.to_string(),
        })?;

        Ok(Self {
            namespace_prefix,
            roots: values
                .into_iter()
                .enumerate()
                .map(|(position, value)| SchemaRootObject::new(position, value))
                .collect(),
        })
    }

    pub fn read_path(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = fs::read_to_string(path).map_err(|error| Error::SchemaReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        Self::parse_text(ModuleName::from_schema_path(path)?, &text)
    }

    pub fn namespace_prefix(&self) -> &ModuleName {
        &self.namespace_prefix
    }

    pub fn roots(&self) -> &[SchemaRootObject] {
        &self.roots
    }

    pub fn namespace_roots(&self) -> impl Iterator<Item = &SchemaRootObject> {
        self.roots
            .iter()
            .filter(|root| root.delimiter() == ObjectDelimiter::CurlyBraces)
    }

    pub fn sequence_roots(&self) -> impl Iterator<Item = &SchemaRootObject> {
        self.roots
            .iter()
            .filter(|root| root.delimiter() == ObjectDelimiter::SquareBrackets)
    }

    pub fn record_roots(&self) -> impl Iterator<Item = &SchemaRootObject> {
        self.roots
            .iter()
            .filter(|root| root.delimiter() == ObjectDelimiter::Parentheses)
    }

    pub fn all_objects(&self) -> Vec<SchemaObjectNode> {
        self.roots
            .iter()
            .flat_map(SchemaRootObject::object_nodes)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SchemaRootObject {
    position: usize,
    delimiter: ObjectDelimiter,
    value: NotaValue,
}

impl SchemaRootObject {
    fn new(position: usize, value: NotaValue) -> Self {
        Self {
            position,
            delimiter: ObjectDelimiter::from_value(&value),
            value,
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn delimiter(&self) -> ObjectDelimiter {
        self.delimiter
    }

    pub fn value(&self) -> &NotaValue {
        &self.value
    }

    pub fn namespace_entries(&self) -> Result<Vec<NamespaceObject<'_>>> {
        let Some(entries) = self.value.as_map() else {
            return Err(Error::InvalidSchemaText {
                context: "schema object pass namespace",
                message: format!(
                    "root {} is {:?}, expected CurlyBraces namespace map",
                    self.position, self.delimiter
                ),
            });
        };

        Ok(entries
            .iter()
            .enumerate()
            .map(|(position, entry)| NamespaceObject {
                position,
                name: entry.key(),
                value: entry.value(),
                delimiter: ObjectDelimiter::from_value(entry.value()),
            })
            .collect())
    }

    pub fn object_nodes(&self) -> Vec<SchemaObjectNode> {
        let mut nodes = Vec::new();
        walk_value(
            self.value(),
            ObjectPath::root(self.position),
            ObjectPosition::Root,
            &mut nodes,
        );
        nodes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectDelimiter {
    Parentheses,
    SquareBrackets,
    CurlyBraces,
    Atom,
}

impl ObjectDelimiter {
    pub fn from_value(value: &NotaValue) -> Self {
        match value.kind() {
            NotaValueKind::Record => Self::Parentheses,
            NotaValueKind::Sequence => Self::SquareBrackets,
            NotaValueKind::Map => Self::CurlyBraces,
            NotaValueKind::Identifier
            | NotaValueKind::InlineString
            | NotaValueKind::BlockString
            | NotaValueKind::Bytes
            | NotaValueKind::Integer
            | NotaValueKind::UnsignedInteger
            | NotaValueKind::Float
            | NotaValueKind::Date
            | NotaValueKind::Time => Self::Atom,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NamespaceObject<'value> {
    position: usize,
    name: &'value str,
    value: &'value NotaValue,
    delimiter: ObjectDelimiter,
}

impl<'value> NamespaceObject<'value> {
    pub fn position(&self) -> usize {
        self.position
    }

    pub fn name(&self) -> &'value str {
        self.name
    }

    pub fn value(&self) -> &'value NotaValue {
        self.value
    }

    pub fn delimiter(&self) -> ObjectDelimiter {
        self.delimiter
    }

    pub fn identifier_vector(&self) -> Option<Vec<&'value str>> {
        identifier_vector(self.value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SchemaObjectNode {
    path: ObjectPath,
    position: ObjectPosition,
    delimiter: ObjectDelimiter,
    head: Option<String>,
    identifier_vector: Option<Vec<String>>,
}

impl SchemaObjectNode {
    pub fn path(&self) -> &ObjectPath {
        &self.path
    }

    pub fn position(&self) -> ObjectPosition {
        self.position
    }

    pub fn delimiter(&self) -> ObjectDelimiter {
        self.delimiter
    }

    pub fn head(&self) -> Option<&str> {
        self.head.as_deref()
    }

    pub fn identifier_vector(&self) -> Option<&[String]> {
        self.identifier_vector.as_deref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectPath(Vec<ObjectPathSegment>);

impl ObjectPath {
    fn root(position: usize) -> Self {
        Self(vec![ObjectPathSegment::Root(position)])
    }

    fn child(&self, segment: ObjectPathSegment) -> Self {
        let mut path = self.0.clone();
        path.push(segment);
        Self(path)
    }

    pub fn segments(&self) -> &[ObjectPathSegment] {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObjectPathSegment {
    Root(usize),
    RecordItem(usize),
    SequenceItem(usize),
    MapValue(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectPosition {
    Root,
    RecordItem,
    SequenceItem,
    MapValue,
}

pub fn identifier_vector(value: &NotaValue) -> Option<Vec<&str>> {
    let values = value.as_sequence()?;
    values
        .iter()
        .map(NotaValue::identifier_text)
        .collect::<Option<Vec<_>>>()
}

fn walk_value(
    value: &NotaValue,
    path: ObjectPath,
    position: ObjectPosition,
    nodes: &mut Vec<SchemaObjectNode>,
) {
    nodes.push(SchemaObjectNode {
        path: path.clone(),
        position,
        delimiter: ObjectDelimiter::from_value(value),
        head: value.record_head().map(str::to_owned),
        identifier_vector: identifier_vector(value)
            .map(|items| items.into_iter().map(str::to_owned).collect()),
    });

    if let Some(items) = value.as_record() {
        for (index, item) in items.iter().enumerate() {
            walk_value(
                item,
                path.child(ObjectPathSegment::RecordItem(index)),
                ObjectPosition::RecordItem,
                nodes,
            );
        }
        return;
    }

    if let Some(items) = value.as_sequence() {
        for (index, item) in items.iter().enumerate() {
            walk_value(
                item,
                path.child(ObjectPathSegment::SequenceItem(index)),
                ObjectPosition::SequenceItem,
                nodes,
            );
        }
        return;
    }

    if let Some(entries) = value.as_map() {
        for entry in entries {
            walk_value(
                entry.value(),
                path.child(ObjectPathSegment::MapValue(entry.key().to_owned())),
                ObjectPosition::MapValue,
                nodes,
            );
        }
    }
}
