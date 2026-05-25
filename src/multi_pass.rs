//! Multi-pass macro pipeline driven by NOTA shape-logic dispatch.
//!
//! This module proves end-to-end that:
//!
//! - The full schema-language pipeline can run on top of
//!   `nota_codec::NotaValue` WITHOUT the schema crate needing its own
//!   NOTA tree assembler.
//! - The four canonical macro families (Import / Header / Type /
//!   Feature) lower a real `.schema` file into the SAME
//!   `AssembledSchema` the canonical `Schema::parse_str` builds —
//!   byte-equivalent against the Spirit contract.
//! - **Builtin macros use shape-logic predicates** (`is_tagged_record`,
//!   `record_arity`, `is_single_ident_record`, ...) to dispatch on
//!   NOTA shape; the dispatch substrate is the same one a user-defined
//!   macro would plug into. The pipeline is a "macro for reading
//!   macros."
//!
//! Pipeline:
//!
//! - Pass 0 + Pass 1 — `nota_codec::parse_sequence` returns the six
//!   top-level `NotaValue`s. Lexical + syntactic in one call now that
//!   nota-codec exposes the tree.
//! - Pass 2 (structural) — `SchemaDocument::from_six_values` checks each
//!   position carries the right NOTA kind (map / sequence) and
//!   captures it.
//! - Pass 3 (index) — `MacroIndex::from_document` records macro
//!   endpoints in source order. Later passes can resolve/import lazily
//!   without rediscovering where each macro lives.
//! - Pass 4 + Pass 5 (identify + apply) — `MacroPipeline::run` walks
//!   indexed candidates and dispatches into the relevant
//!   `BuiltinMacroVariant` through the EXISTING `LoweringContext` from
//!   `schema::engine`. The dispatch is driven by shape-logic
//!   predicates from `nota_codec::NotaValue`.
//! - Pass 6 (assembly) — `LoweringContext::finish()`.
//!
//! Cross-references:
//! - `reports/designer/334-v2-multi-pass-nota-first-schema-reader.md`
//! - `reports/second-designer/182-schema-crate-state-and-version-projection-derivation-2026-05-25.md`
//! - `reports/second-designer/170-schema-lowering-executor-model-2026-05-24.md` §2 (dispatch table)
//! - `reports/designer/329-schema-macro-component-extensibility.md` (SchemaMacro trait + 7 builtins)
//! - `reports/nota-designer/8-nota-schema-lowering-deviation-audit.md` (named-input-struct pattern)
//! - Intent records 506 (data-carrying macro variants), 549 (multi-pass NOTA-first), 588 (reusable
//!   shape-logic layer), 589 (multi-pass passes generic NOTA subobjects).

use std::collections::BTreeMap;

use nota_codec::{NotaValue, parse_sequence};

use crate::{
    AssembledSchema, BuiltinMacroVariant, DeclarationBody, Endpoint, Engine, Error, EventFeature,
    Feature, FeatureInput, Field, HeaderEndpointInput, HeaderInput, ImportBinding, ImportInput,
    ImportedNames, Leg, LoweringContext, Name, NamespaceValueShape, NodeDefinitionPoint,
    NodeDefinitionShape, ObservableFeature, Payload, Primitive, Result, Route, RouteBody,
    SchemaPath, TypeExpression, TypeInput, Upgrade, UpgradeAnnotation, UpgradeRuleInput, Variant,
    Version,
};

/// Run the full multi-pass pipeline against `.schema` text and
/// return the resulting `AssembledSchema`.
///
/// This is the load-bearing entry point for the MVP — it walks the
/// six top-level NOTA values through builtin macro identification +
/// application using shape-logic dispatch all the way down. Imports
/// are NOT resolved against sibling schemas; the imported names are
/// recorded so the assembled output matches what
/// `Schema::parse_str(...).assemble(&[])` would produce for the SAME
/// schema text (where the resolution list is empty).
pub fn read_schema_six_position(text: &str) -> Result<AssembledSchema> {
    let raw_values = parse_sequence(text).map_err(|error| Error::InvalidSchemaText {
        context: "multi_pass parse_sequence",
        message: error.to_string(),
    })?;
    let document = SchemaDocument::from_six_values(raw_values)?;
    let mut pipeline = MacroPipeline::new(&document)?;
    pipeline.run()
}

/// Aggregate counts of macro firings + assembled fragments. The
/// end-to-end test asserts on these. `assembled` is the
/// `AssembledSchema` (which IS `Eq`); the counts use plain integers,
/// so the struct itself is `Eq`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PipelineReport {
    pub macro_index: MacroIndexReport,
    pub import_firings: usize,
    pub header_firings: usize,
    pub type_firings: usize,
    pub feature_firings: usize,
    pub assembled: Option<AssembledSchema>,
}

