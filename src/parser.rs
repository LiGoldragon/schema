use nota_codec::{Decoder, Token};

use crate::{
    Declaration, DeclarationBody, Error, EventFeature, Feature, Field, Header, HeaderRoot,
    ImportDirective, Imports, Name, Namespace, ObservableFeature, Primitive, Result, Schema,
    SchemaPath, TypeExpression, Upgrade, UpgradeAnnotation, Variant, Version,
};

impl Schema {
    #[doc(hidden)]
    pub fn parse_str_with_streaming_decoder(input: &str) -> Result<Self> {
        Parser::new(input).parse_schema()
    }
}

struct Parser<'input> {
    decoder: Decoder<'input>,
}

impl<'input> Parser<'input> {
    fn new(input: &'input str) -> Self {
        Self {
            decoder: Decoder::new(input),
        }
    }

    fn parse_schema(mut self) -> Result<Schema> {
        let schema = Schema::new(
            self.parse_imports()?,
            self.parse_header("ordinary header")?,
            self.parse_header("owner header")?,
            self.parse_header("sema header")?,
            self.parse_namespace()?,
            self.parse_features()?,
        )?;
        if let Some(token) = self.peek_token("end of schema")? {
            return Err(Error::InvalidSchemaText {
                context: "schema",
                message: format!("unexpected trailing token {token:?}"),
            });
        }
        Ok(schema)
    }

    fn parse_imports(&mut self) -> Result<Imports> {
        self.expect_map_start("imports")?;
        let mut entries = Vec::new();
        while !self.peek_is_map_end("imports")? {
            let binding = self.read_map_name("import binding")?;
            let directive = self.parse_import_directive()?;
            entries.push((binding, directive));
        }
        self.expect_map_end("imports")?;
        Imports::new(entries)
    }

    fn parse_import_directive(&mut self) -> Result<ImportDirective> {
        self.expect_record_start("import directive")?;
        let variant = self.read_name_text("import directive variant")?;
        let path = SchemaPath::new(self.read_path("import path")?);
        let directive = match variant.as_str() {
            "Import" => {
                let names = self.parse_name_vector("import selection")?;
                ImportDirective::import(path, names)
            }
            "ImportAll" => ImportDirective::import_all(path),
            _ => {
                return Err(Error::InvalidSchemaText {
                    context: "import directive",
                    message: format!("unknown import directive `{variant}`"),
                });
            }
        };
        self.expect_record_end("import directive")?;
        Ok(directive)
    }

    fn parse_header(&mut self, context: &'static str) -> Result<Header> {
        self.expect_seq_start(context)?;
        let mut roots = Vec::new();
        while !self.peek_is_seq_end(context)? {
            self.expect_record_start(context)?;
            let root = self.read_name(context)?;
            let endpoints = self.parse_name_record(context)?;
            self.expect_record_end(context)?;
            roots.push(HeaderRoot::new(root, endpoints)?);
        }
        self.expect_seq_end(context)?;
        Header::new(roots)
    }

    fn parse_namespace(&mut self) -> Result<Namespace> {
        self.expect_map_start("namespace")?;
        let mut declarations = Vec::new();
        while !self.peek_is_map_end("namespace")? {
            let name = self.read_map_name("namespace name")?;
            let body = self.parse_declaration_body()?;
            declarations.push(Declaration::new(name, body));
        }
        self.expect_map_end("namespace")?;
        Namespace::declarations(declarations)
    }

    fn parse_declaration_body(&mut self) -> Result<DeclarationBody> {
        match self.peek_token("declaration body")? {
            Some(Token::LBracket) => self.parse_record_body(),
            Some(Token::LParen) => self.parse_enum_body(),
            Some(_) => self.parse_type_expression().map(DeclarationBody::Alias),
            None => Err(Error::InvalidSchemaText {
                context: "declaration body",
                message: "unexpected end of input".into(),
            }),
        }
    }

    fn parse_enum_body(&mut self) -> Result<DeclarationBody> {
        self.expect_record_start("enum declaration")?;
        let mut variants = Vec::new();
        while !self.peek_is_record_end("enum declaration")? {
            variants.push(self.parse_variant()?);
        }
        self.expect_record_end("enum declaration")?;
        Ok(DeclarationBody::Enum { variants })
    }

