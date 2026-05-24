use std::collections::BTreeMap;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode};
use schema::{
    Declaration, Engine, Error, Feature, Header, HeaderRoot, ImportDirective, ImportResolution,
    Imports, Layout, Leg, Name, Namespace, Payload, Primitive, Projection, RouteBody, Schema,
    SchemaPath, StandardProjection, TypeExpression, Upgrade, UpgradeAnnotation, Variant, Version,
};

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn named(value: &str) -> TypeExpression {
    TypeExpression::named(name(value))
}

fn string() -> TypeExpression {
    TypeExpression::Primitive(Primitive::String)
}

#[test]
fn validates_spirit_mvp_uniform_header_and_lowers_routes() {
    let schema = Builder::spirit_mvp(Vec::new()).build().unwrap();
    let assembled = schema
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();

    assert_eq!(assembled.routes().len(), 2);

    let state = &assembled.routes()[0];
    assert_eq!(state.leg(), Leg::Ordinary);
    assert_eq!(state.root_slot(), 0);
    assert_eq!(state.root().as_str(), "State");
    assert_eq!(state.endpoint().slot(), 0);
    assert_eq!(state.endpoint().name().as_str(), "Statement");
    assert_eq!(state.body(), &RouteBody::Type(name("Statement")));

    let record = schema.variant(&name("Record"), &name("Entry")).unwrap();
    assert_eq!(record.engine(), Some(Engine::Assert));
    assert_eq!(record.payload(), &Payload::Type(named("Entry")));
}

#[test]
fn rejects_duplicate_declaration_names() {
    let result = Namespace::declarations(vec![
        Declaration::enumeration(name("Kind"), vec![Variant::unit(name("Decision"))]),
        Declaration::newtype(name("Kind"), string()),
    ]);

    assert!(matches!(result, Err(Error::DuplicateDeclaration { name }) if name.as_str() == "Kind"));
}

#[test]
fn route_root_body_declaration_reserves_namespace_key() {
    let result = Namespace::declarations(vec![
        Declaration::record(name("State"), vec![named("Presence")]),
        Declaration::enumeration(
            name("State"),
            vec![Variant::with_type(name("Statement"), named("Statement"))],
        ),
    ]);

    assert!(
        matches!(result, Err(Error::DuplicateDeclaration { name }) if name.as_str() == "State")
    );
}

#[test]
fn rejects_empty_header_root() {
    let result = HeaderRoot::new(name("State"), Vec::new());

    assert!(matches!(result, Err(Error::EmptyHeaderRoot { name }) if name.as_str() == "State"));
}

#[test]
fn rejects_duplicate_variant_names() {
    let result = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::enumeration(
            name("Kind"),
            vec![
                Variant::unit(name("Decision")),
                Variant::unit(name("Decision")),
            ],
        )])
        .unwrap(),
        Vec::new(),
    );

    assert!(
        matches!(result, Err(Error::DuplicateVariant { declaration, variant })
            if declaration.as_str() == "Kind" && variant.as_str() == "Decision")
    );
}

#[test]
fn rejects_unmatched_route_body_variant() {
    let schema = Schema::new(
        Imports::empty(),
        Header::new(vec![
            HeaderRoot::new(name("Watch"), vec![name("Records")]).unwrap(),
        ])
        .unwrap(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![
            Declaration::enumeration(
                name("Watch"),
                vec![
                    Variant::with_type(name("Records"), named("RecordSubscription")),
                    Variant::with_type(name("Questions"), named("QuestionSubscription")),
                ],
            ),
            Declaration::newtype(name("RecordSubscription"), string()),
            Declaration::newtype(name("QuestionSubscription"), string()),
        ])
        .unwrap(),
        Vec::new(),
    )
    .unwrap();

    let result = schema.assemble(&[]);

    assert!(
        matches!(result, Err(Error::UnmatchedRouteBodyVariant { root, variant })
            if root.as_str() == "Watch" && variant.as_str() == "Questions")
    );
}

#[test]
fn rejects_unknown_named_type() {
    let result = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::record(
            name("Entry"),
            vec![named("Topic")],
        )])
        .unwrap(),
        Vec::new(),
    );

    assert!(matches!(result, Err(Error::UnknownType { name }) if name.as_str() == "Topic"));
}