/// Variant of `read_schema_six_position` that also reports per-macro
/// firing counts so the end-to-end test can prove each builtin ran
/// the expected number of times.
pub fn read_schema_with_report(text: &str) -> Result<PipelineReport> {
    let raw_values = parse_sequence(text).map_err(|error| Error::InvalidSchemaText {
        context: "multi_pass parse_sequence",
        message: error.to_string(),
    })?;
    let document = SchemaDocument::from_six_values(raw_values)?;
    let mut pipeline = MacroPipeline::new(&document)?;
    let assembled = pipeline.run()?;
    let macro_index = pipeline.index.report();
    Ok(PipelineReport {
        macro_index,
        import_firings: pipeline.import_firings,
        header_firings: pipeline.header_firings,
        type_firings: pipeline.type_firings,
        feature_firings: pipeline.feature_firings,
        assembled: Some(assembled),
    })
}

/// Six top-level NOTA values arranged by schema position.
///
/// Not `Eq` because `NotaValue` carries `f64`.
#[derive(Clone, Debug, PartialEq)]
pub struct SchemaDocument {
    pub imports: NotaValue,
    pub ordinary_header: NotaValue,
    pub owner_header: NotaValue,
    pub sema_header: NotaValue,
    pub namespace: NotaValue,
    pub features: NotaValue,
}

impl SchemaDocument {
    pub fn from_six_values(values: Vec<NotaValue>) -> Result<Self> {
        if values.len() != 6 {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass structural pass",
                message: format!(
                    "expected six top-level values for a .schema file, got {}",
                    values.len()
                ),
            });
        }
        let mut iter = values.into_iter();
        let imports = iter.next().unwrap();
        let ordinary_header = iter.next().unwrap();
        let owner_header = iter.next().unwrap();
        let sema_header = iter.next().unwrap();
        let namespace = iter.next().unwrap();
        let features = iter.next().unwrap();

        expect_kind("imports", &imports, NotaKind::Map)?;
        expect_kind("ordinary header", &ordinary_header, NotaKind::Sequence)?;
        expect_kind("owner header", &owner_header, NotaKind::Sequence)?;
        expect_kind("sema header", &sema_header, NotaKind::Sequence)?;
        expect_kind("namespace", &namespace, NotaKind::Map)?;
        expect_kind("features", &features, NotaKind::Sequence)?;

        Ok(Self {
            imports,
            ordinary_header,
            owner_header,
            sema_header,
            namespace,
            features,
        })
    }
}

/// Observable candidate counts from the indexing pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MacroIndexReport {
    pub import_candidates: usize,
    pub header_candidates: usize,
    pub type_candidates: usize,
    pub feature_candidates: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaPosition {
    Imports,
    OrdinaryHeader,
    OwnerHeader,
    SemaHeader,
    Namespace,
    Features,
}

#[derive(Clone, Copy, Debug)]
struct ImportCandidate<'document> {
    binding: &'document str,
    value: &'document NotaValue,
}

#[derive(Clone, Copy, Debug)]
struct HeaderCandidate<'document> {
    position: SchemaPosition,
    leg: Leg,
    root_slot: usize,
    value: &'document NotaValue,
}

#[derive(Clone, Copy, Debug)]
struct TypeCandidate<'document> {
    name: &'document str,
    value: &'document NotaValue,
}

#[derive(Clone, Copy, Debug)]
struct FeatureCandidate<'document> {
    value: &'document NotaValue,
}

/// Source-order index of macro endpoints. This is the lazy-resolution
/// foothold: the engine first knows where all named/imported/lowerable
/// macro nodes live, then later passes resolve or lower them in the
/// precedence order the schema language chooses.
#[derive(Clone, Debug)]
struct MacroIndex<'document> {
    imports: Vec<ImportCandidate<'document>>,
    ordinary_headers: Vec<HeaderCandidate<'document>>,
    owner_headers: Vec<HeaderCandidate<'document>>,
    sema_headers: Vec<HeaderCandidate<'document>>,
    types: Vec<TypeCandidate<'document>>,
    features: Vec<FeatureCandidate<'document>>,
}

impl<'document> MacroIndex<'document> {
    fn from_document(document: &'document SchemaDocument) -> Result<Self> {
        let imports = document
            .imports
            .as_map()
            .expect("imports kind enforced by SchemaDocument::from_six_values")
            .iter()
            .map(|entry| ImportCandidate {
                binding: entry.key(),
                value: entry.value(),
            })
            .collect();
        let ordinary_headers = Self::header_candidates(
            SchemaPosition::OrdinaryHeader,
            Leg::Ordinary,
            &document.ordinary_header,
        )?;
        let owner_headers = Self::header_candidates(
            SchemaPosition::OwnerHeader,
            Leg::Owner,
            &document.owner_header,
        )?;
        let sema_headers =
            Self::header_candidates(SchemaPosition::SemaHeader, Leg::Sema, &document.sema_header)?;
        let types = document
            .namespace
            .as_map()
            .expect("namespace kind enforced by SchemaDocument::from_six_values")
            .iter()
            .map(|entry| TypeCandidate {
                name: entry.key(),
                value: entry.value(),
            })
            .collect();
        let features = document
            .features
            .as_sequence()
            .expect("features kind enforced by SchemaDocument::from_six_values")
            .iter()
            .map(|value| FeatureCandidate { value })
            .collect();
        Ok(Self {
            imports,
            ordinary_headers,
            owner_headers,
            sema_headers,
            types,
            features,
        })
    }

