use crate::{FieldName, Name};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum TypeExpression {
    Primitive(Primitive),
    Named(Name),
    Container(Container),
}

impl TypeExpression {
    pub fn named(name: Name) -> Self {
        Self::Named(name)
    }

    pub fn vector(inner: TypeExpression) -> Self {
        Self::Container(Container::Vector(Box::new(inner)))
    }

    pub fn optional(inner: TypeExpression) -> Self {
        Self::Container(Container::Optional(Box::new(inner)))
    }

    pub fn map(key: TypeExpression, value: TypeExpression) -> Self {
        Self::Container(Container::Map {
            key: Box::new(key),
            value: Box::new(value),
        })
    }

    pub fn derived_field_name(&self) -> FieldName {
        match self {
            Self::Primitive(primitive) => primitive.derived_field_name(),
            Self::Named(name) => FieldName::from_schema_name(name),
            Self::Container(container) => container.derived_field_name(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Primitive {
    String,
    Bytes,
    Boolean,
    Unsigned8,
    Unsigned16,
    Unsigned32,
    Unsigned64,
    Date,
    Time,
}

impl Primitive {
    pub fn is_fixed_width(self) -> bool {
        !matches!(self, Self::String | Self::Bytes)
    }

    pub fn derived_field_name(self) -> FieldName {
        let text = match self {
            Self::String => "string",
            Self::Bytes => "bytes",
            Self::Boolean => "boolean",
            Self::Unsigned8 => "unsigned8",
            Self::Unsigned16 => "unsigned16",
            Self::Unsigned32 => "unsigned32",
            Self::Unsigned64 => "unsigned64",
            Self::Date => "date",
            Self::Time => "time",
        };
        FieldName::from_primitive(text)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Container {
    Vector(Box<TypeExpression>),
    Optional(Box<TypeExpression>),
    Map {
        key: Box<TypeExpression>,
        value: Box<TypeExpression>,
    },
}

impl Container {
    pub fn derived_field_name(&self) -> FieldName {
        match self {
            Self::Vector(inner) | Self::Optional(inner) => inner.derived_field_name(),
            Self::Map { .. } => FieldName::from_primitive("map"),
        }
    }
}
