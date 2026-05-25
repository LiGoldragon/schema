use crate::{Container, FieldName, Name, Primitive, TypeExpression};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    name: Name,
    body: DeclarationBody,
}

impl Declaration {
    pub fn new(name: Name, body: DeclarationBody) -> Self {
        Self { name, body }
    }

    pub fn enumeration(name: Name, variants: Vec<Variant>) -> Self {
        Self {
            name,
            body: DeclarationBody::Enum { variants },
        }
    }

    pub fn newtype(name: Name, expression: TypeExpression) -> Self {
        Self {
            name,
            body: DeclarationBody::Newtype(expression),
        }
    }

    pub fn record(name: Name, fields: Vec<TypeExpression>) -> Self {
        Self::record_fields(name, fields.into_iter().map(Field::inferred).collect())
    }

    pub fn record_fields(name: Name, fields: Vec<Field>) -> Self {
        Self {
            name,
            body: DeclarationBody::Record(fields),
        }
    }

    pub fn alias(name: Name, expression: TypeExpression) -> Self {
        Self {
            name,
            body: DeclarationBody::Alias(expression),
        }
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn body(&self) -> &DeclarationBody {
        &self.body
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeclarationBody {
    Enum { variants: Vec<Variant> },
    Newtype(TypeExpression),
    Record(Vec<Field>),
    Alias(TypeExpression),
}

impl DeclarationBody {
    pub fn storage_matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Enum { variants: current }, Self::Enum { variants: previous }) => {
                current.len() == previous.len()
                    && current
                        .iter()
                        .zip(previous)
                        .all(|(current, previous)| current.storage_matches(previous))
            }
            (Self::Newtype(current), Self::Newtype(previous))
            | (Self::Alias(current), Self::Alias(previous)) => current == previous,
            (Self::Record(current), Self::Record(previous)) => {
                current.len() == previous.len()
                    && current
                        .iter()
                        .zip(previous)
                        .all(|(current, previous)| current.storage_matches(previous))
            }
            _ => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    name: Name,
    payload: Payload,
    engine: Option<Engine>,
}

impl Variant {
    pub fn unit(name: Name) -> Self {
        Self {
            name,
            payload: Payload::Unit,
            engine: None,
        }
    }

    pub fn with_type(name: Name, expression: TypeExpression) -> Self {
        Self {
            name,
            payload: Payload::Type(expression),
            engine: None,
        }
    }

    pub fn with_fields(name: Name, fields: Vec<TypeExpression>) -> Self {
        Self::with_field_entries(name, fields.into_iter().map(Field::inferred).collect())
    }

    pub fn with_field_entries(name: Name, fields: Vec<Field>) -> Self {
        Self {
            name,
            payload: Payload::Fields(fields),
            engine: None,
        }
    }

    pub fn with_engine(mut self, engine: Engine) -> Self {
        self.engine = Some(engine);
        self
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    pub fn engine(&self) -> Option<Engine> {
        self.engine
    }

    pub fn storage_matches(&self, other: &Self) -> bool {
        self.name == other.name
            && self.payload.storage_matches(&other.payload)
            && self.engine == other.engine
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    Unit,
    Type(TypeExpression),
    Fields(Vec<Field>),
}

impl Payload {
    pub fn storage_matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Unit, Self::Unit) => true,
            (Self::Type(current), Self::Type(previous)) => current == previous,
            (Self::Fields(current), Self::Fields(previous)) => {
                current.len() == previous.len()
                    && current
                        .iter()
                        .zip(previous)
                        .all(|(current, previous)| current.storage_matches(previous))
            }
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Engine {
    Assert,
    Mutate,
    Retract,
    Match,
    Subscribe,
    Validate,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    name: Option<FieldName>,
    expression: TypeExpression,
}

impl Field {
    pub fn inferred(expression: TypeExpression) -> Self {
        Self {
            name: None,
            expression,
        }
    }

    pub fn named(name: FieldName, expression: TypeExpression) -> Self {
        Self {
            name: Some(name),
            expression,
        }
    }

    pub fn name(&self) -> Option<&FieldName> {
        self.name.as_ref()
    }

    pub fn derived_name(&self) -> FieldName {
        field_name_for_expression(&self.expression)
    }

    pub fn effective_name(&self) -> FieldName {
        self.name.clone().unwrap_or_else(|| self.derived_name())
    }

    pub fn expression(&self) -> &TypeExpression {
        &self.expression
    }

    pub fn storage_matches(&self, other: &Self) -> bool {
        self.expression == other.expression
    }
}

impl From<TypeExpression> for Field {
    fn from(expression: TypeExpression) -> Self {
        Self::inferred(expression)
    }
}

fn field_name_for_expression(expression: &TypeExpression) -> FieldName {
    FieldName::new(field_name_text_for_expression(expression))
        .expect("schema-derived field names are generated in field-name shape")
}

fn field_name_text_for_expression(expression: &TypeExpression) -> String {
    match expression {
        TypeExpression::Named(name) => pascal_to_snake(name.as_str()),
        TypeExpression::Primitive(primitive) => primitive_field_name(*primitive).into(),
        TypeExpression::Container(Container::Optional(inner)) => {
            format!("option_{}", field_name_text_for_expression(inner))
        }
        TypeExpression::Container(Container::Vector(inner)) => {
            format!("vec_{}", field_name_text_for_expression(inner))
        }
        TypeExpression::Container(Container::Map { key, value }) => format!(
            "map_{}_{}",
            field_name_text_for_expression(key),
            field_name_text_for_expression(value)
        ),
    }
}

fn primitive_field_name(primitive: Primitive) -> &'static str {
    match primitive {
        Primitive::String => "string",
        Primitive::Bytes => "bytes",
        Primitive::Boolean => "boolean",
        Primitive::Unsigned8 => "u8",
        Primitive::Unsigned16 => "u16",
        Primitive::Unsigned32 => "u32",
        Primitive::Unsigned64 => "u64",
        Primitive::Date => "date",
        Primitive::Time => "time",
    }
}

fn pascal_to_snake(value: &str) -> String {
    let mut output = String::new();
    let mut previous_was_lower_or_digit = false;
    for character in value.chars() {
        if character.is_ascii_uppercase() {
            if previous_was_lower_or_digit {
                output.push('_');
            }
            output.push(character.to_ascii_lowercase());
            previous_was_lower_or_digit = false;
        } else {
            output.push(character);
            previous_was_lower_or_digit =
                character.is_ascii_lowercase() || character.is_ascii_digit();
        }
    }
    output
}