    fn header_candidates(
        position: SchemaPosition,
        leg: Leg,
        value: &'document NotaValue,
    ) -> Result<Vec<HeaderCandidate<'document>>> {
        let items = value
            .as_sequence()
            .expect("header kind enforced by SchemaDocument::from_six_values");
        Ok(items
            .iter()
            .enumerate()
            .map(|(root_slot, value)| HeaderCandidate {
                position,
                leg,
                root_slot,
                value,
            })
            .collect())
    }

    fn header_count(&self) -> usize {
        self.ordinary_headers.len() + self.owner_headers.len() + self.sema_headers.len()
    }

    fn report(&self) -> MacroIndexReport {
        MacroIndexReport {
            import_candidates: self.imports.len(),
            header_candidates: self.header_count(),
            type_candidates: self.types.len(),
            feature_candidates: self.features.len(),
        }
    }
}

/// Macro pipeline driver. Walks the six positions in order, dispatches
/// each sub-value through `BuiltinMacroVariant`, and tracks firing
/// counts. The dispatch decisions ARE the shape-logic methods on
/// `NotaValue` — `is_tagged_record`, `record_arity`,
/// `is_single_ident_record`, ... — never raw pattern matching.
struct MacroPipeline<'document> {
    index: MacroIndex<'document>,
    context: LoweringContext,
    /// Imported names per binding, captured from `(Import path
    /// [names])` records so the type-macro pass can lower imported
    /// names into the assembled output.
    imported_per_binding: BTreeMap<Name, Vec<Name>>,
    import_firings: usize,
    header_firings: usize,
    type_firings: usize,
    feature_firings: usize,
}

impl<'document> MacroPipeline<'document> {
    fn new(document: &'document SchemaDocument) -> Result<Self> {
        Ok(Self {
            index: MacroIndex::from_document(document)?,
            context: LoweringContext::new(),
            imported_per_binding: BTreeMap::new(),
            import_firings: 0,
            header_firings: 0,
            type_firings: 0,
            feature_firings: 0,
        })
    }

    fn run(&mut self) -> Result<AssembledSchema> {
        // Order mirrors the canonical `Schema::assemble` so the
        // resulting `AssembledSchema` is byte-equivalent: imports,
        // then headers (each leg), then local types, then imported
        // types, then features.
        self.lower_imports()?;
        self.lower_header(self.index.ordinary_headers.clone())?;
        self.lower_header(self.index.owner_headers.clone())?;
        self.lower_header(self.index.sema_headers.clone())?;
        self.lower_namespace_local_types()?;
        self.lower_imported_types()?;
        self.lower_features()?;
        Ok(std::mem::take(&mut self.context).finish())
    }

    fn lower_imports(&mut self) -> Result<()> {
        for candidate in self.index.imports.clone() {
            let binding = Name::new(candidate.binding)?;
            // Shape-logic dispatch — every match clause asks
            // NotaValue a SHAPE question, never a pattern-match
            // against the raw enum.
            let import_input = ImportMacroRecognizer::recognize(&binding, candidate.value)?;
            self.imported_per_binding
                .insert(binding.clone(), import_input.names.clone());
            let names_kind = match import_input.kind {
                ImportKind::Import => ImportedNames::Selected(import_input.names),
                ImportKind::ImportAll => ImportedNames::All(import_input.names),
            };
            let import_binding =
                ImportBinding::new(binding, SchemaPath::new(import_input.path), names_kind);
            self.context
                .apply(BuiltinMacroVariant::Import(ImportInput::new(
                    import_binding,
                )))?;
            self.import_firings += 1;
        }
        Ok(())
    }

