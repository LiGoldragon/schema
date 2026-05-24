use std::collections::HashSet;

use crate::{Container, DeclarationBody, Document, Name, Payload, Result, TypeExpression};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    fields: Vec<FieldLayout>,
}

impl Layout {
    pub fn for_variant(document: &Document, declaration: &Name, variant: &Name) -> Result<Self> {
        let variant = document.variant(declaration, variant)?;
        let fields = match variant.payload() {
            Payload::Unit => Vec::new(),
            Payload::Type(expression) => vec![FieldLayout::new(
                0,
                expression.clone(),
                location(document, expression),
            )],
            Payload::Fields(expressions) => expressions
                .iter()
                .enumerate()
                .map(|(position, expression)| {
                    FieldLayout::new(position, expression.clone(), location(document, expression))
                })
                .collect(),
        };
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FieldLayout {
    position: usize,
    expression: TypeExpression,
    location: FieldLocation,
}

impl FieldLayout {
    pub fn new(position: usize, expression: TypeExpression, location: FieldLocation) -> Self {
        Self {
            position,
            expression,
            location,
        }
    }

    pub fn position(&self) -> usize {
        self.position
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
        TypeExpression::Container(Container::Vector(_) | Container::Optional(_)) => false,
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

    let Some(declaration) = document.declaration(name) else {
        return false;
    };

    match declaration.body() {
        DeclarationBody::Reference(_) => false,
        DeclarationBody::Local { variants } => {
            variants.iter().all(|variant| match variant.payload() {
                Payload::Unit => true,
                Payload::Type(expression) => is_fixed_width(document, expression, visited),
                Payload::Fields(expressions) => expressions
                    .iter()
                    .all(|expression| is_fixed_width(document, expression, visited)),
            })
        }
    }
}
