//! Three-part schema reader.
//!
//! Per /353 §3 + record 751, every schema has the canonical layout:
//!
//!   1. `{...}`  — Specifying: import map.
//!   2. `[...]`  — Input header: operations in variant order.
//!   3. `[...]`  — Input payload extras (vector of payload variants).
//!   4. `{...}`  — Namespace: user-defined types (key-value map).
//!   5. `[...]`  — Output: replies / events.
//!
//! Blocks 2/3 + 5 may be empty (`[]`). The namespace (block 4) is the
//! only block that MUST carry content for the schema to be useful.
//!
//! The reader builds an `AssembledSchema` — a flat namespace table
//! plus operation lists. Schema interpretation downstream (codec
//! emission, persona-spirit dispatch, etc.) consumes the assembled
//! form, never the raw `Node` tree.
//!
//! This module does NOT carry effect tables, fan-out targets, or
//! storage descriptors. Schema declares data types only (records
//! 730-732).

use crate::kernel::{Kernel, KernelError, Node, NodeKind};
use core::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    Kernel(KernelError),
    /// The schema document didn't have the canonical five-block layout.
    WrongLayout {
        found_blocks: usize,
        expected: usize,
    },
    /// Specifying block was not a `{...}` map.
    SpecifyingNotMap,
    /// Input header block was not a `[...]` vector.
    InputHeaderNotVector,
    /// Input extras block was not a `[...]` vector.
    InputExtrasNotVector,
    /// Namespace block was not a `{...}` map.
    NamespaceNotMap,
    /// Output block was not a `[...]` vector.
    OutputNotVector,
    /// Namespace map had an odd number of entries (key without value).
    NamespaceUnpaired,
    /// A namespace key was not an identifier.
    NamespaceKeyNotIdentifier,
    /// An operation entry was malformed.
    MalformedOperation,
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::Kernel(error) => write!(formatter, "kernel: {error}"),
            SchemaError::WrongLayout {
                found_blocks,
                expected,
            } => write!(
                formatter,
                "wrong layout: found {found_blocks} top-level blocks, expected {expected}"
            ),
            SchemaError::SpecifyingNotMap => {
                write!(formatter, "specifying block must be a `{{...}}` map")
            }
            SchemaError::InputHeaderNotVector => {
                write!(formatter, "input header must be a `[...]` vector")
            }
            SchemaError::InputExtrasNotVector => {
                write!(formatter, "input extras must be a `[...]` vector")
            }
            SchemaError::NamespaceNotMap => {
                write!(formatter, "namespace block must be a `{{...}}` map")
            }
            SchemaError::OutputNotVector => {
                write!(formatter, "output block must be a `[...]` vector")
            }
            SchemaError::NamespaceUnpaired => {
                write!(formatter, "namespace map has unpaired entries")
            }
            SchemaError::NamespaceKeyNotIdentifier => {
                write!(formatter, "namespace key must be an identifier")
            }
            SchemaError::MalformedOperation => write!(formatter, "malformed operation entry"),
        }
    }
}

impl std::error::Error for SchemaError {}

impl From<KernelError> for SchemaError {
    fn from(error: KernelError) -> Self {
        SchemaError::Kernel(error)
    }
}

/// Which of the three conceptual parts a section belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaSection {
    /// Block 1: imports / exports.
    Specifying,
    /// Blocks 2 + 3: header (variant order) plus payload extras.
    Input,
    /// Block 5: replies / events.
    Output,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreePartSchema {
    pub specifying: Vec<Node>,
    pub input_header: Vec<Node>,
    pub input_extras: Vec<Node>,
    pub namespace: Vec<Node>,
    pub output: Vec<Node>,
}

impl ThreePartSchema {
    /// Read a schema source string into the three-part view.
    pub fn read(source: &str) -> Result<Self, SchemaError> {
        let mut kernel = Kernel::new(source);
        let blocks = kernel.parse_sequence()?;
        if blocks.len() != 5 {
            return Err(SchemaError::WrongLayout {
                found_blocks: blocks.len(),
                expected: 5,
            });
        }
        let [specifying, input_header, input_extras, namespace, output] =
            match <[Node; 5]>::try_from(blocks) {
                Ok(array) => array,
                Err(other) => {
                    return Err(SchemaError::WrongLayout {
                        found_blocks: other.len(),
                        expected: 5,
                    });
                }
            };
        if !matches!(specifying.kind, NodeKind::Map) {
            return Err(SchemaError::SpecifyingNotMap);
        }
        if !matches!(input_header.kind, NodeKind::Vector) {
            return Err(SchemaError::InputHeaderNotVector);
        }
        if !matches!(input_extras.kind, NodeKind::Vector) {
            return Err(SchemaError::InputExtrasNotVector);
        }
        if !matches!(namespace.kind, NodeKind::Map) {
            return Err(SchemaError::NamespaceNotMap);
        }
        if !matches!(output.kind, NodeKind::Vector) {
            return Err(SchemaError::OutputNotVector);
        }
        Ok(Self {
            specifying: specifying.children,
            input_header: input_header.children,
            input_extras: input_extras.children,
            namespace: namespace.children,
            output: output.children,
        })
    }

    /// Whether this schema declares an operation surface (Input
    /// section has variants).
    pub fn has_input(&self) -> bool {
        !self.input_header.is_empty()
    }

    /// Whether this schema declares an output surface (Output
    /// section has variants).
    pub fn has_output(&self) -> bool {
        !self.output.is_empty()
    }
}

