use nota_next::{Document, NotaDecode, NotaEncode};
use schema_next::{Name, SchemaEngine, SchemaIdentity, SymbolPath};

struct SymbolPathFixture {
    identity: SchemaIdentity,
    source: &'static str,
}

impl SymbolPathFixture {
    fn new() -> Self {
        Self {
            identity: SchemaIdentity::new("spirit-next:lib", "0.1.0"),
            source: "[(Record Entry)] [(Rejected SignalRejection)] { Description String Entry { Description * Kind * } SignalRejection { validationError ValidationError } Kind [Decision] ValidationError [EmptyTopic EmptyDescription] }",
        }
    }

    fn asschema(&self) -> schema_next::Asschema {
        SchemaEngine::default()
            .lower_source(self.source, self.identity.clone())
            .expect("schema lowers")
    }
}

#[test]
fn asschema_derives_canonical_symbol_paths_from_schema_positions() {
    let fixture = SymbolPathFixture::new();
    let asschema = fixture.asschema();

    assert_eq!(
        asschema
            .root_variant_path("Input", "Record")
            .expect("input record variant path"),
        SymbolPath::new([
            Name::new("spirit-next:lib"),
            Name::new("Input"),
            Name::new("Record")
        ])
    );
    assert_eq!(
        asschema.type_path("Entry").expect("entry type path"),
        SymbolPath::new([Name::new("spirit-next:lib"), Name::new("Entry")])
    );
    assert_eq!(
        asschema
            .field_path("Entry", "description")
            .expect("entry description field path"),
        SymbolPath::new([
            Name::new("spirit-next:lib"),
            Name::new("Entry"),
            Name::new("description")
        ])
    );
    assert_eq!(
        asschema
            .enum_variant_path("ValidationError", "EmptyTopic")
            .expect("validation error variant path"),
        SymbolPath::new([
            Name::new("spirit-next:lib"),
            Name::new("ValidationError"),
            Name::new("EmptyTopic")
        ])
    );
}

#[test]
fn symbol_path_round_trips_through_nota_and_rkyv_as_names_not_free_text() {
    let path = SymbolPath::new([
        Name::new("spirit-next:lib"),
        Name::new("Input"),
        Name::new("Record"),
    ]);

    let nota = path.to_nota();
    assert_eq!(nota, "(SymbolPath [spirit-next:lib Input Record])");
    let document = Document::parse(&nota).expect("symbol path nota parses");
    let decoded =
        SymbolPath::from_nota_block(&document.root_objects()[0]).expect("symbol path decodes");
    assert_eq!(decoded, path);
    assert_eq!(decoded.to_string(), "spirit-next:lib/Input/Record");

    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&path).expect("symbol path archives as rkyv");
    let restored = rkyv::from_bytes::<SymbolPath, rkyv::rancor::Error>(&bytes)
        .expect("symbol path decodes from rkyv");
    assert_eq!(restored, path);
}

#[test]
fn symbol_path_rejects_opaque_string_shapes() {
    let document = Document::parse("(SymbolPath spirit-next:lib/Input/Record)")
        .expect("opaque path shape still parses as nota");
    let _error = SymbolPath::from_nota_block(&document.root_objects()[0])
        .expect_err("symbol path body must be a vector of names");
}
