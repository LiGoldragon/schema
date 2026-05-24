use std::collections::BTreeMap;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode};
use schema::{
    AssembledType, Declaration, Engine, Error, Feature, FieldLocation, Header, HeaderRoot,
    ImportDirective, ImportResolution, Imports, Layout, Leg, Name, Namespace, Payload, Primitive,
    Projection, RouteBody, Schema, SchemaPath, StandardProjection, TypeExpression, Upgrade,
    UpgradeAnnotation, Variant, Version,
};

fn magnitude_resolution() -> ImportResolution {
    ImportResolution::new(name("Magnitude"), vec![name("Magnitude")]).unwrap()
}

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

#[test]
fn schema_carries_component_name_and_renders_namespaced_uid() {
    let schema = Builder::spirit_mvp(Vec::new())
        .with_component_name(name("Spirit"))
        .build()
        .unwrap();

    assert_eq!(schema.component_name().as_str(), "Spirit");

    let assembled = schema.assemble(&[magnitude_resolution()]).unwrap();

    assert_eq!(assembled.component().as_str(), "Spirit");

    let entry_uid = assembled.uid_for(&name("Entry"));
    assert_eq!(entry_uid.component().as_str(), "Spirit");
    assert_eq!(entry_uid.type_name().as_str(), "Entry");
    assert_eq!(entry_uid.to_string(), "Spirit::namespace::Entry");

    // Imported types render with the same component anchor; the imported
    // type's *home* schema would render `Sema::namespace::Magnitude`, but
    // this Schema's UID surface qualifies under its own component per intent
    // 469 (each schema names the types it surfaces).
    let magnitude_uid = assembled.uid_for(&name("Magnitude"));
    assert_eq!(magnitude_uid.to_string(), "Spirit::namespace::Magnitude");

    // Anonymous-component path falls back to the literal "Anonymous" name
    // when no component is supplied.
    let anonymous = Builder::spirit_mvp(Vec::new()).build().unwrap();
    assert_eq!(anonymous.component_name().as_str(), "Anonymous");
    let anonymous_assembled = anonymous.assemble(&[magnitude_resolution()]).unwrap();
    assert_eq!(
        anonymous_assembled.uid_for(&name("Entry")).to_string(),
        "Anonymous::namespace::Entry"
    );
}

#[test]
fn layout_for_assembled_places_imported_fixed_width_magnitude_in_root() {
    // The bug audited in /171 §4.3 + §5: `Layout::for_declaration(document,
    // _)` cannot see that imported Magnitude is fixed-width, so it lands
    // Entry position 4 (Magnitude) in `box_positions`. This test exercises
    // the fix: assemble first, supply a fixed-width hint for the imported
    // Magnitude, and `Layout::for_assembled` lands position 4 in
    // `root_positions`.
    let schema = Builder::spirit_mvp(Vec::new())
        .with_component_name(name("Spirit"))
        .build()
        .unwrap();
    let mut import_widths = BTreeMap::new();
    import_widths.insert(name("Magnitude"), true);
    let assembled = schema
        .assemble(&[magnitude_resolution()])
        .unwrap()
        .with_import_widths(import_widths);

    // Sanity check the AssembledSchema knows Magnitude as an imported type
    // (not a local body).
    assert!(matches!(
        assembled.assembled_type(&name("Magnitude")),
        Some(AssembledType::Imported { .. })
    ));

    let layout = Layout::for_assembled(&assembled, &name("Entry")).unwrap();

    assert_eq!(
        layout.root_positions(),
        vec![1, 4],
        "Kind (position 1) and Magnitude (position 4, imported, fixed-width hint) belong in root"
    );
    assert_eq!(
        layout.box_positions(),
        vec![0, 2, 3, 5],
        "Topic (0), Summary (2), Context (3), Quote (5) are all variable-width newtype(String) — boxed"
    );

    // Each field's classification is individually inspectable; check Magnitude.
    let magnitude_field = layout
        .fields()
        .iter()
        .find(|field| field.position() == 4)
        .expect("Entry has a position-4 field");
    assert_eq!(magnitude_field.location(), FieldLocation::Root);
}

#[test]
fn layout_for_declaration_remains_conservative_for_imported_magnitude() {
    // The legacy pre-assembly path: Layout::for_declaration cannot see
    // imported Magnitude's width, so it falls back to variable-width (box).
    // This preserves the prior behaviour while the new for_assembled path
    // gets the fixed-width-in-root result.
    let schema = Builder::spirit_mvp(Vec::new()).build().unwrap();
    let layout = Layout::for_declaration(&schema, &name("Entry")).unwrap();

    assert_eq!(layout.root_positions(), vec![1]);
    assert_eq!(layout.box_positions(), vec![0, 2, 3, 4, 5]);
}

#[test]
fn layout_for_assembled_without_import_hint_falls_back_to_box() {
    // Without an import-width hint, an imported Name conservatively lands in
    // a box. This is the safe default per /171 §5.
    let schema = Builder::spirit_mvp(Vec::new())
        .with_component_name(name("Spirit"))
        .build()
        .unwrap();
    let assembled = schema.assemble(&[magnitude_resolution()]).unwrap();

    let layout = Layout::for_assembled(&assembled, &name("Entry")).unwrap();

    assert_eq!(layout.root_positions(), vec![1]);
    assert_eq!(layout.box_positions(), vec![0, 2, 3, 4, 5]);
}

struct Builder {
    features: Vec<Feature>,
    kind_variants: Vec<&'static str>,
    entry_extra_field: Option<TypeExpression>,
    component_name: Option<Name>,
}

impl Builder {
    fn spirit_mvp(features: Vec<Feature>) -> Self {
        Self {
            features,
            kind_variants: vec!["Decision", "Principle", "Correction"],
            entry_extra_field: None,
            component_name: None,
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

    fn with_component_name(mut self, component: Name) -> Self {
        self.component_name = Some(component);
        self
    }

    fn build(self) -> schema::Result<Schema> {
        let imports = Imports::new(vec![(
            name("Magnitude"),
            ImportDirective::import_all(SchemaPath::new("../signal-sema/magnitude.schema")),
        )])?;
        let ordinary = Header::new(vec![
            HeaderRoot::new(name("State"), vec![name("Statement")])?,
            HeaderRoot::new(name("Record"), vec![name("Entry")])?,
        ])?;
        let namespace = Namespace::declarations(self.declarations())?;

        match self.component_name {
            Some(component) => Schema::for_component(
                component,
                imports,
                ordinary,
                Header::empty(),
                Header::empty(),
                namespace,
                self.features,
            ),
            None => Schema::new(
                imports,
                ordinary,
                Header::empty(),
                Header::empty(),
                namespace,
                self.features,
            ),
        }
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
