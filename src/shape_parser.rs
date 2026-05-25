use nota_codec::{NotaValue, NotaValueKind};

use crate::{
    Declaration, DeclarationBody, Error, EventFeature, Feature, Field, Header, HeaderRoot,
    ImportDirective, Imports, Name, Namespace, ObservableFeature, Primitive, Result, Schema,
    SchemaPath, TypeExpression, Upgrade, UpgradeAnnotation, Variant, Version,
};

impl Schema {
    pub fn parse_str(input: &str) -> Result<Self> {
        ShapeParser::new(input)?.parse_schema()
    }
}

struct ShapeParser {
    values: Vec<NotaValue>,
}

impl ShapeParser {
    fn new(input: &str) -> Result<Self> {
        let values =
            nota_codec::parse_sequence(input).map_err(|error| nota_error("schema", error))?;
        Ok(Self { values })
    }

    fn parse_schema(self) -> Result<Schema> {
        if self.values.len() != 6 {
            return Err(Error::InvalidSchemaText {
                context: "schema",
                message: format!("expected 6 top-level values, got {}", self.values.len()),
            });
        }

        Schema::new(
            self.parse_imports(&self.values[0])?,
            self.parse_header(&self.values[1], "ordinary header")?,
            self.parse_header(&self.values[2], "owner header")?,
            self.parse_header(&self.values[3], "sema header")?,
            self.parse_namespace(&self.values[4])?,
            self.parse_features(&self.values[5])?,
        )
    }

    fn parse_imports(&self, value: &NotaValue) -> Result<Imports> {
        let entries = expect_map(value, "imports")?
            .iter()
            .map(|entry| {
                let binding = Name::new(entry.key().to_owned())?;
                let directive = self.parse_import_directive(entry.value())?;
                Ok((binding, directive))
            })
            .collect::<Result<Vec<_>>>()?;
        Imports::new(entries)
    }

    fn parse_import_directive(&self, value: &NotaValue) -> Result<ImportDirective> {
        let shape = expect_record(value, "import directive")?;
        let head = expect_record_head(shape, "import directive")?;
        let data = expect_record_data(shape, "import directive")?;
        match head {
            "Import" => {
                expect_exact_count("import directive", data.len(), 2)?;
                let path = SchemaPath::new(expect_text(&data[0], "import path")?);
                let names = self.parse_name_sequence(&data[1], "import selection")?;
                Ok(ImportDirective::import(path, names))
            }
            "ImportAll" => {
                expect_exact_count("import directive", data.len(), 1)?;
                let path = SchemaPath::new(expect_text(&data[0], "import path")?);
                Ok(ImportDirective::import_all(path))
            }
            _ => Err(Error::InvalidSchemaText {
                context: "import directive",
                message: format!("unknown import directive `{head}`"),
            }),
        }
    }

    fn parse_header(&self, value: &NotaValue, context: &'static str) -> Result<Header> {
        let roots = expect_sequence(value, context)?
            .iter()
            .map(|root| {
                let shape = expect_record(root, context)?;
                let root_name = Name::new(expect_record_head(shape, context)?.to_owned())?;
                let data = expect_record_data(shape, context)?;
                expect_exact_count(context, data.len(), 1)?;
                let endpoints = self.parse_name_record(&data[0], context)?;
                HeaderRoot::new(root_name, endpoints)
            })
            .collect::<Result<Vec<_>>>()?;
        Header::new(roots)
    }

    fn parse_namespace(&self, value: &NotaValue) -> Result<Namespace> {
        let declarations = expect_map(value, "namespace")?
            .iter()
            .map(|entry| {
                let name = Name::new(entry.key().to_owned())?;
                let body = self.parse_declaration_body(entry.value())?;
                Ok(Declaration::new(name, body))
            })
            .collect::<Result<Vec<_>>>()?;
        Namespace::declarations(declarations)
    }

    fn parse_declaration_body(&self, value: &NotaValue) -> Result<DeclarationBody> {
        match value.kind() {
            NotaValueKind::Sequence => self.parse_record_body(value),
            NotaValueKind::Record => self.parse_enum_body(value),
            _ => self
                .parse_type_expression(value)
                .map(DeclarationBody::Alias),
        }
    }

    fn parse_enum_body(&self, value: &NotaValue) -> Result<DeclarationBody> {
        let variants = expect_record_values(value, "enum declaration")?
            .iter()
            .map(|variant| self.parse_variant(variant))
            .collect::<Result<Vec<_>>>()?;
        Ok(DeclarationBody::Enum { variants })
    }