    fn lower_header(&mut self, candidates: Vec<HeaderCandidate<'document>>) -> Result<()> {
        for candidate in candidates {
            let root_value = candidate.value;
            // A header root is `(Root [SubVariant ...])` — a tagged
            // record with exactly two positions: head + sequence of
            // endpoint names. Shape-logic dispatch.
            if !root_value.is_record() || root_value.record_arity() != Some(2) {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass header",
                    message: format!(
                        "{:?} root must be `(Root [SubVariant ...])` with two positions, got arity {:?}",
                        candidate.position,
                        root_value.record_arity(),
                    ),
                });
            }
            let root_name_text =
                root_value
                    .record_head_identifier()
                    .ok_or_else(|| Error::InvalidSchemaText {
                        context: "multi_pass header",
                        message: "header root must start with an identifier".into(),
                    })?;
            let root = Name::new(root_name_text)?;
            let endpoints_value = &root_value.as_record().unwrap()[1];
            if !endpoints_value.is_sequence() {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass header",
                    message: format!("root `{root_name_text}` requires a `[...]` endpoint list"),
                });
            }
            let endpoint_idents = endpoints_value.as_sequence().unwrap();
            let body_lookup = self.snapshot_local_bodies()?;
            let mut endpoints = Vec::new();
            for (endpoint_slot, endpoint_value) in endpoint_idents.iter().enumerate() {
                let endpoint_name_text =
                    endpoint_value
                        .identifier_text()
                        .ok_or_else(|| Error::InvalidSchemaText {
                            context: "multi_pass header",
                            message: format!(
                                "endpoints under `{root_name_text}` must be identifiers"
                            ),
                        })?;
                let endpoint_name = Name::new(endpoint_name_text)?;
                let (body, engine) =
                    resolve_endpoint_body_from_namespace(&root, &endpoint_name, &body_lookup)?;
                endpoints.push(HeaderEndpointInput::new(
                    endpoint_slot,
                    endpoint_name,
                    body,
                    engine,
                ));
            }
            self.context
                .apply(BuiltinMacroVariant::Header(HeaderInput::new(
                    candidate.leg,
                    candidate.root_slot,
                    root,
                    endpoints,
                )))?;
            self.header_firings += 1;
        }
        Ok(())
    }

    fn lower_namespace_local_types(&mut self) -> Result<()> {
        for candidate in self.index.types.clone() {
            let name = Name::new(candidate.name)?;
            // Each namespace value dispatches by SHAPE into one of
            // the type-macro sub-recognizers: enum / record / newtype
            // / alias. The recognizer reads `NotaValue` shape, never
            // hand-rolled pattern-matching against the enum.
            let body = TypeMacroRecognizer::recognize(candidate.value)?;
            self.context
                .apply(BuiltinMacroVariant::Type(TypeInput::local(name, body)))?;
            self.type_firings += 1;
        }
        Ok(())
    }

    fn lower_imported_types(&mut self) -> Result<()> {
        for (binding, names) in &self.imported_per_binding {
            for name in names {
                self.context
                    .apply(BuiltinMacroVariant::Type(TypeInput::imported(
                        name.clone(),
                        binding.clone(),
                    )))?;
                self.type_firings += 1;
            }
        }
        Ok(())
    }

    fn lower_features(&mut self) -> Result<()> {
        for candidate in self.index.features.clone() {
            // Each feature lowers based on its tagged-record head.
            // Shape-logic dispatch: ask the value WHICH tag it
            // carries, then hand it to the matching recognizer.
            let lowered = FeatureMacroRecognizer::recognize(candidate.value)?;
            match lowered {
                Feature::Upgrade(upgrade) => {
                    self.context
                        .apply(BuiltinMacroVariant::UpgradeRule(UpgradeRuleInput::new(
                            upgrade,
                        )))?;
                }
                other => {
                    self.context
                        .apply(BuiltinMacroVariant::Feature(FeatureInput::new(other)))?;
                }
            }
            self.feature_firings += 1;
        }
        Ok(())
    }

    /// Snapshot local type bodies for header-endpoint resolution. The
    /// canonical reader has to do the same dance — headers reference
    /// types declared in the namespace and need to find their body.
    fn snapshot_local_bodies(&self) -> Result<BTreeMap<Name, DeclarationBody>> {
        let mut bodies = BTreeMap::new();
        for candidate in &self.index.types {
            let name = Name::new(candidate.name)?;
            let body = TypeMacroRecognizer::recognize(candidate.value)?;
            bodies.insert(name, body);
        }
        Ok(bodies)
    }
}

/// Builtin Import macro. Dispatches by record-head identifier
/// (`Import` vs `ImportAll`) and shape-checks the positional payload.
/// The recognizer **does not** match the raw `NotaValue` enum — it
/// asks shape-logic questions.
struct ImportMacroRecognizer;

struct RecognizedImport {
    kind: ImportKind,
    path: String,
    names: Vec<Name>,
}

enum ImportKind {
    Import,
    ImportAll,
}

impl ImportMacroRecognizer {
    fn recognize(binding: &Name, value: &NotaValue) -> Result<RecognizedImport> {
        if !value.is_record() {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass import",
                message: format!(
                    "import directive for `{binding}` must be a `(Import ...)` or `(ImportAll ...)` record"
                ),
            });
        }
        // Shape-logic: ask record head + arity rather than match
        // arms against NotaValue::Record(...).
        if value.is_tagged_record("Import") {
            if value.record_arity() != Some(3) {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass import",
                    message: format!(
                        "`(Import <path> [<name>...])` for `{binding}` requires 3 positions"
                    ),
                });
            }
            let items = value.as_record().unwrap();
            let path = identifier_or_string(&items[1])?;
            let names_value = &items[2];
            if !names_value.is_sequence() {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass import",
                    message: format!("`(Import ...)` for `{binding}` requires a `[name ...]` list"),
                });
            }
            let names: Vec<Name> = names_value
                .as_sequence()
                .unwrap()
                .iter()
                .map(|item| {
                    let text = item
                        .identifier_text()
                        .ok_or_else(|| Error::InvalidSchemaText {
                            context: "multi_pass import",
                            message: format!(
                                "import name list for `{binding}` must be identifiers"
                            ),
                        })?;
                    Name::new(text)
                })
                .collect::<Result<_>>()?;
            return Ok(RecognizedImport {
                kind: ImportKind::Import,
                path,
                names,
            });
        }
        if value.is_tagged_record("ImportAll") {
            if value.record_arity() != Some(2) {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass import",
                    message: format!("`(ImportAll <path>)` for `{binding}` requires 2 positions"),
                });
            }
            let items = value.as_record().unwrap();
            let path = identifier_or_string(&items[1])?;
            // ImportAll without a resolved import set yields the
            // imported-binding name itself as the only known
            // exported name. This mirrors what `Schema::assemble`
            // does when invoked with `&[]` (no resolutions): the
            // imported type appears under the binding's own name.
            let names = vec![binding.clone()];
            return Ok(RecognizedImport {
                kind: ImportKind::ImportAll,
                path,
                names,
            });
        }
        Err(Error::InvalidSchemaText {
            context: "multi_pass import",
            message: format!(
                "unknown import directive `{}` for `{binding}`",
                value.record_head_identifier().unwrap_or("<?>"),
            ),
        })
    }
}

