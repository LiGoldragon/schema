use std::{fmt::Write, path::Path};

use schema_next::{
    Asschema, EnumDeclaration, EnumVariant, ImportResolver, MacroContext, Name, SchemaEngine,
    SchemaIdentity, TypeDeclaration, TypeReference,
};

#[test]
fn big_spirit_example_lowers_to_checked_asschema_output() {
    assert_big_fixture(
        "spirit-reactive-large",
        include_str!("fixtures/big-schemas/spirit-reactive-large.schema"),
        None,
    );
}

#[test]
fn big_triad_example_lowers_to_checked_asschema_output() {
    assert_big_fixture(
        "triad-reactive-large",
        include_str!("fixtures/big-schemas/triad-reactive-large.schema"),
        None,
    );
}

#[test]
fn big_imported_consumer_example_resolves_cross_crate_imports() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("marker-core")
        .join("schema");
    let resolver = ImportResolver::new().with_dependency("marker-core", schema_dir, "0.1.0");
    assert_big_fixture(
        "imported-mail-consumer",
        include_str!("fixtures/big-schemas/imported-mail-consumer.schema"),
        Some(resolver),
    );
}

fn assert_big_fixture(name: &str, source: &str, resolver: Option<ImportResolver>) {
    let engine = SchemaEngine::default();
    let mut context = MacroContext::default();
    let identity = SchemaIdentity::new(format!("example:{name}"), "0.1.0");
    let asschema = match resolver {
        Some(resolver) => engine
            .lower_source_with_resolver(source, identity, &mut context, &resolver)
            .expect("big schema lowers with imports"),
        None => engine
            .lower_source_with_context(source, identity, &mut context)
            .expect("big schema lowers"),
    };
    let rendered = AsschemaWitness::new(name, &asschema, &context).render();
    let expected_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/big-schemas/{name}.witness.txt"));
    if std::env::var_os("SCHEMA_NEXT_UPDATE_BIG_EXAMPLES").is_some() {
        std::fs::write(&expected_path, &rendered).expect("write expected asschema");
    }
    let expected = std::fs::read_to_string(&expected_path).expect("read expected asschema");
    assert_eq!(
        rendered, expected,
        "assembled schema witness drifted for {name}"
    );
}

struct AsschemaWitness<'schema> {
    name: &'schema str,
    asschema: &'schema Asschema,
    context: &'schema MacroContext,
}

impl<'schema> AsschemaWitness<'schema> {
    fn new(
        name: &'schema str,
        asschema: &'schema Asschema,
        context: &'schema MacroContext,
    ) -> Self {
        Self {
            name,
            asschema,
            context,
        }
    }

    fn render(&self) -> String {
        let mut output = String::new();
        writeln!(output, "fixture {}", self.name).expect("write string");
        writeln!(
            output,
            "identity {} {}",
            self.asschema.identity().component().as_str(),
            self.asschema.identity().version()
        )
        .expect("write string");
        self.render_imports(&mut output);
        self.render_macro_trace(&mut output);
        self.render_enum(&mut output, "input", self.asschema.input());
        self.render_enum(&mut output, "output", self.asschema.output());
        writeln!(output, "namespace").expect("write string");
        for declaration in self.asschema.namespace() {
            self.render_declaration(&mut output, declaration);
        }
        output
    }

    fn render_imports(&self, output: &mut String) {
        writeln!(output, "imports").expect("write string");
        if self.asschema.imports().is_empty() {
            writeln!(output, "  none").expect("write string");
        }
        for import in self.asschema.imports() {
            writeln!(
                output,
                "  {} = {}",
                import.local_name.as_str(),
                self.render_reference(&import.source)
            )
            .expect("write string");
        }
        writeln!(output, "resolved_imports").expect("write string");
        if self.asschema.resolved_imports().is_empty() {
            writeln!(output, "  none").expect("write string");
        }
        for import in self.asschema.resolved_imports() {
            writeln!(
                output,
                "  {} = {}",
                import.local_name().as_str(),
                import.source().rust_path()
            )
            .expect("write string");
        }
    }

    fn render_macro_trace(&self, output: &mut String) {
        writeln!(output, "macro_trace").expect("write string");
        writeln!(
            output,
            "  applied {}",
            self.context.macros_applied().join(" -> ")
        )
        .expect("write string");
        let positions = self
            .context
            .positions_seen()
            .iter()
            .map(|position| position.as_str())
            .collect::<Vec<_>>()
            .join(" -> ");
        writeln!(output, "  positions {positions}").expect("write string");
    }

    fn render_enum(&self, output: &mut String, label: &str, declaration: &EnumDeclaration) {
        writeln!(output, "{label} {}", declaration.name.as_str()).expect("write string");
        for variant in &declaration.variants {
            writeln!(output, "  {}", self.render_variant(variant)).expect("write string");
        }
    }

    fn render_declaration(&self, output: &mut String, declaration: &TypeDeclaration) {
        match declaration {
            TypeDeclaration::Struct(declaration) => {
                writeln!(output, "  struct {}", declaration.name.as_str()).expect("write string");
                for field in &declaration.fields {
                    writeln!(
                        output,
                        "    {}: {}",
                        field.name.as_str(),
                        self.render_reference(&field.reference)
                    )
                    .expect("write string");
                }
            }
            TypeDeclaration::Newtype(declaration) => {
                let field = declaration.fields.first().expect("newtype field");
                writeln!(
                    output,
                    "  newtype {} = {}",
                    declaration.name.as_str(),
                    self.render_reference(&field.reference)
                )
                .expect("write string");
            }
            TypeDeclaration::Enum(declaration) => {
                writeln!(output, "  enum {}", declaration.name.as_str()).expect("write string");
                for variant in &declaration.variants {
                    writeln!(output, "    {}", self.render_variant(variant)).expect("write string");
                }
            }
        }
    }

    fn render_variant(&self, variant: &EnumVariant) -> String {
        match &variant.payload {
            Some(payload) => format!(
                "{}({})",
                variant.name.as_str(),
                self.render_reference(payload)
            ),
            None => variant.name.as_str().to_owned(),
        }
    }

    fn render_reference(&self, reference: &TypeReference) -> String {
        match reference {
            TypeReference::Plain(name) => self.render_name(name),
            TypeReference::Vector(inner) => format!("Vec<{}>", self.render_reference(inner)),
            TypeReference::Map(key, value) => format!(
                "KeyValue<{}, {}>",
                self.render_reference(key),
                self.render_reference(value)
            ),
            TypeReference::Optional(inner) => format!("Option<{}>", self.render_reference(inner)),
        }
    }

    fn render_name(&self, name: &Name) -> String {
        name.as_str().to_owned()
    }
}