    fn parse_variant(&self, value: &NotaValue) -> Result<Variant> {
        if value.is_record() {
            let shape = expect_record(value, "data-carrying variant")?;
            let name = Name::new(expect_record_head(shape, "variant name")?.to_owned())?;
            let data = expect_record_data(shape, "data-carrying variant")?;
            if data.is_empty() {
                return Ok(Variant::with_type(
                    name.clone(),
                    TypeExpression::named(name),
                ));
            }
            let fields = data
                .iter()
                .map(|field| self.parse_field(field))
                .collect::<Result<Vec<_>>>()?;
            reject_repeated_self_payload(&name, &fields, "data-carrying variant")?;
            Ok(variant_with_fields(name, fields))
        } else {
            self.read_name(value, "unit variant").map(Variant::unit)
        }
    }

    fn parse_record_body(&self, value: &NotaValue) -> Result<DeclarationBody> {
        let values = expect_sequence(value, "record declaration")?;
        let fields = values
            .iter()
            .map(|field| self.parse_field(field))
            .collect::<Result<Vec<_>>>()?;

        match fields.len() {
            0 => Err(Error::InvalidSchemaText {
                context: "record declaration",
                message: "declaration record must carry at least one type expression".into(),
            }),
            1 if fields[0].name().is_none() => {
                Ok(DeclarationBody::Newtype(fields[0].expression().clone()))
            }
            _ => Ok(DeclarationBody::Record(fields)),
        }
    }

    fn parse_field(&self, value: &NotaValue) -> Result<Field> {
        if value.is_record() {
            return self.parse_container_field(value);
        }
        self.parse_type_expression(value).map(Field::inferred)
    }

    fn parse_type_expression(&self, value: &NotaValue) -> Result<TypeExpression> {
        if value.is_record() {
            return self.parse_container_expression(value);
        }

        let text = expect_text(value, "type expression")?;
        if let Some(primitive) = primitive(&text) {
            Ok(primitive)
        } else {
            Name::new(text).map(TypeExpression::named)
        }
    }

    fn parse_container_expression(&self, value: &NotaValue) -> Result<TypeExpression> {
        let shape = expect_record(value, "container expression")?;
        let head = expect_record_head(shape, "container expression")?;
        let data = expect_record_data(shape, "container expression")?;
        self.parse_container_expression_after_head(head, data)
    }

    fn parse_container_field(&self, value: &NotaValue) -> Result<Field> {
        let values = expect_record_values(value, "field")?;
        let Some(head_value) = values.first() else {
            return Err(Error::InvalidSchemaText {
                context: "field",
                message: "field record must carry a head".into(),
            });
        };
        let head = expect_text(head_value, "field head")?;
        let data = &values[1..];
        if is_container_head(&head) {
            return self
                .parse_container_expression_after_head(&head, data)
                .map(Field::inferred);
        }
        if data.is_empty() {
            return self.parse_type_expression(head_value).map(Field::inferred);
        }

        Err(Error::InvalidSchemaText {
            context: "field",
            message: format!(
                "field names are inferred from type names; `{head}` is not a container type expression"
            ),
        })
    }

    fn parse_container_expression_after_head(
        &self,
        head: &str,
        data: &[NotaValue],
    ) -> Result<TypeExpression> {
        match head {
            "Option" => {
                expect_exact_count("container expression", data.len(), 1)?;
                self.parse_type_expression(&data[0])
                    .map(TypeExpression::optional)
            }
            "Vec" => {
                expect_exact_count("container expression", data.len(), 1)?;
                self.parse_type_expression(&data[0])
                    .map(TypeExpression::vector)
            }
            "Map" => {
                expect_exact_count("container expression", data.len(), 2)?;
                let key = self.parse_type_expression(&data[0])?;
                let value = self.parse_type_expression(&data[1])?;
                Ok(TypeExpression::map(key, value))
            }
            _ => Err(Error::InvalidSchemaText {
                context: "container expression",
                message: format!("unknown container `{head}`"),
            }),
        }
    }

    fn parse_features(&self, value: &NotaValue) -> Result<Vec<Feature>> {
        expect_sequence(value, "features")?
            .iter()
            .map(|feature| self.parse_feature(feature))
            .collect()
    }

