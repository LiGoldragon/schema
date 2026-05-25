//! Pass 4 — Macro application (lowering).
//!
//! Run each `MacroVariantInstance`'s lowerer in the canonical order
//! (imports first, then types, then headers, then features) and emit
//! `AssembledFragment` entries into a `LoweringContext`. Pass 5
//! finishes the builder.
//!
//! Wiring note: the existing `schema::engine::{HeaderMacro, TypeMacro,
//! FeatureMacro, ImportMacro, UpgradeRuleMacro}` impls operate on
//! typed input structs that ALREADY come from the current
//! single-pass parser. We can reuse them only if we LOWER our Pass 3
//! `Identified*` types into the same input shapes. That conversion is
//! the real Pass 4 body. Three flaws surface here:
//!
//! - NewtypeDefinition / FieldType / UpgradeRule input variants are
//!   NOT registered as distinct `BuiltinMacroVariant`s today —
//!   `TypeInput` carries enum/record/newtype/alias under one bag, and
//!   `UpgradeRuleInput` exists but is invoked from the single-pass
//!   path. We stub the gap by reusing TypeInput + UpgradeRuleInput.
//! - `ImportMacro` only attaches binding metadata; it does NOT load
//!   sibling schemas (the side-effectful load happens in `reader.rs`'s
//!   private `Reader::read`, OUTSIDE the macro layer). So macros are
//!   pure today by accident — the purity boundary lives in `Reader`,
//!   not in any typed effect capability per /334 §8 Q1.
//! - There is no FieldType `NodeDefinitionPoint`; container
//!   expressions are parsed inline by the single-pass parser. We mirror
//!   it here by lowering type-expression NotaValues inline.

use std::collections::BTreeMap;

use crate::multi_pass::pass3_identify::{
    IdentifiedDocument, IdentifiedFeature, IdentifiedHeaderRoot, IdentifiedTypeBody,
    IdentifiedVariant, ImportKind,
};
use crate::multi_pass::{ImportResolver, NotaValue};
use crate::{
    AssembledSchema, BuiltinMacroVariant, Container, DeclarationBody, Endpoint, Engine, Error,
    EventFeature, Feature, FeatureInput, HeaderEndpointInput, HeaderInput, ImportBinding,
    ImportInput, ImportedNames, Leg, LoweringContext, Name, ObservableFeature, Payload, Primitive,
    Result, Route, RouteBody, SchemaPath, TypeExpression, TypeInput, Upgrade, UpgradeAnnotation,
    UpgradeRuleInput, Variant, Version,
};

pub fn lower_and_assemble(
    document: IdentifiedDocument,
    resolver: &ImportResolver<'_>,
) -> Result<AssembledSchema> {
    let mut context = LoweringContext::new();
    let mut import_names: BTreeMap<String, Vec<String>> = BTreeMap::new();

    // 1. Imports first.
    for import in &document.imports {
        let imported_names = match import.kind {
            ImportKind::Import => import.names.clone(),
            ImportKind::ImportAll => match resolver(&import.path) {
                Ok(assembled) => assembled
                    .types()
                    .map(|t| t.name().as_str().to_string())
                    .collect(),
                Err(_) => Vec::new(),
            },
        };
        import_names.insert(import.binding.clone(), imported_names.clone());
        let names_kind = match import.kind {
            ImportKind::Import => ImportedNames::Selected(string_names(&imported_names)?),
            ImportKind::ImportAll => ImportedNames::All(string_names(&imported_names)?),
        };
        let binding = ImportBinding::new(
            Name::new(&import.binding)?,
            SchemaPath::new(import.path.clone()),
            names_kind,
        );
        context.apply(BuiltinMacroVariant::Import(ImportInput::new(binding)))?;
    }

    // Build local type name set (used by route-body resolution).
    let local_names: Vec<Name> = document
        .types
        .iter()
        .map(|t| Name::new(&t.name))
        .collect::<Result<_>>()?;
    let local_set: std::collections::BTreeSet<&Name> = local_names.iter().collect();
    let imported_set: std::collections::BTreeMap<String, String> = document
        .imports
        .iter()
        .flat_map(|import| {
            import_names
                .get(&import.binding)
                .into_iter()
                .flatten()
                .map(move |name| (name.clone(), import.binding.clone()))
        })
        .collect();
    let _ = local_set;

    // Lower local enum/record/newtype/alias bodies BEFORE headers,
    // because headers reference local type names. (The existing
    // single-pass reader does these in the order: imports, headers,
    // types, features — but the order doesn't actually matter for the
    // final AssembledSchema content; only for validation diagnostics.)
    let local_bodies: BTreeMap<String, DeclarationBody> = document
        .types
        .iter()
        .map(|t| Ok::<_, Error>((t.name.clone(), lower_type_body(&t.body)?)))
        .collect::<Result<_>>()?;
    for ident_type in &document.types {
        let body = local_bodies
            .get(&ident_type.name)
            .cloned()
            .expect("local body just computed above");
        let name = Name::new(&ident_type.name)?;
        context.apply(BuiltinMacroVariant::Type(TypeInput::local(name, body)))?;
    }
    for (name, binding) in &imported_set {
        // Imported types are also pushed (mirroring the canonical reader).
        let type_name = Name::new(name)?;
        let binding_name = Name::new(binding)?;
        context.apply(BuiltinMacroVariant::Type(TypeInput::imported(
            type_name,
            binding_name,
        )))?;
    }

    // Headers next. Map endpoint -> body type via the local enum.
    lower_header_leg(
        &mut context,
        &document.ordinary_routes,
        Leg::Ordinary,
        &local_bodies,
        &imported_set,
    )?;
    lower_header_leg(
        &mut context,
        &document.owner_routes,
        Leg::Owner,
        &local_bodies,
        &imported_set,
    )?;
    lower_header_leg(
        &mut context,
        &document.sema_routes,
        Leg::Sema,
        &local_bodies,
        &imported_set,
    )?;

    // Features last.
    for feature in &document.features {
        let lowered = lower_feature(feature)?;
        match lowered {
            Feature::Upgrade(upgrade) => {
                context.apply(BuiltinMacroVariant::UpgradeRule(UpgradeRuleInput::new(
                    upgrade,
                )))?;
            }
            other => {
                context.apply(BuiltinMacroVariant::Feature(FeatureInput::new(other)))?;
            }
        }
    }

    Ok(context.finish())
}