/// Builtin Type macro recognizer. Dispatches by SHAPE — sequence /
/// record-with-arity / bare identifier — into enum / record / newtype
/// / alias bodies. This is the load-bearing
/// `(F1 F2 …)` vs `(T)` vs `[V1 V2 …]` vs `bare-identifier` dispatch
/// from `reports/second-designer/170-schema-lowering-executor-model-2026-05-24.md` §2.
struct TypeMacroRecognizer;

impl TypeMacroRecognizer {
    fn recognize(value: &NotaValue) -> Result<DeclarationBody> {
        let NodeDefinitionShape::NamespaceValue(shape) =
            NodeDefinitionShape::recognize(NodeDefinitionPoint::NamespaceValue, value)?
        else {
            unreachable!("NamespaceValue point can only produce a NamespaceValue shape")
        };
        match shape {
            NamespaceValueShape::Enum => Self::recognize_enum(value),
            NamespaceValueShape::Record => Self::recognize_record(value),
            NamespaceValueShape::Newtype => Self::recognize_newtype(value),
            NamespaceValueShape::Alias => {
                // bare ident — alias to another type / primitive
                let expr = lower_type_expression(value)?;
                Ok(DeclarationBody::Alias(expr))
            }
        }
    }

    fn recognize_enum(value: &NotaValue) -> Result<DeclarationBody> {
        let variants_value = value.as_sequence().unwrap();
        let mut variants = Vec::new();
        for variant_value in variants_value {
            // Each variant dispatches on shape too: bare ident =
            // unit variant; record = data-carrying variant whose
            // first positional value is the variant name.
            if variant_value.is_identifier() {
                let name = Name::new(variant_value.identifier_text().unwrap())?;
                variants.push(Variant::unit(name));
            } else if variant_value.is_record() {
                let items = variant_value.as_record().unwrap();
                let head = variant_value.record_head_identifier().ok_or_else(|| {
                    Error::InvalidSchemaText {
                        context: "multi_pass type",
                        message: "data-carrying variant must start with an identifier".into(),
                    }
                })?;
                let name = Name::new(head)?;
                let fields = items[1..]
                    .iter()
                    .map(parse_field)
                    .collect::<Result<Vec<_>>>()?;
                variants.push(variant_with_fields(name, fields));
            } else {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass type",
                    message: "enum variants must be identifier or record".into(),
                });
            }
        }
        Ok(DeclarationBody::Enum { variants })
    }

    fn recognize_record(value: &NotaValue) -> Result<DeclarationBody> {
        Ok(DeclarationBody::Record(fields_from_record(value)?))
    }

    fn recognize_newtype(value: &NotaValue) -> Result<DeclarationBody> {
        let items = value.as_record().unwrap();
        if let [inner] = items {
            return Ok(DeclarationBody::Newtype(lower_type_expression(inner)?));
        }
        if value
            .record_head_identifier()
            .is_some_and(is_container_head)
        {
            return Ok(DeclarationBody::Newtype(lower_type_expression(value)?));
        }
        Err(Error::InvalidSchemaText {
            context: "multi_pass type",
            message: "newtype declaration must carry exactly one inferred type expression".into(),
        })
    }
}

fn fields_from_record(value: &NotaValue) -> Result<Vec<Field>> {
    let items = value.as_record().unwrap();
    if items.is_empty() {
        return Err(Error::InvalidSchemaText {
            context: "multi_pass type",
            message: "namespace record must carry at least one type expression".into(),
        });
    }
    items.iter().map(parse_field).collect::<Result<_>>()
}

/// Builtin Feature macro recognizer. Dispatches by record-head tag
/// (`Reply`, `Event`, `Observable`, `Upgrade`). Each tag opens its
/// own recognizer body. Shape-logic dispatch all the way down.
struct FeatureMacroRecognizer;