    fn parse_feature(&self, value: &NotaValue) -> Result<Feature> {
        let shape = expect_record(value, "feature")?;
        let head = expect_record_head(shape, "feature")?;
        let data = expect_record_data(shape, "feature")?;
        match head {
            "Reply" => self.parse_names(data, "reply feature").map(Feature::Reply),
            "Event" => self.parse_event_feature(data),
            "Observable" => self.parse_observable_feature(data),
            "Upgrade" => self.parse_upgrade_feature(data),
            _ => Err(Error::InvalidSchemaText {
                context: "feature",
                message: format!("unknown feature `{head}`"),
            }),
        }
    }

    fn parse_event_feature(&self, data: &[NotaValue]) -> Result<Feature> {
        let (stream, event_values) = if data
            .first()
            .is_some_and(|value| value.has_record_head("belongs"))
        {
            (Some(self.parse_belongs_record(&data[0])?), &data[1..])
        } else {
            (None, data)
        };
        let events = self.parse_names(event_values, "event feature")?;
        Ok(Feature::Event(EventFeature::new(stream, events)))
    }

    fn parse_belongs_record(&self, value: &NotaValue) -> Result<Name> {
        let shape = expect_record(value, "event belongs")?;
        let relation = expect_record_head(shape, "event belongs")?;
        if relation != "belongs" {
            return Err(Error::InvalidSchemaText {
                context: "event belongs",
                message: format!("expected `belongs`, got `{relation}`"),
            });
        }
        let data = expect_record_data(shape, "event belongs")?;
        expect_exact_count("event belongs", data.len(), 1)?;
        self.read_name(&data[0], "event stream")
    }