fn string_names(names: &[String]) -> Result<Vec<Name>> {
    names.iter().map(Name::new).collect()
}

fn lower_type_body(body: &IdentifiedTypeBody) -> Result<DeclarationBody> {
    match body {
        IdentifiedTypeBody::Enum(variants) => {
            let mut lowered = Vec::new();
            for variant in variants {
                lowered.push(lower_variant(variant)?);
            }
            Ok(DeclarationBody::Enum { variants: lowered })
        }
        IdentifiedTypeBody::Record(fields) => {
            let mut exprs = Vec::new();
            for field in fields {
                exprs.push(lower_type_expression(field)?);
            }
            Ok(DeclarationBody::Record(exprs))
        }
        IdentifiedTypeBody::Newtype(value) => {
            let expr = lower_type_expression(value)?;
            Ok(DeclarationBody::Newtype(expr))
        }
        IdentifiedTypeBody::Alias(value) => {
            let expr = lower_type_expression(value)?;
            Ok(DeclarationBody::Alias(expr))
        }
    }
}

fn lower_variant(variant: &IdentifiedVariant) -> Result<Variant> {
    let name = Name::new(&variant.name)?;
    if variant.fields.is_empty() {
        return Ok(Variant::unit(name));
    }
    let exprs = variant
        .fields
        .iter()
        .map(lower_type_expression)
        .collect::<Result<Vec<_>>>()?;
    Ok(match exprs.len() {
        0 => Variant::unit(name),
        1 => Variant::with_type(name, exprs.into_iter().next().unwrap()),
        _ => Variant::with_fields(name, exprs),
    })
}