impl FeatureMacroRecognizer {
    fn recognize(value: &NotaValue) -> Result<Feature> {
        if !value.is_record() {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass feature",
                message: "feature must be a record".into(),
            });
        }
        let head = value
            .record_head_identifier()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "multi_pass feature",
                message: "feature record must start with an identifier".into(),
            })?;
        let items = value.as_record().unwrap();
        match head {
            "Reply" => {
                let names = items[1..]
                    .iter()
                    .map(|item| {
                        let text =
                            item.identifier_text()
                                .ok_or_else(|| Error::InvalidSchemaText {
                                    context: "multi_pass feature",
                                    message: "Reply payload must be identifiers".into(),
                                })?;
                        Name::new(text)
                    })
                    .collect::<Result<_>>()?;
                Ok(Feature::Reply(names))
            }
            "Event" => {
                let (stream, start) = if let Some(first) = items.get(1)
                    && first.is_record()
                    && first.record_head_identifier() == Some("belongs")
                {
                    if first.record_arity() != Some(2) {
                        return Err(Error::InvalidSchemaText {
                            context: "multi_pass feature",
                            message: "`(belongs <name>)` requires two positions".into(),
                        });
                    }
                    let stream_name =
                        first.as_record().unwrap()[1]
                            .identifier_text()
                            .ok_or_else(|| Error::InvalidSchemaText {
                                context: "multi_pass feature",
                                message: "`(belongs <name>)` requires an identifier".into(),
                            })?;
                    (Some(Name::new(stream_name)?), 2)
                } else {
                    (None, 1)
                };
                let events = items[start..]
                    .iter()
                    .map(|item| {
                        let text =
                            item.identifier_text()
                                .ok_or_else(|| Error::InvalidSchemaText {
                                    context: "multi_pass feature",
                                    message: "Event names must be identifiers".into(),
                                })?;
                        Name::new(text)
                    })
                    .collect::<Result<_>>()?;
                Ok(Feature::Event(EventFeature::new(stream, events)))
            }
            "Observable" => {
                let mut filter = None;
                let mut operation_event = None;
                let mut effect_event = None;
                for field in &items[1..] {
                    if !field.is_record() || field.record_arity() != Some(2) {
                        return Err(Error::InvalidSchemaText {
                            context: "multi_pass feature",
                            message: "Observable field must be a 2-position record".into(),
                        });
                    }
                    let key =
                        field
                            .record_head_identifier()
                            .ok_or_else(|| Error::InvalidSchemaText {
                                context: "multi_pass feature",
                                message: "Observable field needs key identifier".into(),
                            })?;
                    let value_item = &field.as_record().unwrap()[1];
                    let value_text =
                        value_item
                            .identifier_text()
                            .ok_or_else(|| Error::InvalidSchemaText {
                                context: "multi_pass feature",
                                message: format!("Observable `{key}` value must be identifier"),
                            })?;
                    match key {
                        "filter" => filter = Some(value_text.to_string()),
                        "operation_event" => operation_event = Some(Name::new(value_text)?),
                        "effect_event" => effect_event = Some(Name::new(value_text)?),
                        other => {
                            return Err(Error::InvalidSchemaText {
                                context: "multi_pass feature",
                                message: format!("unknown Observable field `{other}`"),
                            });
                        }
                    }
                }
                Ok(Feature::Observable(ObservableFeature::new(
                    filter,
                    operation_event,
                    effect_event,
                )))
            }
            "Upgrade" => UpgradeFeatureRecognizer::recognize(value).map(Feature::Upgrade),
            other => Err(Error::InvalidSchemaText {
                context: "multi_pass feature",
                message: format!("unknown feature `{other}`"),
            }),
        }
    }
}

struct UpgradeFeatureRecognizer;

impl UpgradeFeatureRecognizer {
    fn recognize(value: &NotaValue) -> Result<Upgrade> {
        let items = value.as_record().unwrap();
        let from_version_value = items.get(1).ok_or_else(|| Error::InvalidSchemaText {
            context: "multi_pass upgrade",
            message: "Upgrade requires FromVersion".into(),
        })?;
        if !from_version_value.is_tagged_record("FromVersion") {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass upgrade",
                message: "Upgrade first sub-record must be FromVersion".into(),
            });
        }
        if from_version_value.record_arity() != Some(2) {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass upgrade",
                message: "FromVersion requires exactly one path argument".into(),
            });
        }
        let path_value = &from_version_value.as_record().unwrap()[1];
        let path = identifier_or_string(path_value)?;
        let version = Version::new(path);
        let mut annotations = Vec::new();
        for annotation_value in &items[2..] {
            annotations.push(Self::recognize_annotation(annotation_value)?);
        }
        Ok(Upgrade::new(version, annotations))
    }

    fn recognize_annotation(value: &NotaValue) -> Result<UpgradeAnnotation> {
        if !value.is_record() {
            return Err(Error::InvalidSchemaText {
                context: "multi_pass upgrade annotation",
                message: "annotation must be a record".into(),
            });
        }
        let head = value
            .record_head_identifier()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "multi_pass upgrade annotation",
                message: "annotation needs head identifier".into(),
            })?;
        let items = value.as_record().unwrap();
        match head {
            "Migrate" => Ok(UpgradeAnnotation::Migrate(Name::new(require_ident(
                items.get(1),
                "Migrate type",
            )?)?)),
            "RenamedFrom" => Ok(UpgradeAnnotation::RenamedFrom {
                current: Name::new(require_ident(items.get(1), "RenamedFrom current")?)?,
                previous: Name::new(require_ident(items.get(2), "RenamedFrom previous")?)?,
            }),
            "Drop" => Ok(UpgradeAnnotation::Drop(Name::new(require_ident(
                items.get(1),
                "Drop type",
            )?)?)),
            "Custom" => Ok(UpgradeAnnotation::Custom {
                name: Name::new(require_ident(items.get(1), "Custom name")?)?,
                implementation: Name::new(require_ident(items.get(2), "Custom impl")?)?,
            }),
            "Untranslatable" => Ok(UpgradeAnnotation::Untranslatable(Name::new(
                require_ident(items.get(1), "Untranslatable type")?,
            )?)),
            other => Err(Error::InvalidSchemaText {
                context: "multi_pass upgrade annotation",
                message: format!("unknown upgrade annotation `{other}`"),
            }),
        }
    }
}

