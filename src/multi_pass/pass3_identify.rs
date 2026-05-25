//! Pass 3 — Macro identification.
//!
//! Walk positions; classify each `NotaValue` into a typed
//! `MacroVariantInstance` by syntactic shape per /334 §3.4.
//!
//! Sub-passes (per /334 §8 Q3):
//! 1. NAME COLLECTION — gather every declared type name from the
//!    namespace map keys + every imported binding name. This is
//!    required because at namespace position the shape `(Foo Bar)` is
//!    ambiguous (struct? newtype? alias? import reference?) without a
//!    name table. Q3 was right: a pre-pass is needed.
//! 2. DISPATCH — walk each position with the name table in hand.

use std::collections::{BTreeMap, BTreeSet};

use crate::multi_pass::{MultiPassDocument, NotaValue};
use crate::{Error, Leg, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedDocument {
    pub imports: Vec<IdentifiedImport>,
    pub ordinary_routes: Vec<IdentifiedHeaderRoot>,
    pub owner_routes: Vec<IdentifiedHeaderRoot>,
    pub sema_routes: Vec<IdentifiedHeaderRoot>,
    pub types: Vec<IdentifiedType>,
    pub features: Vec<IdentifiedFeature>,
    pub name_table: NameTable,
}

/// All names declared or imported in the document. Built by the name-
/// collection sub-pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameTable {
    pub locals: BTreeSet<String>,
    pub imported_bindings: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MacroVariantInstance {
    Import(IdentifiedImport),
    HeaderRoot(IdentifiedHeaderRoot),
    Type(IdentifiedType),
    Feature(IdentifiedFeature),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedImport {
    pub binding: String,
    pub kind: ImportKind,
    pub path: String,
    pub names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Import,
    ImportAll,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedHeaderRoot {
    pub leg: Leg,
    pub root: String,
    pub endpoints: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedType {
    pub name: String,
    pub body: IdentifiedTypeBody,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentifiedTypeBody {
    Enum(Vec<IdentifiedVariant>),
    Record(Vec<NotaValue>),
    Newtype(NotaValue),
    Alias(NotaValue),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IdentifiedVariant {
    pub name: String,
    pub fields: Vec<NotaValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentifiedFeature {
    Reply(Vec<String>),
    Event {
        stream: Option<String>,
        events: Vec<String>,
    },
    Observable {
        filter: Option<String>,
        operation_event: Option<String>,
        effect_event: Option<String>,
    },
    Upgrade(NotaValue),
}

pub fn identify(document: &MultiPassDocument) -> Result<IdentifiedDocument> {
    // Sub-pass 1 — collect names.
    let name_table = collect_names(document)?;

    // Sub-pass 2 — dispatch.
    let imports = identify_imports(&document.imports)?;
    let ordinary_routes = identify_header(&document.ordinary_header, Leg::Ordinary)?;
    let owner_routes = identify_header(&document.owner_header, Leg::Owner)?;
    let sema_routes = identify_header(&document.sema_header, Leg::Sema)?;
    let types = identify_types(&document.namespace, &name_table)?;
    let features = identify_features(&document.features)?;

    Ok(IdentifiedDocument {
        imports,
        ordinary_routes,
        owner_routes,
        sema_routes,
        types,
        features,
        name_table,
    })
}

fn collect_names(document: &MultiPassDocument) -> Result<NameTable> {
    let mut table = NameTable::default();
    if let Some(entries) = document.namespace.as_map() {
        for (name, _) in entries {
            if !table.locals.insert(name.clone()) {
                return Err(Error::InvalidSchemaText {
                    context: "pass3 name collection",
                    message: format!("duplicate declaration `{name}`"),
                });
            }
        }
    }
    if let Some(entries) = document.imports.as_map() {
        for (binding, _) in entries {
            table.imported_bindings.insert(binding.clone());
        }
    }
    Ok(table)
}

fn identify_imports(value: &NotaValue) -> Result<Vec<IdentifiedImport>> {
    let entries = value.as_map().ok_or_else(|| Error::InvalidSchemaText {
        context: "pass3 imports",
        message: "imports must be a map".into(),
    })?;
    let mut imports = Vec::new();
    let mut seen_bindings: BTreeMap<String, ()> = BTreeMap::new();
    for (binding, directive_value) in entries {
        if seen_bindings.insert(binding.clone(), ()).is_some() {
            return Err(Error::InvalidSchemaText {
                context: "pass3 imports",
                message: format!("duplicate import binding `{binding}`"),
            });
        }
        let items = directive_value
            .as_record()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass3 imports",
                message: format!("binding `{binding}` directive must be a record"),
            })?;
        // Identify by HEAD identifier — disambiguation by syntactic shape per /334 §3.4.
        let head = items
            .first()
            .and_then(|item| item.as_identifier())
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass3 imports",
                message: format!("binding `{binding}` directive has no head identifier"),
            })?;
        let import = match head {
            "Import" => {
                // (Import <path> [<name> ...])
                if items.len() != 3 {
                    return Err(Error::InvalidSchemaText {
                        context: "pass3 imports",
                        message: format!(
                            "`Import` for binding `{binding}` expected 3 positions, got {}",
                            items.len()
                        ),
                    });
                }
                let path = identifier_or_string(&items[1])?;
                let names_list = items[2].as_list().ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass3 imports",
                    message: format!("`Import` for binding `{binding}` expected a list of names"),
                })?;
                let names = names_list
                    .iter()
                    .map(|item| {
                        item.as_identifier().map(|s| s.to_string()).ok_or_else(|| {
                            Error::InvalidSchemaText {
                                context: "pass3 imports",
                                message: format!(
                                    "`Import` names for `{binding}` must be identifiers"
                                ),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;
                IdentifiedImport {
                    binding: binding.clone(),
                    kind: ImportKind::Import,
                    path,
                    names,
                }
            }
            "ImportAll" => {
                if items.len() != 2 {
                    return Err(Error::InvalidSchemaText {
                        context: "pass3 imports",
                        message: format!(
                            "`ImportAll` for binding `{binding}` expected 2 positions, got {}",
                            items.len()
                        ),
                    });
                }
                let path = identifier_or_string(&items[1])?;
                IdentifiedImport {
                    binding: binding.clone(),
                    kind: ImportKind::ImportAll,
                    path,
                    names: Vec::new(),
                }
            }
            other => {
                return Err(Error::InvalidSchemaText {
                    context: "pass3 imports",
                    message: format!("unknown import directive `{other}` for `{binding}`"),
                });
            }
        };
        imports.push(import);
    }
    Ok(imports)
}

fn identifier_or_string(value: &NotaValue) -> Result<String> {
    match value {
        NotaValue::Identifier(name, _) => Ok(name.clone()),
        NotaValue::String(text, _) => Ok(text.clone()),
        other => Err(Error::InvalidSchemaText {
            context: "pass3",
            message: format!(
                "expected identifier or string for path, got {}",
                other.kind_name()
            ),
        }),
    }
}

fn identify_header(value: &NotaValue, leg: Leg) -> Result<Vec<IdentifiedHeaderRoot>> {
    let items = value.as_list().ok_or_else(|| Error::InvalidSchemaText {
        context: "pass3 header",
        message: format!("header for {leg:?} must be a list"),
    })?;
    let mut roots = Vec::new();
    for item in items {
        let positions = item.as_record().ok_or_else(|| Error::InvalidSchemaText {
            context: "pass3 header",
            message: format!("header root must be a record, got {}", item.kind_name()),
        })?;
        if positions.len() != 2 {
            return Err(Error::InvalidSchemaText {
                context: "pass3 header",
                message: format!(
                    "v13 header root requires exactly 2 positions (Root [Endpoints]), got {}",
                    positions.len()
                ),
            });
        }
        let root = positions[0]
            .as_identifier()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass3 header",
                message: "header root must start with an identifier".into(),
            })?;
        let endpoint_list = positions[1]
            .as_list()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass3 header",
                message: format!("header root `{root}` requires a list of endpoints (v13 shape)"),
            })?;
        let endpoints = endpoint_list
            .iter()
            .map(|item| {
                item.as_identifier().map(|s| s.to_string()).ok_or_else(|| {
                    Error::InvalidSchemaText {
                        context: "pass3 header",
                        message: format!(
                            "endpoints under `{root}` must be identifiers, got {}",
                            item.kind_name()
                        ),
                    }
                })
            })
            .collect::<Result<Vec<_>>>()?;
        roots.push(IdentifiedHeaderRoot {
            leg,
            root: root.to_string(),
            endpoints,
        });
    }
    Ok(roots)
}

fn identify_types(
    namespace_value: &NotaValue,
    _name_table: &NameTable,
) -> Result<Vec<IdentifiedType>> {
    let entries = namespace_value
        .as_map()
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "pass3 namespace",
            message: "namespace must be a map".into(),
        })?;
    let mut types = Vec::new();
    for (name, body_value) in entries {
        let body = match body_value {
            NotaValue::List(items, _) => {
                // EnumDefinition. Each variant is either an identifier
                // (unit) or a record (Name field-expressions...).
                let mut variants = Vec::new();
                for item in items {
                    let variant = match item {
                        NotaValue::Identifier(variant_name, _) => IdentifiedVariant {
                            name: variant_name.clone(),
                            fields: Vec::new(),
                        },
                        NotaValue::Record(items, _) => {
                            let head = items.first().and_then(|i| i.as_identifier()).ok_or_else(
                                || Error::InvalidSchemaText {
                                    context: "pass3 namespace",
                                    message: format!(
                                        "data-carrying variant under `{name}` must start with an identifier"
                                    ),
                                },
                            )?;
                            IdentifiedVariant {
                                name: head.to_string(),
                                fields: items[1..].to_vec(),
                            }
                        }
                        other => {
                            return Err(Error::InvalidSchemaText {
                                context: "pass3 namespace",
                                message: format!(
                                    "variant under `{name}` must be identifier or record, got {}",
                                    other.kind_name()
                                ),
                            });
                        }
                    };
                    variants.push(variant);
                }
                IdentifiedTypeBody::Enum(variants)
            }
            NotaValue::Record(items, _) => {
                if items.is_empty() {
                    return Err(Error::InvalidSchemaText {
                        context: "pass3 namespace",
                        message: format!(
                            "namespace record `{name}` must carry at least one type expression"
                        ),
                    });
                }
                // (T) is newtype; (A B ...) is record/struct.
                if items.len() == 1 {
                    IdentifiedTypeBody::Newtype(items[0].clone())
                } else {
                    IdentifiedTypeBody::Record(items.clone())
                }
            }
            NotaValue::Identifier(_, _) => IdentifiedTypeBody::Alias(body_value.clone()),
            other => {
                return Err(Error::InvalidSchemaText {
                    context: "pass3 namespace",
                    message: format!(
                        "namespace value `{name}` has unsupported shape {}",
                        other.kind_name()
                    ),
                });
            }
        };
        types.push(IdentifiedType {
            name: name.clone(),
            body,
        });
    }
    Ok(types)
}