fn lower_type_expression(value: &NotaValue) -> Result<TypeExpression> {
    match value {
        NotaValue::Identifier(name, _) => {
            if let Some(primitive) = primitive(name) {
                Ok(primitive)
            } else {
                Name::new(name).map(TypeExpression::named)
            }
        }
        NotaValue::Record(items, _) => {
            let head = items
                .first()
                .and_then(|item| item.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 type-expression",
                    message: "container record must start with identifier".into(),
                })?;
            match head {
                "Option" => {
                    if items.len() != 2 {
                        return Err(Error::InvalidSchemaText {
                            context: "pass4 type-expression",
                            message: "Option must wrap exactly one inner expression".into(),
                        });
                    }
                    Ok(TypeExpression::optional(lower_type_expression(&items[1])?))
                }
                "Vec" => {
                    if items.len() != 2 {
                        return Err(Error::InvalidSchemaText {
                            context: "pass4 type-expression",
                            message: "Vec must wrap exactly one inner expression".into(),
                        });
                    }
                    Ok(TypeExpression::vector(lower_type_expression(&items[1])?))
                }
                "Map" => {
                    if items.len() != 3 {
                        return Err(Error::InvalidSchemaText {
                            context: "pass4 type-expression",
                            message: "Map needs key and value expressions".into(),
                        });
                    }
                    Ok(TypeExpression::map(
                        lower_type_expression(&items[1])?,
                        lower_type_expression(&items[2])?,
                    ))
                }
                other => Err(Error::InvalidSchemaText {
                    context: "pass4 type-expression",
                    message: format!("unknown container `{other}`"),
                }),
            }
        }
        NotaValue::List(_, _) => Err(Error::InvalidSchemaText {
            context: "pass4 type-expression",
            message: "list shape `[T]` is not a valid container — use `(Vec T)`".into(),
        }),
        other => Err(Error::InvalidSchemaText {
            context: "pass4 type-expression",
            message: format!(
                "cannot interpret {} as a type expression",
                other.kind_name()
            ),
        }),
    }
}

fn primitive(text: &str) -> Option<TypeExpression> {
    let primitive = match text {
        "String" => Primitive::String,
        "Bytes" => Primitive::Bytes,
        "Boolean" => Primitive::Boolean,
        "u8" => Primitive::Unsigned8,
        "u16" => Primitive::Unsigned16,
        "u32" => Primitive::Unsigned32,
        "u64" => Primitive::Unsigned64,
        "Date" => Primitive::Date,
        "Time" => Primitive::Time,
        _ => return None,
    };
    Some(TypeExpression::Primitive(primitive))
}

fn lower_header_leg(
    context: &mut LoweringContext,
    roots: &[IdentifiedHeaderRoot],
    leg: Leg,
    local_bodies: &BTreeMap<String, DeclarationBody>,
    imports: &BTreeMap<String, String>,
) -> Result<()> {
    for (root_slot, root) in roots.iter().enumerate() {
        let mut endpoint_inputs = Vec::new();
        for (slot, endpoint) in root.endpoints.iter().enumerate() {
            let (body, engine) = resolve_endpoint_body(root, endpoint, local_bodies, imports)?;
            let endpoint_name = Name::new(endpoint)?;
            endpoint_inputs.push(HeaderEndpointInput::new(slot, endpoint_name, body, engine));
        }
        let root_name = Name::new(&root.root)?;
        context.apply(BuiltinMacroVariant::Header(HeaderInput::new(
            leg,
            root_slot,
            root_name,
            endpoint_inputs,
        )))?;
    }
    Ok(())
}

fn resolve_endpoint_body(
    root: &IdentifiedHeaderRoot,
    endpoint: &str,
    local_bodies: &BTreeMap<String, DeclarationBody>,
    _imports: &BTreeMap<String, String>,
) -> Result<(RouteBody, Option<Engine>)> {
    // The root enum lives in local namespace.
    let body = local_bodies
        .get(&root.root)
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "pass4 header lowering",
            message: format!("root `{}` has no namespace declaration", root.root),
        })?;
    let DeclarationBody::Enum { variants } = body else {
        return Err(Error::InvalidSchemaText {
            context: "pass4 header lowering",
            message: format!("root `{}` must be an enum", root.root),
        });
    };
    let target = variants
        .iter()
        .find(|v| v.name().as_str() == endpoint)
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "pass4 header lowering",
            message: format!("endpoint `{endpoint}` not found in root `{}`", root.root),
        })?;
    match target.payload() {
        Payload::Unit => Ok((RouteBody::Unit, target.engine())),
        Payload::Type(TypeExpression::Named(name)) => {
            Ok((RouteBody::Type(name.clone()), target.engine()))
        }
        Payload::Type(_) | Payload::Fields(_) => Err(Error::InvalidSchemaText {
            context: "pass4 header lowering",
            message: format!(
                "endpoint `{}/{}` must resolve to a named body type",
                root.root, endpoint
            ),
        }),
    }
}