fn require_ident<'value>(value: Option<&'value NotaValue>, what: &str) -> Result<&'value str> {
    value
        .and_then(|item| item.identifier_text())
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "multi_pass upgrade annotation",
            message: format!("{what} must be an identifier"),
        })
}

fn resolve_endpoint_body_from_namespace(
    root: &Name,
    endpoint: &Name,
    bodies: &BTreeMap<Name, DeclarationBody>,
) -> Result<(RouteBody, Option<Engine>)> {
    let body = bodies.get(root).ok_or_else(|| Error::InvalidSchemaText {
        context: "multi_pass header lowering",
        message: format!("root `{root}` has no namespace declaration"),
    })?;
    let DeclarationBody::Enum { variants } = body else {
        return Err(Error::InvalidSchemaText {
            context: "multi_pass header lowering",
            message: format!("root `{root}` must be an enum"),
        });
    };
    let target = variants
        .iter()
        .find(|candidate| candidate.name() == endpoint)
        .ok_or_else(|| Error::InvalidSchemaText {
            context: "multi_pass header lowering",
            message: format!("endpoint `{endpoint}` not found in root `{root}`"),
        })?;
    match target.payload() {
        Payload::Unit => Ok((RouteBody::Unit, target.engine())),
        Payload::Type(TypeExpression::Named(name)) => {
            Ok((RouteBody::Type(name.clone()), target.engine()))
        }
        Payload::Type(_) | Payload::Fields(_) => Err(Error::InvalidSchemaText {
            context: "multi_pass header lowering",
            message: format!("endpoint `{root}.{endpoint}` must resolve to a named body type"),
        }),
    }
}

fn parse_field(value: &NotaValue) -> Result<Field> {
    // A field is either:
    //  - a bare identifier (positional field, name = type) — handled
    //    by lower_type_expression below;
    //  - a `(Container ...)` record (Option / Vec / Map) which is
    //    ITSELF the type expression.
    if value.is_record() {
        let items = value.as_record().unwrap();
        let head_value = items.first().ok_or_else(|| Error::InvalidSchemaText {
            context: "multi_pass field",
            message: "field record must be non-empty".into(),
        })?;
        let head_text = head_value
            .identifier_text()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "multi_pass field",
                message: "field record must start with an identifier".into(),
            })?;
        if is_container_head(head_text) {
            let expr = lower_container_expression_after_head(head_text, &items[1..])?;
            return Ok(Field::inferred(expr));
        }
        return Err(Error::InvalidSchemaText {
            context: "multi_pass field",
            message: format!(
                "record field expressions are positional; `{head_text}` is not a container type"
            ),
        });
    }
    let expr = lower_type_expression(value)?;
    Ok(Field::inferred(expr))
}

fn lower_type_expression(value: &NotaValue) -> Result<TypeExpression> {
    if value.is_identifier() {
        let text = value.identifier_text().unwrap();
        if let Some(primitive) = primitive(text) {
            return Ok(primitive);
        }
        return Name::new(text).map(TypeExpression::named);
    }
    if value.is_record() {
        let items = value.as_record().unwrap();
        let head_text = items
            .first()
            .and_then(|item| item.identifier_text())
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "multi_pass type expression",
                message: "container record must start with identifier".into(),
            })?;
        return lower_container_expression_after_head(head_text, &items[1..]);
    }
    Err(Error::InvalidSchemaText {
        context: "multi_pass type expression",
        message: format!("cannot lower `{value:?}` as a type expression"),
    })
}

fn lower_container_expression_after_head(head: &str, rest: &[NotaValue]) -> Result<TypeExpression> {
    match head {
        "Option" => {
            if rest.len() != 1 {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass type expression",
                    message: "Option must wrap exactly one inner expression".into(),
                });
            }
            Ok(TypeExpression::optional(lower_type_expression(&rest[0])?))
        }
        "Vec" => {
            if rest.len() != 1 {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass type expression",
                    message: "Vec must wrap exactly one inner expression".into(),
                });
            }
            Ok(TypeExpression::vector(lower_type_expression(&rest[0])?))
        }
        "Map" => {
            if rest.len() != 2 {
                return Err(Error::InvalidSchemaText {
                    context: "multi_pass type expression",
                    message: "Map needs key and value expressions".into(),
                });
            }
            Ok(TypeExpression::map(
                lower_type_expression(&rest[0])?,
                lower_type_expression(&rest[1])?,
            ))
        }
        other => Err(Error::InvalidSchemaText {
            context: "multi_pass type expression",
            message: format!("unknown container `{other}`"),
        }),
    }
}

