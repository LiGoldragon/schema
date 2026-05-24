use crate::Name;

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
