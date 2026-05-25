use std::collections::HashSet;

use crate::{
    Container, DeclarationBody, Document, Field, FieldName, Name, Payload, Result, TypeExpression,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    fields: Vec<FieldLayout>,
}

impl Layout {
    pub fn for_declaration(document: &Document, declaration: &Name) -> Result<Self> {
        let body = document.declaration_body(declaration).ok_or_else(|| {
            crate::Error::MissingDeclaration {
                name: declaration.clone(),
            }
        })?;
        Ok(Self {
            fields: fields_for_body(document, body),
        })
    }

    pub fn for_variant(document: &Document, declaration: &Name, variant: &Name) -> Result<Self> {
        let variant = document.variant(declaration, variant)?;
        let fields = fields_for_payload(document, variant.payload());
        Ok(Self { fields })
    }

    pub fn fields(&self) -> &[FieldLayout] {
        &self.fields
    }

    pub fn root_positions(&self) -> Vec<usize> {
        self.positions_for(FieldLocation::Root)
    }

    pub fn box_positions(&self) -> Vec<usize> {
        self.positions_for(FieldLocation::Box)
    }

    fn positions_for(&self, location: FieldLocation) -> Vec<usize> {
        self.fields
            .iter()
            .filter(|field| field.location == location)
            .map(|field| field.position)
            .collect()
    }
}

fn fields_for_body(document: &Document, body: &DeclarationBody) -> Vec<FieldLayout> {
    match body {
        DeclarationBody::Enum { .. } => Vec::new(),
        DeclarationBody::Newtype(expression) | DeclarationBody::Alias(expression) => {
            vec![FieldLayout::new(
                0,
                expression.clone(),
                location(document, expression),
            )]
        }
        DeclarationBody::Record(fields) => fields_for_schema_fields(document, fields),
    }
}

fn fields_for_payload(document: &Document, payload: &Payload) -> Vec<FieldLayout> {
    match payload {
        Payload::Unit => Vec::new(),
        Payload::Type(expression) => vec![FieldLayout::new(
            0,
            expression.clone(),
            location(document, expression),
        )],
        Payload::Fields(fields) => fields_for_schema_fields(document, fields),
    }
}

fn fields_for_schema_fields(document: &Document, fields: &[Field]) -> Vec<FieldLayout> {
    fields
        .iter()
        .enumerate()
        .map(|(position, field)| FieldLayout::from_field(position, document, field))
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldLayout {
    position: usize,
    name: FieldName,
    expression: TypeExpression,
    location: FieldLocation,
}

impl FieldLayout {
    pub fn new(position: usize, expression: TypeExpression, location: FieldLocation) -> Self {
        Self {
            position,
            name: expression.derived_field_name(),
            expression,
            location,
        }
    }

    pub fn from_field(position: usize, document: &Document, field: &Field) -> Self {
        Self {
            position,
            name: field.name(),
            expression: field.expression().clone(),
            location: location(document, field.expression()),
        }
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn name(&self) -> &FieldName {
        &self.name
    }

    pub fn expression(&self) -> &TypeExpression {
        &self.expression
    }

    pub fn location(&self) -> FieldLocation {
        self.location
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FieldLocation {
    Root,
    Box,
}

fn location(document: &Document, expression: &TypeExpression) -> FieldLocation {
    if is_fixed_width(document, expression, &mut HashSet::new()) {
        FieldLocation::Root
    } else {
        FieldLocation::Box
    }
}

fn is_fixed_width(
    document: &Document,
    expression: &TypeExpression,
    visited: &mut HashSet<Name>,
) -> bool {
    match expression {
        TypeExpression::Primitive(primitive) => primitive.is_fixed_width(),
        TypeExpression::Container(
            Container::Vector(_) | Container::Optional(_) | Container::Map { .. },
        ) => false,
        TypeExpression::Named(name) => is_fixed_width_declaration(document, name, visited),
    }
}

fn is_fixed_width_declaration(
    document: &Document,
    name: &Name,
    visited: &mut HashSet<Name>,
) -> bool {
    if !visited.insert(name.clone()) {
        return false;
    }

    let Some(body) = document.declaration_body(name) else {
        return false;
    };

    match body {
        DeclarationBody::Enum { variants } => {
            variants.iter().all(|variant| match variant.payload() {
                Payload::Unit => true,
                Payload::Type(expression) => is_fixed_width(document, expression, visited),
                Payload::Fields(fields) => fields
                    .iter()
                    .all(|field| is_fixed_width(document, field.expression(), visited)),
            })
        }
        DeclarationBody::Newtype(expression) | DeclarationBody::Alias(expression) => {
            is_fixed_width(document, expression, visited)
        }
        DeclarationBody::Record(fields) => fields
            .iter()
            .all(|field| is_fixed_width(document, field.expression(), visited)),
    }
}