fn lower_feature(feature: &IdentifiedFeature) -> Result<Feature> {
    match feature {
        IdentifiedFeature::Reply(names) => {
            let names = names.iter().map(Name::new).collect::<Result<Vec<_>>>()?;
            Ok(Feature::Reply(names))
        }
        IdentifiedFeature::Event { stream, events } => {
            let stream = stream.as_ref().map(Name::new).transpose()?;
            let events = events.iter().map(Name::new).collect::<Result<Vec<_>>>()?;
            Ok(Feature::Event(EventFeature::new(stream, events)))
        }
        IdentifiedFeature::Observable {
            filter,
            operation_event,
            effect_event,
        } => {
            let operation = operation_event.as_ref().map(Name::new).transpose()?;
            let effect = effect_event.as_ref().map(Name::new).transpose()?;
            Ok(Feature::Observable(ObservableFeature::new(
                filter.clone(),
                operation,
                effect,
            )))
        }
        IdentifiedFeature::Upgrade(value) => lower_upgrade(value).map(Feature::Upgrade),
    }
}

fn lower_upgrade(value: &NotaValue) -> Result<Upgrade> {
    let items = value.as_record().ok_or_else(|| Error::InvalidSchemaText {
        context: "pass4 upgrade",
        message: "Upgrade must be a record".into(),
    })?;
    // (Upgrade (FromVersion <path>) <annotations>...)
    let from_version_value = items.get(1).ok_or_else(|| Error::InvalidSchemaText {
        context: "pass4 upgrade",
        message: "Upgrade requires FromVersion".into(),
    })?;
    let from_items = from_version_value
        .as_record()
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "pass4 upgrade",
            message: "FromVersion must be a record".into(),
        })?;
    if from_items.first().and_then(|i| i.as_identifier()) != Some("FromVersion") {
        return Err(Error::InvalidSchemaText {
            context: "pass4 upgrade",
            message: "Upgrade first sub-record must be FromVersion".into(),
        });
    }
    let path_value = from_items.get(1).ok_or_else(|| Error::InvalidSchemaText {
        context: "pass4 upgrade",
        message: "FromVersion needs a path".into(),
    })?;
    let path_string = match path_value {
        NotaValue::Identifier(name, _) => name.clone(),
        NotaValue::String(text, _) => text.clone(),
        _ => {
            return Err(Error::InvalidSchemaText {
                context: "pass4 upgrade",
                message: "FromVersion path must be identifier or string".into(),
            });
        }
    };
    let version = Version::new(path_string);
    let mut annotations = Vec::new();
    for annotation_value in &items[2..] {
        annotations.push(lower_upgrade_annotation(annotation_value)?);
    }
    Ok(Upgrade::new(version, annotations))
}

fn lower_upgrade_annotation(value: &NotaValue) -> Result<UpgradeAnnotation> {
    let items = value.as_record().ok_or_else(|| Error::InvalidSchemaText {
        context: "pass4 upgrade annotation",
        message: "annotation must be a record".into(),
    })?;
    let head = items
        .first()
        .and_then(|i| i.as_identifier())
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "pass4 upgrade annotation",
            message: "annotation needs head identifier".into(),
        })?;
    match head {
        "Migrate" => {
            let name = items
                .get(1)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "Migrate needs a name".into(),
                })?;
            Ok(UpgradeAnnotation::Migrate(Name::new(name)?))
        }
        "RenamedFrom" => {
            let current = items
                .get(1)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "RenamedFrom needs current".into(),
                })?;
            let previous = items
                .get(2)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "RenamedFrom needs previous".into(),
                })?;
            Ok(UpgradeAnnotation::RenamedFrom {
                current: Name::new(current)?,
                previous: Name::new(previous)?,
            })
        }
        "Drop" => {
            let name = items
                .get(1)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "Drop needs a name".into(),
                })?;
            Ok(UpgradeAnnotation::Drop(Name::new(name)?))
        }
        "Custom" => {
            let name = items
                .get(1)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "Custom needs a name".into(),
                })?;
            let impl_name = items
                .get(2)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "Custom needs implementation".into(),
                })?;
            Ok(UpgradeAnnotation::Custom {
                name: Name::new(name)?,
                implementation: Name::new(impl_name)?,
            })
        }
        "Untranslatable" => {
            let name = items
                .get(1)
                .and_then(|i| i.as_identifier())
                .ok_or_else(|| Error::InvalidSchemaText {
                    context: "pass4 upgrade annotation",
                    message: "Untranslatable needs a name".into(),
                })?;
            Ok(UpgradeAnnotation::Untranslatable(Name::new(name)?))
        }
        other => Err(Error::InvalidSchemaText {
            context: "pass4 upgrade annotation",
            message: format!("unknown upgrade annotation `{other}`"),
        }),
    }
}

// Unused but kept for Pass 5 readers.
#[allow(dead_code)]
fn _route_marker(_: Route, _: Endpoint, _: Container, _: Engine) {}
