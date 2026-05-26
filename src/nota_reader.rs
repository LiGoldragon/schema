use nota_codec::NotaValue;

use crate::{
    Container, DeclarationBody, Error, Field, ModuleName, Name, NamespaceObject, ObjectDelimiter,
    Payload, Primitive, Result, SchemaObjectPass, TypeExpression, Variant,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledNotaSchema {
    module: ModuleName,
    types: Vec<AssembledNotaType>,
}

impl AssembledNotaSchema {
    pub fn from_namespace_text(module: ModuleName, text: &str) -> Result<Self> {
        let pass = SchemaObjectPass::parse_text(module, text)?;
        Self::from_object_pass(&pass)
    }

    pub fn from_object_pass(pass: &SchemaObjectPass) -> Result<Self> {
        let namespace = pass
            .namespace_roots()
            .last()
            .ok_or_else(|| Error::InvalidSchemaText {
                context: "nota reader schema",
                message: "expected at least one curly-brace namespace map".into(),
            })?;

        let types = namespace
            .namespace_entries()?
            .into_iter()
            .map(NamespaceTypeLowerer::lower)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            module: pass.namespace_prefix().clone(),
            types,
        })
    }

    pub fn module(&self) -> &ModuleName {
        &self.module
    }

    pub fn types(&self) -> &[AssembledNotaType] {
        &self.types
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssembledNotaType {
    name: Name,
    body: DeclarationBody,
}

impl AssembledNotaType {
    pub fn new(name: Name, body: DeclarationBody) -> Self {
        Self { name, body }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn body(&self) -> &DeclarationBody {
        &self.body
    }
}

pub struct NotaReaderRustEmitter;

impl NotaReaderRustEmitter {
    pub fn emit_module(schema: &AssembledNotaSchema) -> Result<String> {
        let mut output = String::new();
        output.push_str(&format!("pub mod {} {{\n", schema.module().as_str()));
        output.push_str("    use nota_codec::{Decoder, NotaDecode, Result};\n\n");

        for schema_type in schema.types() {
            Self::emit_type(schema_type, &mut output)?;
            output.push('\n');
        }

        output.push_str("}\n");
        Ok(output)
    }

    fn emit_type(schema_type: &AssembledNotaType, output: &mut String) -> Result<()> {
        match schema_type.body() {
            DeclarationBody::Newtype(expression) => {
                Self::emit_newtype(schema_type.name(), expression, output)
            }
            DeclarationBody::Record(fields) => {
                Self::emit_record(schema_type.name(), fields, output)
            }
            DeclarationBody::Enum { variants } => {
                Self::emit_enum(schema_type.name(), variants, output)
            }
            DeclarationBody::Alias(expression) => {
                Self::emit_alias(schema_type.name(), expression, output)
            }
        }
    }

    fn emit_newtype(name: &Name, expression: &TypeExpression, output: &mut String) -> Result<()> {
        let type_text = TypeRenderer::rust_type(expression)?;
        let decode = TypeRenderer::decode_expression(expression, "decoder")?;

        output.push_str("#[derive(Clone, Debug, PartialEq, Eq)]\n");
        output.push_str(&format!("pub struct {}(pub {});\n\n", name, type_text));
        output.push_str(&format!("impl NotaDecode for {} {{\n", name));
        output.push_str("    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {\n");
        output.push_str(&format!("        Ok(Self({decode}))\n"));
        output.push_str("    }\n");
        output.push_str("}\n");
        Ok(())
    }

    fn emit_record(name: &Name, fields: &[Field], output: &mut String) -> Result<()> {
        output.push_str("#[derive(Clone, Debug, PartialEq, Eq)]\n");
        output.push_str(&format!("pub struct {} {{\n", name));
        for field in fields {
            output.push_str(&format!(
                "    pub {}: {},\n",
                field.effective_name().as_str(),
                TypeRenderer::rust_type(field.expression())?
            ));
        }
        output.push_str("}\n\n");

        output.push_str(&format!("impl NotaDecode for {} {{\n", name));
        output.push_str("    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {\n");
        output.push_str(&format!(
            "        decoder.expect_positional_record_start(\"{}\", {})?;\n",
            name,
            fields.len()
        ));
        for field in fields {
            output.push_str(&format!(
                "        let {} = {};\n",
                field.effective_name().as_str(),
                TypeRenderer::decode_expression(field.expression(), "decoder")?
            ));
        }
        output.push_str("        decoder.expect_record_end()?;\n");
        output.push_str("        Ok(Self {\n");
        for field in fields {
            output.push_str(&format!(
                "            {},\n",
                field.effective_name().as_str()
            ));
        }
        output.push_str("        })\n");
        output.push_str("    }\n");
        output.push_str("}\n");
        Ok(())
    }

    fn emit_enum(name: &Name, variants: &[Variant], output: &mut String) -> Result<()> {
        output.push_str("#[derive(Clone, Debug, PartialEq, Eq)]\n");
        output.push_str(&format!("pub enum {} {{\n", name));
        for variant in variants {
            match variant.payload() {
                Payload::Unit => output.push_str(&format!("    {},\n", variant.name())),
                Payload::Type(expression) => output.push_str(&format!(
                    "    {}({}),\n",
                    variant.name(),
                    TypeRenderer::rust_type(expression)?
                )),
                Payload::Fields(_) => {
                    return Err(Error::InvalidSchemaText {
                        context: "nota reader rust emitter",
                        message: format!(
                            "enum variant `{name}.{}` uses field payloads; prototype reader supports unit or single-type payload variants",
                            variant.name()
                        ),
                    });
                }
            }
        }
        output.push_str("}\n\n");

        output.push_str(&format!("impl NotaDecode for {} {{\n", name));
        output.push_str("    fn decode(decoder: &mut Decoder<'_>) -> Result<Self> {\n");
        output.push_str("        if decoder.peek_is_record_start()? {\n");
        output.push_str("            let variant = decoder.peek_record_head()?;\n");
        output.push_str("            match variant.as_str() {\n");
        for variant in variants {
            match variant.payload() {
                Payload::Unit => output.push_str(&format!(
                    "                \"{}\" => Err(nota_codec::Error::UnitVariantInRecordForm {{ enum_name: \"{}\", variant: \"{}\" }}),\n",
                    variant.name(),
                    name,
                    variant.name()
                )),
                Payload::Type(expression) => {
                    let field = variant.name().as_str().to_ascii_lowercase();
                    output.push_str(&format!("                \"{}\" => {{\n", variant.name()));
                    output.push_str(&format!(
                        "                    decoder.expect_record_head(\"{}\")?;\n",
                        variant.name()
                    ));
                    output.push_str(&format!(
                        "                    let {} = {};\n",
                        field,
                        TypeRenderer::decode_expression(expression, "decoder")?
                    ));
                    output.push_str("                    decoder.expect_record_end()?;\n");
                    output.push_str(&format!(
                        "                    Ok(Self::{}({}))\n",
                        variant.name(),
                        field
                    ));
                    output.push_str("                }\n");
                }
                Payload::Fields(_) => {}
            }
        }
        output.push_str(&format!(
            "                _ => Err(nota_codec::Error::UnknownVariant {{ enum_name: \"{}\", got: variant }}),\n",
            name
        ));
        output.push_str("            }\n");
        output.push_str("        } else {\n");
        output.push_str("            let variant = decoder.read_pascal_identifier()?;\n");
        output.push_str("            match variant.as_str() {\n");
        for variant in variants {
            match variant.payload() {
                Payload::Unit => output.push_str(&format!(
                    "                \"{}\" => Ok(Self::{}),\n",
                    variant.name(),
                    variant.name()
                )),
                Payload::Type(_) => output.push_str(&format!(
                    "                \"{}\" => Err(nota_codec::Error::DataVariantWithoutRecord {{ enum_name: \"{}\", variant: \"{}\" }}),\n",
                    variant.name(),
                    name,
                    variant.name()
                )),
                Payload::Fields(_) => {}
            }
        }
        output.push_str(&format!(
            "                _ => Err(nota_codec::Error::UnknownVariant {{ enum_name: \"{}\", got: variant }}),\n",
            name
        ));
        output.push_str("            }\n");
        output.push_str("        }\n");
        output.push_str("    }\n");
        output.push_str("}\n");
        Ok(())
    }

    fn emit_alias(name: &Name, expression: &TypeExpression, output: &mut String) -> Result<()> {
        output.push_str(&format!(
            "pub type {} = {};\n",
            name,
            TypeRenderer::rust_type(expression)?
        ));
        Ok(())
    }
}

struct NamespaceTypeLowerer;

impl NamespaceTypeLowerer {
    fn lower(entry: NamespaceObject<'_>) -> Result<AssembledNotaType> {
        let name = Name::new(entry.name())?;
        let body = match entry.delimiter() {
            ObjectDelimiter::SquareBrackets => Self::lower_sequence(entry.value())?,
            ObjectDelimiter::Parentheses => Self::lower_enum(entry.value())?,
            ObjectDelimiter::Atom => {
                DeclarationBody::Alias(TypeExpressionLowerer::lower(entry.value())?)
            }
            ObjectDelimiter::CurlyBraces => {
                return Err(Error::InvalidSchemaText {
                    context: "nota reader namespace",
                    message: format!(
                        "nested map declaration `{name}` is not part of the prototype"
                    ),
                });
            }
        };
        Ok(AssembledNotaType::new(name, body))
    }

    fn lower_sequence(value: &NotaValue) -> Result<DeclarationBody> {
        let items = value.as_sequence().expect("delimiter already checked");
        if items.is_empty() {
            return Err(Error::InvalidSchemaText {
                context: "nota reader namespace",
                message: "struct field vector must not be empty".into(),
            });
        }
        let fields = items
            .iter()
            .map(TypeExpressionLowerer::field)
            .collect::<Result<Vec<_>>>()?;
        if let [field] = fields.as_slice() {
            return Ok(DeclarationBody::Newtype(field.expression().clone()));
        }
        Ok(DeclarationBody::Record(fields))
    }

    fn lower_enum(value: &NotaValue) -> Result<DeclarationBody> {
        let items = value.as_record().expect("delimiter already checked");
        let variants = items
            .iter()
            .map(TypeExpressionLowerer::variant)
            .collect::<Result<Vec<_>>>()?;
        Ok(DeclarationBody::Enum { variants })
    }
}

struct TypeExpressionLowerer;

impl TypeExpressionLowerer {
    fn field(value: &NotaValue) -> Result<Field> {
        TypeExpressionLowerer::lower(value).map(Field::inferred)
    }

    fn variant(value: &NotaValue) -> Result<Variant> {
        if let Some(text) = value.identifier_text() {
            return Name::new(text).map(Variant::unit);
        }

        let Some(items) = value.as_record() else {
            return Err(Error::InvalidSchemaText {
                context: "nota reader enum",
                message: "enum variants must be identifiers or parenthesized data variants".into(),
            });
        };

        let Some(head) = items.first().and_then(NotaValue::identifier_text) else {
            return Err(Error::InvalidSchemaText {
                context: "nota reader enum",
                message: "data-carrying enum variant must start with an identifier".into(),
            });
        };
        let name = Name::new(head)?;

        match items {
            [_] => Ok(Variant::with_type(
                name.clone(),
                TypeExpression::named(name),
            )),
            [_, payload] => Ok(Variant::with_type(name, Self::lower(payload)?)),
            _ => Err(Error::InvalidSchemaText {
                context: "nota reader enum",
                message: format!(
                    "variant `{name}` has multiple payload fields; prototype reader supports one payload type"
                ),
            }),
        }
    }

    fn lower(value: &NotaValue) -> Result<TypeExpression> {
        if let Some(text) = value.identifier_text() {
            return Self::named_or_primitive(text);
        }

        let Some(items) = value.as_record() else {
            return Err(Error::InvalidSchemaText {
                context: "nota reader type expression",
                message: format!("cannot lower `{value:?}` as a type expression"),
            });
        };
        let Some(head) = items.first().and_then(NotaValue::identifier_text) else {
            return Err(Error::InvalidSchemaText {
                context: "nota reader type expression",
                message: "container expression must start with an identifier".into(),
            });
        };
        Self::container(head, &items[1..])
    }

    fn named_or_primitive(text: &str) -> Result<TypeExpression> {
        let expression = match text {
            "String" => TypeExpression::Primitive(Primitive::String),
            "Bytes" => TypeExpression::Primitive(Primitive::Bytes),
            "Boolean" | "bool" => TypeExpression::Primitive(Primitive::Boolean),
            "u8" => TypeExpression::Primitive(Primitive::Unsigned8),
            "u16" => TypeExpression::Primitive(Primitive::Unsigned16),
            "u32" => TypeExpression::Primitive(Primitive::Unsigned32),
            "u64" => TypeExpression::Primitive(Primitive::Unsigned64),
            "Date" => TypeExpression::Primitive(Primitive::Date),
            "Time" => TypeExpression::Primitive(Primitive::Time),
            _ => TypeExpression::named(Name::new(text)?),
        };
        Ok(expression)
    }

    fn container(head: &str, rest: &[NotaValue]) -> Result<TypeExpression> {
        match head {
            "Vec" => {
                let [inner] = rest else {
                    return Err(Error::InvalidSchemaText {
                        context: "nota reader type expression",
                        message: "Vec must wrap exactly one type expression".into(),
                    });
                };
                Ok(TypeExpression::vector(Self::lower(inner)?))
            }
            "Option" => {
                let [inner] = rest else {
                    return Err(Error::InvalidSchemaText {
                        context: "nota reader type expression",
                        message: "Option must wrap exactly one type expression".into(),
                    });
                };
                Ok(TypeExpression::optional(Self::lower(inner)?))
            }
            "Map" => {
                let [key, value] = rest else {
                    return Err(Error::InvalidSchemaText {
                        context: "nota reader type expression",
                        message: "Map must carry key and value type expressions".into(),
                    });
                };
                Ok(TypeExpression::map(Self::lower(key)?, Self::lower(value)?))
            }
            other => Err(Error::InvalidSchemaText {
                context: "nota reader type expression",
                message: format!("unknown container `{other}`"),
            }),
        }
    }
}

struct TypeRenderer;

impl TypeRenderer {
    fn rust_type(expression: &TypeExpression) -> Result<String> {
        match expression {
            TypeExpression::Primitive(primitive) => Ok(Self::primitive_type(*primitive).into()),
            TypeExpression::Named(name) => Ok(name.to_string()),
            TypeExpression::Container(container) => match container {
                Container::Vector(inner) => Ok(format!("Vec<{}>", Self::rust_type(inner)?)),
                Container::Optional(inner) => Ok(format!("Option<{}>", Self::rust_type(inner)?)),
                Container::Map { key, value } => Ok(format!(
                    "::std::collections::BTreeMap<{}, {}>",
                    Self::rust_type(key)?,
                    Self::rust_type(value)?
                )),
            },
        }
    }

    fn decode_expression(expression: &TypeExpression, decoder: &str) -> Result<String> {
        match expression {
            TypeExpression::Primitive(Primitive::Date) => Ok(format!("{decoder}.read_date()?")),
            TypeExpression::Primitive(Primitive::Time) => Ok(format!("{decoder}.read_time()?")),
            _ => Ok(format!(
                "<{} as NotaDecode>::decode({decoder})?",
                Self::rust_type(expression)?
            )),
        }
    }

    fn primitive_type(primitive: Primitive) -> &'static str {
        match primitive {
            Primitive::String => "String",
            Primitive::Bytes => "Vec<u8>",
            Primitive::Boolean => "bool",
            Primitive::Unsigned8 => "u8",
            Primitive::Unsigned16 => "u16",
            Primitive::Unsigned32 => "u32",
            Primitive::Unsigned64 => "u64",
            Primitive::Date => "(u16, u8, u8)",
            Primitive::Time => "(u8, u8, u8)",
        }
    }
}