    fn parse_variant(&mut self) -> Result<Variant> {
        if matches!(self.peek_token("variant")?, Some(Token::LParen)) {
            self.expect_record_start("data-carrying variant")?;
            let name = self.read_name("variant name")?;
            if self.peek_is_record_end("data-carrying variant")? {
                self.expect_record_end("data-carrying variant")?;
                return Ok(Variant::with_type(
                    name.clone(),
                    TypeExpression::named(name),
                ));
            }
            let mut fields = Vec::new();
            while !self.peek_is_record_end("data-carrying variant")? {
                fields.push(self.parse_field()?);
            }
            self.expect_record_end("data-carrying variant")?;
            reject_repeated_self_payload(&name, &fields, "data-carrying variant")?;
            Ok(variant_with_fields(name, fields))
        } else {
            self.read_name("unit variant").map(Variant::unit)
        }
    }

    fn parse_record_body(&mut self) -> Result<DeclarationBody> {
        self.expect_seq_start("record declaration")?;
        let mut fields = Vec::new();
        while !self.peek_is_seq_end("record declaration")? {
            fields.push(self.parse_field()?);
        }
        self.expect_seq_end("record declaration")?;

        match fields.len() {
            0 => Err(Error::InvalidSchemaText {
                context: "record declaration",
                message: "declaration record must carry at least one type expression".into(),
            }),
            1 if fields[0].name().is_none() => Ok(DeclarationBody::Newtype(
                fields.remove(0).expression().clone(),
            )),
            _ => Ok(DeclarationBody::Record(fields)),
        }
    }

    fn parse_field(&mut self) -> Result<Field> {
        if matches!(self.peek_token("field")?, Some(Token::LParen)) {
            return self.parse_container_field();
        }
        self.parse_type_expression().map(Field::inferred)
    }

    fn parse_type_expression(&mut self) -> Result<TypeExpression> {
        if matches!(self.peek_token("type expression")?, Some(Token::LParen)) {
            return self.parse_container_expression();
        }

        let text = match self.peek_token("type expression")? {
            Some(Token::Ident(name)) if starts_with_uppercase(&name) => {
                self.read_name_text("type expression")?
            }
            Some(Token::Ident(_)) => self.read_string("type expression")?,
            Some(token) => {
                return Err(Error::InvalidSchemaText {
                    context: "type expression",
                    message: format!("expected type name, got {token:?}"),
                });
            }
            None => {
                return Err(Error::InvalidSchemaText {
                    context: "type expression",
                    message: "unexpected end of input".into(),
                });
            }
        };

        if let Some(primitive) = primitive(&text) {
            Ok(primitive)
        } else {
            Name::new(text).map(TypeExpression::named)
        }
    }

    fn parse_container_expression(&mut self) -> Result<TypeExpression> {
        self.expect_record_start("container expression")?;
        let head = self.read_name_text("container expression")?;
        let expression = self.parse_container_expression_after_head(&head)?;
        self.expect_record_end("container expression")?;
        Ok(expression)
    }

    fn parse_container_field(&mut self) -> Result<Field> {
        self.expect_record_start("field")?;
        let head = self.read_field_or_container_head()?;
        if is_container_head(&head) {
            let expression = self.parse_container_expression_after_head(&head)?;
            self.expect_record_end("field")?;
            return Ok(Field::inferred(expression));
        }
        if self.peek_is_record_end("field")? {
            self.expect_record_end("field")?;
            return Ok(Field::inferred(type_expression_from_text(&head)?));
        }

        Err(Error::InvalidSchemaText {
            context: "field",
            message: format!(
                "field names are inferred from type names; `{head}` is not a container type expression"
            ),
        })
    }

    fn parse_container_expression_after_head(&mut self, head: &str) -> Result<TypeExpression> {
        let expression = match head {
            "Option" => {
                let inner = self.parse_type_expression()?;
                TypeExpression::optional(inner)
            }
            "Vec" => {
                let inner = self.parse_type_expression()?;
                TypeExpression::vector(inner)
            }
            "Map" => {
                let key = self.parse_type_expression()?;
                let value = self.parse_type_expression()?;
                TypeExpression::map(key, value)
            }
            _ => {
                return Err(Error::InvalidSchemaText {
                    context: "container expression",
                    message: format!("unknown container `{head}`"),
                });
            }
        };
        Ok(expression)
    }

    fn read_field_or_container_head(&mut self) -> Result<String> {
        match self.peek_token("field head")? {
            Some(Token::Ident(name)) if starts_with_uppercase(&name) => {
                self.read_name_text("field head")
            }
            Some(_) => self.read_string("field head"),
            None => Err(Error::InvalidSchemaText {
                context: "field head",
                message: "unexpected end of input".into(),
            }),
        }
    }