    fn parse_observable_feature(&self, data: &[NotaValue]) -> Result<Feature> {
        let mut filter = None;
        let mut operation_event = None;
        let mut effect_event = None;

        for field in data {
            let shape = expect_record(field, "observable field")?;
            let name = expect_record_head(shape, "observable field name")?;
            let values = expect_record_data(shape, "observable field")?;
            expect_exact_count("observable field", values.len(), 1)?;
            match name {
                "filter" => filter = Some(expect_text(&values[0], "observable filter")?),
                "operation_event" => {
                    operation_event = Some(self.read_name(&values[0], "operation event")?)
                }
                "effect_event" => effect_event = Some(self.read_name(&values[0], "effect event")?),
                _ => {
                    return Err(Error::InvalidSchemaText {
                        context: "observable feature",
                        message: format!("unknown observable field `{name}`"),
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

    fn parse_upgrade_feature(&self, data: &[NotaValue]) -> Result<Feature> {
        let Some(from_version_value) = data.first() else {
            return Err(Error::InvalidSchemaText {
                context: "upgrade feature",
                message: "missing FromVersion record".into(),
            });
        };
        let from_version = self.parse_from_version(from_version_value)?;
        let annotations = data[1..]
            .iter()
            .map(|annotation| self.parse_upgrade_annotation(annotation))
            .collect::<Result<Vec<_>>>()?;
        Ok(Feature::Upgrade(Upgrade::new(from_version, annotations)))
    }

    fn parse_from_version(&self, value: &NotaValue) -> Result<Version> {
        let shape = expect_record(value, "upgrade version")?;
        let head = expect_record_head(shape, "upgrade version")?;
        if head != "FromVersion" {
            return Err(Error::InvalidSchemaText {
                context: "upgrade version",
                message: format!("expected `FromVersion`, got `{head}`"),
            });
        }
        let data = expect_record_data(shape, "upgrade version")?;
        expect_exact_count("upgrade version", data.len(), 1)?;
        Ok(Version::new(expect_text(
            &data[0],
            "upgrade source version",
        )?))
    }

    fn parse_upgrade_annotation(&self, value: &NotaValue) -> Result<UpgradeAnnotation> {
        let shape = expect_record(value, "upgrade annotation")?;
        let head = expect_record_head(shape, "upgrade annotation")?;
        let data = expect_record_data(shape, "upgrade annotation")?;
        match head {
            "Migrate" => {
                expect_exact_count("upgrade annotation", data.len(), 1)?;
                self.read_name(&data[0], "migrated type")
                    .map(UpgradeAnnotation::Migrate)
            }
            "RenamedFrom" => {
                expect_exact_count("upgrade annotation", data.len(), 2)?;
                Ok(UpgradeAnnotation::RenamedFrom {
                    current: self.read_name(&data[0], "current type")?,
                    previous: self.read_name(&data[1], "previous type")?,
                })
            }
            "Drop" => {
                expect_exact_count("upgrade annotation", data.len(), 1)?;
                self.read_name(&data[0], "dropped type")
                    .map(UpgradeAnnotation::Drop)
            }
            "Custom" => {
                expect_exact_count("upgrade annotation", data.len(), 2)?;
                Ok(UpgradeAnnotation::Custom {
                    name: self.read_name(&data[0], "custom migrated type")?,
                    implementation: self.read_name(&data[1], "custom migration implementation")?,
                })
            }
            "Untranslatable" => {
                expect_exact_count("upgrade annotation", data.len(), 1)?;
                self.read_name(&data[0], "untranslatable type")
                    .map(UpgradeAnnotation::Untranslatable)
            }
            _ => Err(Error::InvalidSchemaText {
                context: "upgrade annotation",
                message: format!("unknown upgrade annotation `{head}`"),
            }),
        }
    }

    fn parse_name_sequence(&self, value: &NotaValue, context: &'static str) -> Result<Vec<Name>> {
        let values = expect_sequence(value, context)?;
        self.parse_names(values, context)
    }

    fn parse_name_record(&self, value: &NotaValue, context: &'static str) -> Result<Vec<Name>> {
        let values = expect_record_values(value, context)?;
        self.parse_names(values, context)
    }

    fn parse_names(&self, values: &[NotaValue], context: &'static str) -> Result<Vec<Name>> {
        values
            .iter()
            .map(|value| self.read_name(value, context))
            .collect()
    }

    fn read_name(&self, value: &NotaValue, context: &'static str) -> Result<Name> {
        let Some(text) = value.identifier_text() else {
            return Err(Error::InvalidSchemaText {
                context,
                message: format!("expected PascalCase identifier, got {:?}", value.kind()),
            });
        };
        Name::new(text.to_owned())
    }
}

fn expect_map<'value>(
    value: &'value NotaValue,
    context: &'static str,
) -> Result<&'value [nota_codec::NotaMapEntry]> {
    value.as_map().ok_or_else(|| Error::InvalidSchemaText {
        context,
        message: format!("expected map, got {:?}", value.kind()),
    })
}

fn expect_sequence<'value>(
    value: &'value NotaValue,
    context: &'static str,
) -> Result<&'value [NotaValue]> {
    value.as_sequence().ok_or_else(|| Error::InvalidSchemaText {
        context,
        message: format!("expected sequence, got {:?}", value.kind()),
    })
}

fn expect_record<'value>(
    value: &'value NotaValue,
    context: &'static str,
) -> Result<nota_codec::NotaRecordShape<'value>> {
    value
        .as_record_shape()
        .ok_or_else(|| Error::InvalidSchemaText {
            context,
            message: format!("expected record, got {:?}", value.kind()),
        })
}

fn expect_record_values<'value>(
    value: &'value NotaValue,
    context: &'static str,
) -> Result<&'value [NotaValue]> {
    expect_record(value, context).map(nota_codec::NotaRecordShape::values)
}

fn expect_record_head<'value>(
    shape: nota_codec::NotaRecordShape<'value>,
    context: &'static str,
) -> Result<&'value str> {
    shape.head().ok_or_else(|| Error::InvalidSchemaText {
        context,
        message: "expected record head identifier".into(),
    })
}

fn expect_record_data<'value>(
    shape: nota_codec::NotaRecordShape<'value>,
    context: &'static str,
) -> Result<&'value [NotaValue]> {
    shape.data().ok_or_else(|| Error::InvalidSchemaText {
        context,
        message: "expected record head identifier".into(),
    })
}

fn expect_text(value: &NotaValue, context: &'static str) -> Result<String> {
    value
        .identifier_text()
        .or_else(|| value.string_text())
        .map(ToOwned::to_owned)
        .ok_or_else(|| Error::InvalidSchemaText {
            context,
            message: format!("expected identifier or string, got {:?}", value.kind()),
        })
}

fn expect_exact_count(context: &'static str, got: usize, expected: usize) -> Result<()> {
    if got == expected {
        Ok(())
    } else {
        Err(Error::InvalidSchemaText {
            context,
            message: format!("expected {expected} values, got {got}"),
        })
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

fn nota_error(context: &'static str, error: nota_codec::Error) -> Error {
    Error::InvalidSchemaText {
        context,
        message: error.to_string(),
    }
}
