use std::collections::BTreeMap;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode};
use schema::{
    BuiltinMacroVariant, Declaration, DeclarationBody, Engine, Error, Feature, Field, FieldName,
    Header, HeaderEndpointInput, HeaderInput, HeaderRoot, ImportDirective, ImportResolution,
    Imports, Layout, Leg, LoweringContext, Name, Namespace, NodeDefinitionPoint, Payload,
    Primitive, Projection, RouteBody, Schema, SchemaPath, StandardProjection, TypeExpression,
    TypeInput, Upgrade, UpgradeAnnotation, UpgradeRuleInput, Variant, Version,
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
    assert_eq!(state.engine(), Some(Engine::Assert));
    assert_eq!(state.short_header().unwrap(), 0);

    let record = schema.variant(&name("Record"), &name("Entry")).unwrap();
    assert_eq!(record.engine(), Some(Engine::Assert));
    assert_eq!(record.payload(), &Payload::Type(named("Entry")));

    let record_route = &assembled.routes()[1];
    assert_eq!(record_route.engine(), Some(Engine::Assert));
    assert_eq!(record_route.short_header().unwrap(), 1);
    assert_eq!(
        assembled.route_for_short_header(Leg::Ordinary, 1).unwrap(),
        record_route
    );
}

#[test]
fn builtin_macro_variants_lower_into_assembled_schema_fragments() {
    let mut context = LoweringContext::new();

    context
        .apply(BuiltinMacroVariant::Type(TypeInput::local(
            name("Entry"),
            DeclarationBody::Record(vec![
                Field::inferred(named("Topic")),
                Field::inferred(named("Kind")),
            ]),
        )))
        .unwrap();
    context
        .apply(BuiltinMacroVariant::Header(HeaderInput::new(
            Leg::Ordinary,
            0,
            name("Record"),
            vec![HeaderEndpointInput::new(
                2,
                name("Entry"),
                RouteBody::Type(name("Entry")),
                Some(Engine::Assert),
            )],
        )))
        .unwrap();

    let assembled = context.finish();
    let route = assembled.routes().first().unwrap();

    assert_eq!(route.root().as_str(), "Record");
    assert_eq!(route.endpoint().name().as_str(), "Entry");
    assert_eq!(route.engine(), Some(Engine::Assert));
    assert_eq!(route.short_header().unwrap(), 512);
    assert_eq!(
        assembled.route_for_short_header(Leg::Ordinary, 512),
        Some(route)
    );
}

#[test]
fn upgrade_rule_macro_variant_lowers_into_assembled_upgrade_feature() {
    let upgrade = Upgrade::new(
        Version::new("v0.1"),
        vec![UpgradeAnnotation::Migrate(name("Entry"))],
    );
    let variant = BuiltinMacroVariant::UpgradeRule(UpgradeRuleInput::new(upgrade.clone()));
    assert_eq!(variant.point(), NodeDefinitionPoint::UpgradeRule);

    let mut context = LoweringContext::new();
    context.apply(variant).unwrap();
    let assembled = context.finish();

    assert_eq!(assembled.features(), &[Feature::Upgrade(upgrade)]);
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
fn parser_accepts_bool_as_boolean_primitive_alias() {
    let schema = Schema::parse_str("{} [] [] [] { Flag (bool) } []").unwrap();

    assert_eq!(
        schema.declaration_body(&name("Flag")),
        Some(&DeclarationBody::Newtype(TypeExpression::Primitive(
            Primitive::Boolean
        )))
    );
}

#[test]
fn parser_accepts_explicit_field_names_without_changing_field_types() {
    let schema = Schema::parse_str(
        "{} [] [] [] { Magnitude [Maximum Medium] Confidence ((certainty Magnitude) (priority Magnitude)) } []",
    )
    .unwrap();

    let Some(DeclarationBody::Record(fields)) = schema.declaration_body(&name("Confidence")) else {
        panic!("expected record body");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(
        fields[0].name(),
        Some(&FieldName::new("certainty").unwrap())
    );
    assert_eq!(fields[0].expression(), &named("Magnitude"));
    assert_eq!(fields[1].name(), Some(&FieldName::new("priority").unwrap()));
    assert_eq!(fields[1].expression(), &named("Magnitude"));

    let layout = Layout::for_declaration(&schema, &name("Confidence")).unwrap();
    assert_eq!(
        layout.fields()[0].name(),
        Some(&FieldName::new("certainty").unwrap())
    );
    assert_eq!(layout.root_positions(), vec![0, 1]);
}

#[test]
fn field_name_only_change_does_not_require_storage_upgrade_annotation() {
    let previous = Schema::parse_str(
        "{} [] [] [] { Magnitude [Maximum Medium] Entry (Topic Magnitude) Topic (String) } []",
    )
    .unwrap()
    .assemble(&[])
    .unwrap();
    let current = Schema::parse_str(
        "{} [] [] [] { Magnitude [Maximum Medium] Entry (Topic (certainty Magnitude)) Topic (String) } []",
    )
    .unwrap()
    .assemble(&[])
    .unwrap();

    let plan = current.plan_upgrade_from(&previous).unwrap();

    assert!(plan.projections().iter().any(|projection| matches!(
        projection,
        Projection::Identity { name } if name.as_str() == "Entry"
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