    fn parse_features(&mut self) -> Result<Vec<Feature>> {
        self.expect_seq_start("features")?;
        let mut features = Vec::new();
        while !self.peek_is_seq_end("features")? {
            features.push(self.parse_feature()?);
        }
        self.expect_seq_end("features")?;
        Ok(features)
    }

    fn parse_feature(&mut self) -> Result<Feature> {
        self.expect_record_start("feature")?;
        let head = self.read_name_text("feature")?;
        let feature = match head.as_str() {
            "Reply" => Feature::Reply(self.parse_names_until_record_end("reply feature")?),
            "Event" => self.parse_event_feature()?,
            "Observable" => self.parse_observable_feature()?,
            "Upgrade" => self.parse_upgrade_feature()?,
            _ => {
                return Err(Error::InvalidSchemaText {
                    context: "feature",
                    message: format!("unknown feature `{head}`"),
                });
            }
        };
        self.expect_record_end("feature")?;
        Ok(feature)
    }

    fn parse_event_feature(&mut self) -> Result<Feature> {
        let stream = if matches!(self.peek_token("event feature")?, Some(Token::LParen)) {
            Some(self.parse_belongs_record()?)
        } else {
            None
        };
        let events = self.parse_names_until_record_end("event feature")?;
        Ok(Feature::Event(EventFeature::new(stream, events)))
    }

    fn parse_belongs_record(&mut self) -> Result<Name> {
        self.expect_record_start("event belongs")?;
        let relation = self.read_string("event belongs relation")?;
        if relation != "belongs" {
            return Err(Error::InvalidSchemaText {
                context: "event belongs",
                message: format!("expected `belongs`, got `{relation}`"),
            });
        }
        let stream = self.read_name("event stream")?;
        self.expect_record_end("event belongs")?;
        Ok(stream)
    }

    fn parse_observable_feature(&mut self) -> Result<Feature> {
        let mut filter = None;
        let mut operation_event = None;
        let mut effect_event = None;

        while !self.peek_is_record_end("observable feature")? {
            self.expect_record_start("observable field")?;
            let field = self.read_string("observable field name")?;
            match field.as_str() {
                "filter" => filter = Some(self.read_string("observable filter")?),
                "operation_event" => operation_event = Some(self.read_name("operation event")?),
                "effect_event" => effect_event = Some(self.read_name("effect event")?),
                _ => {
                    return Err(Error::InvalidSchemaText {
                        context: "observable feature",
                        message: format!("unknown observable field `{field}`"),
                    });
                }
            }
            self.expect_record_end("observable field")?;
        }

        Ok(Feature::Observable(ObservableFeature::new(
            filter,
            operation_event,
            effect_event,
        )))
    }

    fn parse_upgrade_feature(&mut self) -> Result<Feature> {
        let from_version = self.parse_from_version()?;
        let mut annotations = Vec::new();
        while !self.peek_is_record_end("upgrade feature")? {
            annotations.push(self.parse_upgrade_annotation()?);
        }
        Ok(Feature::Upgrade(Upgrade::new(from_version, annotations)))
    }

    fn parse_from_version(&mut self) -> Result<Version> {
        self.expect_record_start("upgrade version")?;
        let head = self.read_name_text("upgrade version")?;
        if head != "FromVersion" {
            return Err(Error::InvalidSchemaText {
                context: "upgrade version",
                message: format!("expected `FromVersion`, got `{head}`"),
            });
        }
        let version = Version::new(self.read_path("upgrade source version")?);
        self.expect_record_end("upgrade version")?;
        Ok(version)
    }

    fn parse_upgrade_annotation(&mut self) -> Result<UpgradeAnnotation> {
        self.expect_record_start("upgrade annotation")?;
        let head = self.read_name_text("upgrade annotation")?;
        let annotation = match head.as_str() {
            "Migrate" => UpgradeAnnotation::Migrate(self.read_name("migrated type")?),
            "RenamedFrom" => UpgradeAnnotation::RenamedFrom {
                current: self.read_name("current type")?,
                previous: self.read_name("previous type")?,
            },
            "Drop" => UpgradeAnnotation::Drop(self.read_name("dropped type")?),
            "Custom" => UpgradeAnnotation::Custom {
                name: self.read_name("custom migrated type")?,
                implementation: self.read_name("custom migration implementation")?,
            },
            "Untranslatable" => {
                UpgradeAnnotation::Untranslatable(self.read_name("untranslatable type")?)
            }
            _ => {
                return Err(Error::InvalidSchemaText {
                    context: "upgrade annotation",
                    message: format!("unknown upgrade annotation `{head}`"),
                });
            }
        };
        self.expect_record_end("upgrade annotation")?;
        Ok(annotation)
    }