fn primitive(text: &str) -> Option<TypeExpression> {
    let primitive = match text {
        "String" => Primitive::String,
        "Bytes" => Primitive::Bytes,
        "Boolean" | "bool" => Primitive::Boolean,
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

fn variant_with_fields(name: Name, fields: Vec<Field>) -> Variant {
    match fields.len() {
        0 => Variant::unit(name),
        1 => Variant::with_type(
            name,
            fields.into_iter().next().unwrap().expression().clone(),
        ),
        _ => Variant::with_field_entries(name, fields),
    }
}

fn identifier_or_string(value: &NotaValue) -> Result<String> {
    if let Some(text) = value.identifier_text() {
        return Ok(text.to_string());
    }
    if let Some(text) = value.string_text() {
        return Ok(text.to_string());
    }
    Err(Error::InvalidSchemaText {
        context: "multi_pass",
        message: format!("expected identifier or string, got `{value:?}`"),
    })
}

fn is_container_head(text: &str) -> bool {
    matches!(text, "Option" | "Vec" | "Map")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NotaKind {
    Map,
    Sequence,
}

fn expect_kind(name: &'static str, value: &NotaValue, expected: NotaKind) -> Result<()> {
    let matches = match expected {
        NotaKind::Map => value.is_map(),
        NotaKind::Sequence => value.is_sequence(),
    };
    if matches {
        Ok(())
    } else {
        Err(Error::InvalidSchemaText {
            context: "multi_pass structural pass",
            message: format!(
                "position `{name}` expected {expected:?}, got {:?}",
                value.kind()
            ),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────
// Meta-circular extension example — `(Storage [...])` user macro.
// ─────────────────────────────────────────────────────────────────────
//
// The builtin macros above are themselves "macros for reading macros":
// each takes a `NotaValue` and uses the SAME shape-logic predicates a
// user-defined macro would use. The pattern below sketches how a
// downstream consumer adds a `(Storage [(TableName StoredType) ...])`
// feature variant without touching the builtin engine:
//
// ```text
// pub struct StorageMacro;
//
// impl StorageMacro {
//     /// Same shape-logic dispatch the builtins use. The user macro
//     /// asks NotaValue what shape it has and rejects unknown shapes
//     /// with the same `Error::InvalidSchemaText { context, message }`
//     /// the engine emits.
//     pub fn recognize(value: &NotaValue) -> Result<Vec<StorageTable>> {
//         if !value.is_tagged_record("Storage") {
//             return Err(Error::InvalidSchemaText {
//                 context: "user storage macro",
//                 message: "Storage feature must be `(Storage [...])`".into(),
//             });
//         }
//         let body = &value.as_record().unwrap()[1];
//         if !body.is_sequence() {
//             return Err(Error::InvalidSchemaText {
//                 context: "user storage macro",
//                 message: "Storage body must be a sequence of `(TableName StoredType)` records"
//                     .into(),
//             });
//         }
//         let mut tables = Vec::new();
//         for entry in body.as_sequence().unwrap() {
//             // shape: `(TableName StoredType)` — arity 2, both
//             // PascalCase identifiers. Same `record_arity`,
//             // `record_head_identifier`, `is_pascal_case_identifier`
//             // predicates the builtins use.
//             if !entry.is_record() || entry.record_arity() != Some(2) {
//                 return Err(Error::InvalidSchemaText {
//                     context: "user storage macro",
//                     message: "Storage entry must be `(TableName StoredType)` record".into(),
//                 });
//             }
//             let items = entry.as_record().unwrap();
//             let table_name = Name::new(items[0].identifier_text().unwrap())?;
//             let stored_type = Name::new(items[1].identifier_text().unwrap())?;
//             tables.push(StorageTable { table_name, stored_type });
//         }
//         Ok(tables)
//     }
// }
// ```
//
// The user wires `StorageMacro::recognize` into the engine by adding
// a new arm in `FeatureMacroRecognizer::recognize` (or by registering
// the recognizer through the `SchemaMacro` trait surface per
// `reports/designer/329-schema-macro-component-extensibility.md` §4).
// No change to the dispatch substrate; the user macro plugs in as
// peer to the builtins because they share the shape-logic vocabulary.
//
// This is the meta-circular property: the builtins ARE EXPRESSED as
// macros over NotaValue, so user macros plug into the same
// substrate. See report §6 for the full demonstration.

// `Route` and `Endpoint` are re-exported via `crate::{Route,
// Endpoint}` already; this keeps Cargo's dead-code lint quiet for the
// rare reader who imports just `multi_pass`.
#[allow(dead_code)]
fn _re_export_marker() {
    let _: Option<&Route> = None;
    let _: Option<&Endpoint> = None;
}