#[test]
fn rejects_unknown_reply_feature_type() {
    let result = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(Vec::new()).unwrap(),
        vec![Feature::Reply(vec![name("RecordAccepted")])],
    );

    assert!(
        matches!(result, Err(Error::UnknownType { name }) if name.as_str() == "RecordAccepted")
    );
}

#[test]
fn layout_places_fixed_fields_in_root_and_growing_fields_in_boxes() {
    let schema = Builder::spirit_mvp(Vec::new()).build().unwrap();
    let layout = Layout::for_declaration(&schema, &name("Entry")).unwrap();

    assert_eq!(layout.root_positions(), vec![1]);
    assert_eq!(layout.box_positions(), vec![0, 2, 3, 4, 5]);
}

#[test]
fn import_all_requires_resolution_before_assembly() {
    let schema = Builder::spirit_mvp(Vec::new()).build().unwrap();

    let result = schema.assemble(&[]);
    assert!(
        matches!(result, Err(Error::MissingImportResolution { binding }) if binding.as_str() == "Magnitude")
    );
}

#[test]
fn selected_import_collisions_are_loud() {
    let imports = Imports::new(vec![
        (
            name("SemaA"),
            ImportDirective::import(
                SchemaPath::new("../signal-sema/operation.schema"),
                vec![name("SemaOperation")],
            ),
        ),
        (
            name("SemaB"),
            ImportDirective::import(
                SchemaPath::new("../other/operation.schema"),
                vec![name("SemaOperation")],
            ),
        ),
    ])
    .unwrap();

    let result = Schema::new(
        imports,
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(Vec::new()).unwrap(),
        Vec::new(),
    );

    assert!(
        matches!(result, Err(Error::DuplicateImportedName { name, first_binding, second_binding })
            if name.as_str() == "SemaOperation"
                && first_binding.as_str() == "SemaA"
                && second_binding.as_str() == "SemaB")
    );
}

#[test]
fn import_local_collisions_are_loud() {
    let imports = Imports::new(vec![(
        name("SemaSet"),
        ImportDirective::import(
            SchemaPath::new("../signal-sema/operation.schema"),
            vec![name("SemaOperation")],
        ),
    )])
    .unwrap();

    let result = Schema::new(
        imports,
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::enumeration(
            name("SemaOperation"),
            Vec::new(),
        )])
        .unwrap(),
        Vec::new(),
    );

    assert!(
        matches!(result, Err(Error::ImportCollisionWithLocal { name, binding })
            if name.as_str() == "SemaOperation" && binding.as_str() == "SemaSet")
    );
}

#[test]
fn additive_enum_variant_gets_standard_upgrade_projection() {
    let previous = Builder::spirit_mvp(Vec::new())
        .with_kind_variants(vec!["Decision", "Principle"])
        .build()
        .unwrap()
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();
    let current = Builder::spirit_mvp(Vec::new())
        .with_kind_variants(vec!["Decision", "Principle", "Correction"])
        .build()
        .unwrap()
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Standard {
            name,
            kind: StandardProjection::AdditiveEnumVariant
        } if name.as_str() == "Kind"
    )));
}

#[test]
fn changed_record_requires_upgrade_annotation() {
    let previous = Builder::spirit_mvp(Vec::new())
        .build()
        .unwrap()
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();
    let current = Builder::spirit_mvp(Vec::new())
        .with_entry_extra_field(named("Source"))
        .build()
        .unwrap()
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();

    let result = current.plan_upgrade_from(&previous);

    assert!(
        matches!(result, Err(Error::MissingUpgradeAnnotation { name }) if name.as_str() == "Entry")
    );
}

#[test]
fn migrate_annotation_allows_changed_record_projection() {
    let previous = Builder::spirit_mvp(Vec::new())
        .build()
        .unwrap()
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();
    let current = Builder::spirit_mvp(vec![Feature::Upgrade(Upgrade::new(
        Version::new("v0.1.1"),
        vec![UpgradeAnnotation::Migrate(name("Entry"))],
    ))])
    .with_entry_extra_field(named("Source"))
    .build()
    .unwrap()
    .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
    .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Annotated { name, annotation: UpgradeAnnotation::Migrate(migrated) }
            if name.as_str() == "Entry" && migrated.as_str() == "Entry"
    )));
}

