use crate::{Name, TypeExpression};

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
    Record(Vec<TypeExpression>),
    Alias(TypeExpression),
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Payload {
    Unit,
    Type(TypeExpression),
    Fields(Vec<TypeExpression>),
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
