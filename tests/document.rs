use std::collections::BTreeMap;

use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode};
use schema::{
    Declaration, DeclarationBody, Engine, Error, FieldLocation, Layout, Name, Namespace, Payload,
    Primitive, Reference, Section, TypeExpression, Variant,
};

fn name(value: &str) -> Name {
    Name::new(value).unwrap()
}

fn named(value: &str) -> TypeExpression {
    TypeExpression::named(name(value))
}

#[test]
fn validates_spirit_style_subset() {
    let document = DocumentBuilder::spirit_subset().build().unwrap();
    let record = document
        .variant(&name("Operation"), &name("Record"))
        .unwrap();

    assert_eq!(record.engine(), Some(Engine::Assert));
    assert_eq!(record.payload(), &Payload::Type(named("Entry")));
}

#[test]
fn rejects_duplicate_declaration_names() {
    let result = schema::Document::new(vec![
        Section::Messaging(vec![Declaration::local(
            name("Kind"),
            vec![Variant::unit(name("Decision"))],
        )]),
        Section::Namespace(
            Namespace::local(vec![(name("Kind"), vec![Variant::unit(name("Principle"))])]).unwrap(),
        ),
    ]);

    assert!(matches!(result, Err(Error::DuplicateDeclaration { name }) if name.as_str() == "Kind"));
}

#[test]
fn rejects_duplicate_variant_names() {
    let result = schema::Document::new(vec![Section::Namespace(
        Namespace::local(vec![(
            name("Kind"),
            vec![
                Variant::unit(name("Decision")),
                Variant::unit(name("Decision")),
            ],
        )])
        .unwrap(),
    )]);

    assert!(
        matches!(result, Err(Error::DuplicateVariant { declaration, variant })
            if declaration.as_str() == "Kind" && variant.as_str() == "Decision")
    );
}

#[test]
fn rejects_unknown_named_type() {
    let result = schema::Document::new(vec![Section::Namespace(
        Namespace::local(vec![(
            name("Entry"),
            vec![Variant::with_fields(name("Entry"), vec![named("Topic")])],
        )])
        .unwrap(),
    )]);

    assert!(matches!(result, Err(Error::UnknownType { name }) if name.as_str() == "Topic"));
}

#[test]
fn layout_places_fixed_fields_in_root_and_growing_fields_in_boxes() {
    let document = DocumentBuilder::spirit_subset().build().unwrap();
    let layout = Layout::for_variant(&document, &name("Entry"), &name("Entry")).unwrap();

    assert_eq!(layout.root_positions(), vec![1, 4]);
    assert_eq!(layout.box_positions(), vec![0, 2, 3, 5]);
    assert_eq!(layout.fields()[0].location(), FieldLocation::Box);
    assert_eq!(layout.fields()[1].location(), FieldLocation::Root);
}

#[test]
fn cross_schema_references_validate_but_layout_is_conservative() {
    let document = schema::Document::new(vec![Section::Namespace(
        Namespace::new(vec![
            (
                name("Magnitude"),
                DeclarationBody::Reference(Reference::Path(
                    "signal-sema/magnitude.schema.nota".into(),
                )),
            ),
            (
                name("Entry"),
                DeclarationBody::Local {
                    variants: vec![Variant::with_fields(
                        name("Entry"),
                        vec![named("Magnitude")],
                    )],
                },
            ),
        ])
        .unwrap(),
    )])
    .unwrap();

    let layout = Layout::for_variant(&document, &name("Entry"), &name("Entry")).unwrap();
    assert_eq!(layout.box_positions(), vec![0]);
}

#[test]
fn nota_curly_map_is_usable_for_schema_namespace_names() {
    let mut decoder = Decoder::new("{Entry 1 Operation 2}");
    let decoded = BTreeMap::<Name, u64>::decode(&mut decoder).unwrap();

    assert_eq!(decoded.get(&name("Entry")), Some(&1));
    assert_eq!(decoded.get(&name("Operation")), Some(&2));

    let mut encoder = Encoder::new();
    decoded.encode(&mut encoder).unwrap();
    assert_eq!(encoder.into_string(), "{Entry 1 Operation 2}");
}

struct DocumentBuilder {
    messaging: Vec<Declaration>,
    namespace: Namespace,
}

impl DocumentBuilder {
    fn spirit_subset() -> Self {
        Self {
            messaging: vec![Declaration::local(
                name("Operation"),
                vec![
                    Variant::with_type(name("State"), named("Statement"))
                        .with_engine(Engine::Assert),
                    Variant::with_type(name("Record"), named("Entry")).with_engine(Engine::Assert),
                ],
            )],
            namespace: Namespace::local(vec![
                (
                    name("Kind"),
                    vec![
                        Variant::unit(name("Decision")),
                        Variant::unit(name("Principle")),
                        Variant::unit(name("Correction")),
                    ],
                ),
                (
                    name("Magnitude"),
                    vec![
                        Variant::unit(name("Minimum")),
                        Variant::unit(name("Medium")),
                        Variant::unit(name("Maximum")),
                    ],
                ),
                (
                    name("Topic"),
                    vec![Variant::with_type(
                        name("Topic"),
                        TypeExpression::Primitive(Primitive::String),
                    )],
                ),
                (
                    name("Summary"),
                    vec![Variant::with_type(
                        name("Summary"),
                        TypeExpression::Primitive(Primitive::String),
                    )],
                ),
                (
                    name("Context"),
                    vec![Variant::with_type(
                        name("Context"),
                        TypeExpression::Primitive(Primitive::String),
                    )],
                ),
                (
                    name("Quote"),
                    vec![Variant::with_type(
                        name("Quote"),
                        TypeExpression::Primitive(Primitive::String),
                    )],
                ),
                (
                    name("Entry"),
                    vec![Variant::with_fields(
                        name("Entry"),
                        vec![
                            named("Topic"),
                            named("Kind"),
                            named("Summary"),
                            named("Context"),
                            named("Magnitude"),
                            named("Quote"),
                        ],
                    )],
                ),
                (
                    name("Statement"),
                    vec![Variant::with_type(
                        name("Statement"),
                        TypeExpression::Primitive(Primitive::String),
                    )],
                ),
            ])
            .unwrap(),
        }
    }

    fn build(self) -> schema::Result<schema::Document> {
        schema::Document::new(vec![
            Section::Messaging(self.messaging),
            Section::Namespace(self.namespace),
        ])
    }
}