/// A namespace entry — one user-defined type binding. The body is
/// kept structural so downstream consumers (codec emitter, macro
/// engine) can inspect it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceEntry {
    pub name: String,
    pub body: TypeBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBody {
    /// `Foo (Variant1 Variant2 ...)` — enum, possibly data-carrying.
    Enum { variants: Vec<EnumVariant> },
    /// `Foo [Field1 Field2 ...]` — positional struct.
    Struct { fields: Vec<Node> },
    /// `Foo { key value ... }` — map / namespace-shaped sub-definition.
    Map { entries: Vec<(Node, Node)> },
    /// `Foo MacroInvocation` — macro-shaped value; the macro engine
    /// inspects the structural shape (per record 753).
    Macro { invocation: Node },
    /// `Foo BareIdentifier` — an alias to another namespace name.
    Alias { target: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<Node>,
}

/// One operation in an Input section, or one reply in an Output
/// section. Both share the same record shape: `(Name (Payload))`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationEntry {
    pub name: String,
    pub payload: Vec<Node>,
}

/// Fully read schema: namespace flattened to a list (insertion order
/// preserved) plus the Input/Output operation lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssembledSchema {
    pub namespace: Vec<NamespaceEntry>,
    pub input_operations: Vec<OperationEntry>,
    pub output_operations: Vec<OperationEntry>,
}

impl AssembledSchema {
    pub fn from_three_part(parts: &ThreePartSchema) -> Result<Self, SchemaError> {
        let namespace = NamespaceReader::new(&parts.namespace).read()?;
        let input_operations = OperationReader::new(&parts.input_header).read()?;
        let output_operations = OperationReader::new(&parts.output).read()?;
        Ok(Self {
            namespace,
            input_operations,
            output_operations,
        })
    }

    pub fn read(source: &str) -> Result<Self, SchemaError> {
        let three_part = ThreePartSchema::read(source)?;
        Self::from_three_part(&three_part)
    }

    pub fn lookup(&self, name: &str) -> Option<&NamespaceEntry> {
        self.namespace.iter().find(|entry| entry.name == name)
    }
}

struct NamespaceReader<'children> {
    children: &'children [Node],
}

impl<'children> NamespaceReader<'children> {
    fn new(children: &'children [Node]) -> Self {
        Self { children }
    }

    fn read(self) -> Result<Vec<NamespaceEntry>, SchemaError> {
        if self.children.len() % 2 != 0 {
            return Err(SchemaError::NamespaceUnpaired);
        }
        let mut entries = Vec::new();
        for chunk in self.children.chunks_exact(2) {
            let key = &chunk[0];
            let value = &chunk[1];
            let name = key
                .as_identifier()
                .ok_or(SchemaError::NamespaceKeyNotIdentifier)?
                .to_string();
            let body = Self::classify_body(value)?;
            entries.push(NamespaceEntry { name, body });
        }
        Ok(entries)
    }

    fn classify_body(value: &Node) -> Result<TypeBody, SchemaError> {
        match &value.kind {
            NodeKind::Record => {
                // `(Variant1 Variant2 ...)` — enum form.
                let mut variants = Vec::new();
                let mut cursor = 0;
                while cursor < value.children.len() {
                    let head = &value.children[cursor];
                    if head.is_identifier() {
                        // Bare identifier — unit variant OR variant name with following
                        // fields. We treat the FOLLOWING fields as data only if the
                        // next item is parenthesised/bracketed; otherwise treat each
                        // bare PascalCase as a unit variant. For prototype simplicity,
                        // bare-identifier-then-bare-identifier reads as two unit
                        // variants.
                        variants.push(EnumVariant {
                            name: head.text.clone(),
                            fields: Vec::new(),
                        });
                        cursor += 1;
                    } else if head.is_record() {
                        // `(Variant ...fields)` form
                        if let Some(first) = head.children.first() {
                            if let Some(name) = first.as_identifier() {
                                let fields = head.children.iter().skip(1).cloned().collect();
                                variants.push(EnumVariant {
                                    name: name.to_string(),
                                    fields,
                                });
                                cursor += 1;
                                continue;
                            }
                        }
                        return Err(SchemaError::MalformedOperation);
                    } else {
                        return Err(SchemaError::MalformedOperation);
                    }
                }
                Ok(TypeBody::Enum { variants })
            }
            NodeKind::Vector => Ok(TypeBody::Struct {
                fields: value.children.clone(),
            }),
            NodeKind::Map => {
                if value.children.len() % 2 != 0 {
                    return Err(SchemaError::NamespaceUnpaired);
                }
                let entries = value
                    .children
                    .chunks_exact(2)
                    .map(|chunk| (chunk[0].clone(), chunk[1].clone()))
                    .collect();
                Ok(TypeBody::Map { entries })
            }
            NodeKind::Identifier => Ok(TypeBody::Alias {
                target: value.text.clone(),
            }),
            _ => Ok(TypeBody::Macro {
                invocation: value.clone(),
            }),
        }
    }
}

struct OperationReader<'children> {
    children: &'children [Node],
}

impl<'children> OperationReader<'children> {
    fn new(children: &'children [Node]) -> Self {
        Self { children }
    }

    fn read(self) -> Result<Vec<OperationEntry>, SchemaError> {
        let mut entries = Vec::new();
        for node in self.children {
            if !node.is_record() {
                return Err(SchemaError::MalformedOperation);
            }
            let first = node
                .children
                .first()
                .ok_or(SchemaError::MalformedOperation)?;
            let name = first
                .as_identifier()
                .ok_or(SchemaError::MalformedOperation)?
                .to_string();
            let payload = node.children.iter().skip(1).cloned().collect();
            entries.push(OperationEntry { name, payload });
        }
        Ok(entries)
    }
}
