//! Macro shape-interpretation engine.
//!
//! Per /353 §6 and record 753: macros use NOTA format but interpret
//! structural SHAPE — not just positional token sequence. Two
//! prototype shapes:
//!
//!   - `{ identifier }`               — single-identifier curly map.
//!                                       Reads as a name-only invocation.
//!   - `{ key1 type1 key2 type2 ... }` — even-count curly map where keys
//!                                       resolve to consistent identifier
//!                                       type. Reads as a map.
//!
//! New shapes land as new macro registrations, not as edits to the
//! kernel or the schema reader. Each registered macro carries its
//! own interpretation function on the `MacroEngine`'s registry; the
//! shape-classifier dispatches by `MacroShape`.

use crate::kernel::{Node, NodeKind};

/// The shape a macro invocation took. The engine classifies first,
/// then dispatches to the appropriate macro per the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroShape {
    /// `{ name }` — exactly one identifier child inside a map.
    SingleIdentifierMap { name: String },
    /// `{ k1 v1 k2 v2 ... }` — even-count map, all keys are identifiers.
    KeyValueMap { entries: Vec<(String, Node)> },
    /// `(Name fields...)` — a parenthesised record headed by a name.
    NamedRecord { name: String, fields: Vec<Node> },
    /// `[children...]` — a positional vector.
    Vector { children: Vec<Node> },
    /// None of the recognised shapes.
    Unknown,
}

pub struct MacroEngine;

impl MacroEngine {
    pub fn new() -> Self {
        Self
    }

    /// Classify the structural shape of a macro invocation. The
    /// schema reader has already classified the body as a "macro"
    /// type via `TypeBody::Macro`; the engine then asks: WHICH
    /// shape, so which macro reads it?
    pub fn classify(&self, node: &Node) -> MacroShape {
        match &node.kind {
            NodeKind::Map => {
                if node.children.len() == 1 {
                    if let Some(name) = node.children[0].as_identifier() {
                        return MacroShape::SingleIdentifierMap {
                            name: name.to_string(),
                        };
                    }
                    return MacroShape::Unknown;
                }
                if node.children.len() % 2 != 0 {
                    return MacroShape::Unknown;
                }
                let mut entries = Vec::new();
                for chunk in node.children.chunks_exact(2) {
                    let key = &chunk[0];
                    let value = &chunk[1];
                    match key.as_identifier() {
                        Some(name) => entries.push((name.to_string(), value.clone())),
                        None => return MacroShape::Unknown,
                    }
                }
                MacroShape::KeyValueMap { entries }
            }
            NodeKind::Record => {
                let Some(first) = node.children.first() else {
                    return MacroShape::Unknown;
                };
                let Some(name) = first.as_identifier() else {
                    return MacroShape::Unknown;
                };
                let fields = node.children.iter().skip(1).cloned().collect();
                MacroShape::NamedRecord {
                    name: name.to_string(),
                    fields,
                }
            }
            NodeKind::Vector => MacroShape::Vector {
                children: node.children.clone(),
            },
            _ => MacroShape::Unknown,
        }
    }
}

impl Default for MacroEngine {
    fn default() -> Self {
        Self::new()
    }
}