#[test]
fn nota_curly_map_is_usable_for_schema_namespace_names() {
    let mut decoder = Decoder::new("{Entry 1 Record 2}");
    let decoded = BTreeMap::<Name, u64>::decode(&mut decoder).unwrap();

    assert_eq!(decoded.get(&name("Entry")), Some(&1));
    assert_eq!(decoded.get(&name("Record")), Some(&2));

    let mut encoder = Encoder::new();
    decoded.encode(&mut encoder).unwrap();
    assert_eq!(encoder.into_string(), "{Entry 1 Record 2}");
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.4):
// Header root `(Watch [State Records Questions])` with namespace
// `Watch [(State StateSubscription) (Records RecordSubscription) (Questions QuestionSubscription)]`
// is the architectural seam the multi-sub-variant header form was designed
// around (per /174-v5 "Better Separation"). The pre-existing test only
// exercised single-sub-variant routes; this one proves all three
// sub-variants lower independently with monotonic endpoint slots and
// distinct route bodies.
#[test]
fn multi_sub_variant_header_lowers_to_three_distinct_routes() {
    let schema = Schema::new(
        Imports::empty(),
        Header::new(vec![
            HeaderRoot::new(
                name("Watch"),
                vec![name("State"), name("Records"), name("Questions")],
            )
            .unwrap(),
        ])
        .unwrap(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![
            Declaration::enumeration(
                name("Watch"),
                vec![
                    Variant::with_type(name("State"), named("StateSubscription"))
                        .with_engine(Engine::Subscribe),
                    Variant::with_type(name("Records"), named("RecordSubscription"))
                        .with_engine(Engine::Subscribe),
                    Variant::with_type(name("Questions"), named("QuestionSubscription"))
                        .with_engine(Engine::Subscribe),
                ],
            ),
            Declaration::newtype(name("StateSubscription"), string()),
            Declaration::newtype(name("RecordSubscription"), string()),
            Declaration::newtype(name("QuestionSubscription"), string()),
        ])
        .unwrap(),
        Vec::new(),
    )
    .unwrap();

    let assembled = schema.assemble(&[]).unwrap();
    let routes = assembled.routes();

    assert_eq!(routes.len(), 3);

    for route in routes {
        assert_eq!(route.leg(), Leg::Ordinary);
        assert_eq!(route.root_slot(), 0);
        assert_eq!(route.root().as_str(), "Watch");
        assert_eq!(route.engine(), Some(Engine::Subscribe));
    }

    assert_eq!(routes[0].endpoint().slot(), 0);
    assert_eq!(routes[0].endpoint().name().as_str(), "State");
    assert_eq!(
        routes[0].body(),
        &RouteBody::Type(name("StateSubscription"))
    );

    assert_eq!(routes[1].endpoint().slot(), 1);
    assert_eq!(routes[1].endpoint().name().as_str(), "Records");
    assert_eq!(
        routes[1].body(),
        &RouteBody::Type(name("RecordSubscription"))
    );

    assert_eq!(routes[2].endpoint().slot(), 2);
    assert_eq!(routes[2].endpoint().name().as_str(), "Questions");
    assert_eq!(
        routes[2].body(),
        &RouteBody::Type(name("QuestionSubscription"))
    );
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.5):
// Non-Ordinary leg coverage. Owner header routes carry leg=Leg::Owner;
// sema header routes carry leg=Leg::Sema. Without this smoke test we never
// verified non-Ordinary legs lower at all — the lower_header dispatch is
// generic but the test corpus only exercised one leg variant.
#[test]
fn owner_header_lowers_with_owner_leg() {
    let schema = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::new(vec![
            HeaderRoot::new(name("Configure"), vec![name("PolicyUpdate")]).unwrap(),
        ])
        .unwrap(),
        Header::empty(),
        Namespace::declarations(vec![
            Declaration::enumeration(
                name("Configure"),
                vec![
                    Variant::with_type(name("PolicyUpdate"), named("PolicyChange"))
                        .with_engine(Engine::Mutate),
                ],
            ),
            Declaration::newtype(name("PolicyChange"), string()),
        ])
        .unwrap(),
        Vec::new(),
    )
    .unwrap();

    let assembled = schema.assemble(&[]).unwrap();
    let routes = assembled.routes();

    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.leg(), Leg::Owner);
    assert_eq!(route.root().as_str(), "Configure");
    assert_eq!(route.endpoint().name().as_str(), "PolicyUpdate");
    assert_eq!(route.body(), &RouteBody::Type(name("PolicyChange")));
    assert_eq!(route.engine(), Some(Engine::Mutate));
}