fn identify_features(value: &NotaValue) -> Result<Vec<IdentifiedFeature>> {
    let items = value.as_list().ok_or_else(|| Error::InvalidSchemaText {
        context: "pass3 features",
        message: "features must be a list".into(),
    })?;
    let mut features = Vec::new();
    for item in items {
        let positions = item.as_record().ok_or_else(|| Error::InvalidSchemaText {
            context: "pass3 features",
            message: format!("feature must be a record, got {}", item.kind_name()),
        })?;
        let head = positions
            .first()
            .and_then(|i| i.as_identifier())
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "pass3 features",
                message: "feature record must start with an identifier".into(),
            })?;
        let feature = match head {
            "Reply" => {
                let names = positions[1..]
                    .iter()
                    .map(|i| {
                        i.as_identifier().map(|s| s.to_string()).ok_or_else(|| {
                            Error::InvalidSchemaText {
                                context: "pass3 features",
                                message: "Reply payload must be identifiers".into(),
                            }
                        })
                    })
                    .collect::<Result<_>>()?;
                IdentifiedFeature::Reply(names)
            }
            "Event" => {
                let (stream, events_start) = if let Some(first) = positions.get(1) {
                    if let Some(record_items) = first.as_record() {
                        // (belongs StreamName) sub-record.
                        let head = record_items
                            .first()
                            .and_then(|i| i.as_identifier())
                            .or_else(|| {
                                record_items.first().and_then(|i| {
                                    if let NotaValue::String(text, _) = i {
                                        Some(text.as_str())
                                    } else {
                                        None
                                    }
                                })
                            });
                        if head == Some("belongs") {
                            let stream_name = record_items
                                .get(1)
                                .and_then(|i| i.as_identifier())
                                .ok_or_else(|| Error::InvalidSchemaText {
                                    context: "pass3 features",
                                    message: "`(belongs <name>)` requires an identifier".into(),
                                })?
                                .to_string();
                            (Some(stream_name), 2)
                        } else {
                            (None, 1)
                        }
                    } else {
                        (None, 1)
                    }
                } else {
                    (None, 1)
                };
                let events = positions[events_start..]
                    .iter()
                    .map(|i| {
                        i.as_identifier().map(|s| s.to_string()).ok_or_else(|| {
                            Error::InvalidSchemaText {
                                context: "pass3 features",
                                message: "Event names must be identifiers".into(),
                            }
                        })
                    })
                    .collect::<Result<_>>()?;
                IdentifiedFeature::Event { stream, events }
            }
            "Observable" => {
                let mut filter = None;
                let mut operation_event = None;
                let mut effect_event = None;
                for field in &positions[1..] {
                    let field_items =
                        field.as_record().ok_or_else(|| Error::InvalidSchemaText {
                            context: "pass3 features",
                            message: "Observable field must be a record".into(),
                        })?;
                    let key = field_items
                        .first()
                        .and_then(|i| i.as_identifier())
                        .ok_or_else(|| Error::InvalidSchemaText {
                            context: "pass3 features",
                            message: "Observable field needs key".into(),
                        })?;
                    match key {
                        "filter" => {
                            filter = Some(
                                field_items
                                    .get(1)
                                    .and_then(|i| i.as_identifier())
                                    .map(|s| s.to_string())
                                    .ok_or_else(|| Error::InvalidSchemaText {
                                        context: "pass3 features",
                                        message: "filter value must be identifier".into(),
                                    })?,
                            );
                        }
                        "operation_event" => {
                            operation_event = Some(
                                field_items
                                    .get(1)
                                    .and_then(|i| i.as_identifier())
                                    .map(|s| s.to_string())
                                    .ok_or_else(|| Error::InvalidSchemaText {
                                        context: "pass3 features",
                                        message: "operation_event value must be identifier".into(),
                                    })?,
                            );
                        }
                        "effect_event" => {
                            effect_event = Some(
                                field_items
                                    .get(1)
                                    .and_then(|i| i.as_identifier())
                                    .map(|s| s.to_string())
                                    .ok_or_else(|| Error::InvalidSchemaText {
                                        context: "pass3 features",
                                        message: "effect_event value must be identifier".into(),
                                    })?,
                            );
                        }
                        other => {
                            return Err(Error::InvalidSchemaText {
                                context: "pass3 features",
                                message: format!("unknown Observable field `{other}`"),
                            });
                        }
                    }
                }
                IdentifiedFeature::Observable {
                    filter,
                    operation_event,
                    effect_event,
                }
            }
            "Upgrade" => IdentifiedFeature::Upgrade(item.clone()),
            other => {
                return Err(Error::InvalidSchemaText {
                    context: "pass3 features",
                    message: format!("unknown feature `{other}`"),
                });
            }
        };
        features.push(feature);
    }
    Ok(features)
}