    fn parse_name_vector(&mut self, context: &'static str) -> Result<Vec<Name>> {
        self.expect_seq_start(context)?;
        let mut names = Vec::new();
        while !self.peek_is_seq_end(context)? {
            names.push(self.read_name(context)?);
        }
        self.expect_seq_end(context)?;
        Ok(names)
    }

    fn parse_name_record(&mut self, context: &'static str) -> Result<Vec<Name>> {
        self.expect_record_start(context)?;
        let mut names = Vec::new();
        while !self.peek_is_record_end(context)? {
            names.push(self.read_name(context)?);
        }
        self.expect_record_end(context)?;
        Ok(names)
    }

    fn parse_names_until_record_end(&mut self, context: &'static str) -> Result<Vec<Name>> {
        let mut names = Vec::new();
        while !self.peek_is_record_end(context)? {
            names.push(self.read_name(context)?);
        }
        Ok(names)
    }

    fn read_map_name(&mut self, context: &'static str) -> Result<Name> {
        Name::new(self.read_map_key(context)?)
    }

    fn read_name(&mut self, context: &'static str) -> Result<Name> {
        Name::new(self.read_name_text(context)?)
    }

    fn read_name_text(&mut self, context: &'static str) -> Result<String> {
        self.decoder
            .read_pascal_identifier()
            .map_err(|error| nota_error(context, error))
    }

    fn read_string(&mut self, context: &'static str) -> Result<String> {
        self.decoder
            .read_string()
            .map_err(|error| nota_error(context, error))
    }

    fn read_path(&mut self, context: &'static str) -> Result<String> {
        self.decoder
            .read_path()
            .map_err(|error| nota_error(context, error))
    }

    fn read_map_key(&mut self, context: &'static str) -> Result<String> {
        self.decoder
            .read_map_key()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_record_start(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_record_start()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_record_end(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_record_end()
            .map_err(|error| nota_error(context, error))
    }

    fn peek_is_record_end(&mut self, context: &'static str) -> Result<bool> {
        self.decoder
            .peek_is_record_end()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_seq_start(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_seq_start()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_seq_end(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_seq_end()
            .map_err(|error| nota_error(context, error))
    }

    fn peek_is_seq_end(&mut self, context: &'static str) -> Result<bool> {
        self.decoder
            .peek_is_seq_end()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_map_start(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_map_start()
            .map_err(|error| nota_error(context, error))
    }

    fn expect_map_end(&mut self, context: &'static str) -> Result<()> {
        self.decoder
            .expect_map_end()
            .map_err(|error| nota_error(context, error))
    }

    fn peek_is_map_end(&mut self, context: &'static str) -> Result<bool> {
        self.decoder
            .peek_is_map_end()
            .map_err(|error| nota_error(context, error))
    }

    fn peek_token(&mut self, context: &'static str) -> Result<Option<Token>> {
        self.decoder
            .peek_token()
            .map_err(|error| nota_error(context, error))
    }
}

fn type_expression_from_text(text: &str) -> Result<TypeExpression> {
    if let Some(primitive) = primitive(text) {
        Ok(primitive)
    } else {
        Name::new(text).map(TypeExpression::named)
    }
}

fn variant_with_fields(name: Name, fields: Vec<Field>) -> Variant {
    match fields.len() {
        0 => Variant::unit(name),
        1 if fields[0].name().is_none() => Variant::with_type(
            name,
            fields.into_iter().next().unwrap().expression().clone(),
        ),
        _ => Variant::with_field_entries(name, fields),
    }
}

fn reject_repeated_self_payload(
    name: &Name,
    fields: &[Field],
    context: &'static str,
) -> Result<()> {
    if let [field] = fields
        && field.name().is_none()
        && matches!(field.expression(), TypeExpression::Named(payload) if payload == name)
    {
        return Err(Error::InvalidSchemaText {
            context,
            message: format!(
                "self-named variant payload `{name}` uses `({name})`, not `({name} {name})`"
            ),
        });
    }
    Ok(())
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

fn is_container_head(text: &str) -> bool {
    matches!(text, "Option" | "Vec" | "Map")
}

fn starts_with_uppercase(text: &str) -> bool {
    text.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_uppercase())
}

fn nota_error(context: &'static str, error: nota_codec::Error) -> Error {
    Error::InvalidSchemaText {
        context,
        message: error.to_string(),
    }
}