#[test]
fn sema_header_lowers_with_sema_leg() {
    let schema = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::new(vec![
            HeaderRoot::new(name("Reflect"), vec![name("Observe")]).unwrap(),
        ])
        .unwrap(),
        Namespace::declarations(vec![
            Declaration::enumeration(
                name("Reflect"),
                vec![
                    Variant::with_type(name("Observe"), named("Observation"))
                        .with_engine(Engine::Validate),
                ],
            ),
            Declaration::newtype(name("Observation"), string()),
        ])
        .unwrap(),
        Vec::new(),
    )
    .unwrap();

    let assembled = schema.assemble(&[]).unwrap();
    let routes = assembled.routes();

    assert_eq!(routes.len(), 1);
    let route = &routes[0];
    assert_eq!(route.leg(), Leg::Sema);
    assert_eq!(route.root().as_str(), "Reflect");
    assert_eq!(route.endpoint().name().as_str(), "Observe");
    assert_eq!(route.body(), &RouteBody::Type(name("Observation")));
    assert_eq!(route.engine(), Some(Engine::Validate));
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.2 + §6):
// Engine annotations declared on namespace-side Variants must reach the
// lowered Route table. Without this, the macro library (per /324 §3.1)
// cannot build an engine-driven dispatch table. The test checks the
// per-route accessor AND the routes_by_engine helper.
#[test]
fn engine_annotations_thread_through_to_routes() {
    let schema = Builder::spirit_mvp(Vec::new()).build().unwrap();
    let assembled = schema
        .assemble(&[ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()])
        .unwrap();

    let routes = assembled.routes();
    assert_eq!(routes.len(), 2);
    assert_eq!(routes[0].engine(), Some(Engine::Assert));
    assert_eq!(routes[1].engine(), Some(Engine::Assert));

    let assert_routes: Vec<_> = assembled.routes_by_engine(Engine::Assert).collect();
    assert_eq!(assert_routes.len(), 2);

    let mutate_routes: Vec<_> = assembled.routes_by_engine(Engine::Mutate).collect();
    assert!(mutate_routes.is_empty());
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.6):
// Renamed projection chases the RenamedFrom annotation. Previous schema
// has Utterance, current schema declares Statement + carries
// RenamedFrom { current: Statement, previous: Utterance }. Projection is
// Projection::Renamed { current, previous }; the previous Utterance is
// consumed by the rename and does NOT show up as Dropped.
#[test]
fn renamed_annotation_produces_renamed_projection() {
    let previous = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::newtype(name("Utterance"), string())]).unwrap(),
        Vec::new(),
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let current = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::newtype(name("Statement"), string())]).unwrap(),
        vec![Feature::Upgrade(Upgrade::new(
            Version::new("v0.2.0"),
            vec![UpgradeAnnotation::RenamedFrom {
                current: name("Statement"),
                previous: name("Utterance"),
            }],
        ))],
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Renamed { current, previous }
            if current.as_str() == "Statement" && previous.as_str() == "Utterance"
    )));

    assert!(!plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Dropped { name } if name.as_str() == "Utterance"
    )));
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.6):
// Drop annotation on a removed type produces Projection::Dropped, not the
// RemovedTypeRequiresAnnotation error path.
#[test]
fn drop_annotation_produces_dropped_projection() {
    let previous = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::newtype(name("Reflection"), string())]).unwrap(),
        Vec::new(),
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let current = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(Vec::new()).unwrap(),
        vec![Feature::Upgrade(Upgrade::new(
            Version::new("v0.2.0"),
            vec![UpgradeAnnotation::Drop(name("Reflection"))],
        ))],
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Dropped { name } if name.as_str() == "Reflection"
    )));
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.6):
// Untranslatable annotation marks a removed type that cannot be migrated.
// Resolves to Projection::Untranslatable { name }.
#[test]
fn untranslatable_annotation_produces_untranslatable_projection() {
    let previous = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::newtype(name("Reflection"), string())]).unwrap(),
        Vec::new(),
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let current = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(Vec::new()).unwrap(),
        vec![Feature::Upgrade(Upgrade::new(
            Version::new("v0.2.0"),
            vec![UpgradeAnnotation::Untranslatable(name("Reflection"))],
        ))],
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Untranslatable { name } if name.as_str() == "Reflection"
    )));
}

// DESIGN-DECISION-REVIEW (second-designer/172 §3.2 + /171 §4.6):
// Removed type with NO Drop or Untranslatable annotation is an upgrade
// error. The error name carries the offending type so callers can point
// the psyche at exactly which decision they need to make.
#[test]
fn removed_type_without_annotation_errors() {
    let previous = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(vec![Declaration::newtype(name("Reflection"), string())]).unwrap(),
        Vec::new(),
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let current = Schema::new(
        Imports::empty(),
        Header::empty(),
        Header::empty(),
        Header::empty(),
        Namespace::declarations(Vec::new()).unwrap(),
        Vec::new(),
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let result = current.plan_upgrade_from(&previous);

    assert!(matches!(
        result,
        Err(Error::RemovedTypeRequiresAnnotation { name }) if name.as_str() == "Reflection"
    ));
}

struct Builder {
    features: Vec<Feature>,
    kind_variants: Vec<&'static str>,
    entry_extra_field: Option<TypeExpression>,
}

impl Builder {
    fn spirit_mvp(features: Vec<Feature>) -> Self {
        Self {
            features,
            kind_variants: vec!["Decision", "Principle", "Correction"],
            entry_extra_field: None,
        }
    }

    fn with_kind_variants(mut self, variants: Vec<&'static str>) -> Self {
        self.kind_variants = variants;
        self
    }

    fn with_entry_extra_field(mut self, field: TypeExpression) -> Self {
        self.entry_extra_field = Some(field);
        self
    }

    fn build(self) -> schema::Result<Schema> {
        Schema::new(
            Imports::new(vec![(
                name("Magnitude"),
                ImportDirective::import_all(SchemaPath::new("../signal-sema/magnitude.schema")),
            )])?,
            Header::new(vec![
                HeaderRoot::new(name("State"), vec![name("Statement")])?,
                HeaderRoot::new(name("Record"), vec![name("Entry")])?,
            ])?,
            Header::empty(),
            Header::empty(),
            Namespace::declarations(self.declarations())?,
            self.features,
        )
    }

    fn declarations(&self) -> Vec<Declaration> {
        let mut entry_fields = vec![
            named("Topic"),
            named("Kind"),
            named("Summary"),
            named("Context"),
            named("Magnitude"),
            named("Quote"),
        ];
        if let Some(field) = &self.entry_extra_field {
            entry_fields.push(field.clone());
        }

        let mut declarations = vec![
            Declaration::enumeration(
                name("State"),
                vec![
                    Variant::with_type(name("Statement"), named("Statement"))
                        .with_engine(Engine::Assert),
                ],
            ),
            Declaration::enumeration(
                name("Record"),
                vec![Variant::with_type(name("Entry"), named("Entry")).with_engine(Engine::Assert)],
            ),
            Declaration::enumeration(
                name("Kind"),
                self.kind_variants
                    .iter()
                    .map(|variant| Variant::unit(name(variant)))
                    .collect(),
            ),
            Declaration::newtype(name("Topic"), string()),
            Declaration::newtype(name("Summary"), string()),
            Declaration::newtype(name("Context"), string()),
            Declaration::newtype(name("Quote"), string()),
            Declaration::newtype(name("StatementText"), string()),
            Declaration::record(name("Entry"), entry_fields),
            Declaration::newtype(name("Statement"), named("StatementText")),
        ];

        if self.entry_extra_field.is_some() {
            declarations.push(Declaration::newtype(name("Source"), string()));
        }

        declarations
    }
}
