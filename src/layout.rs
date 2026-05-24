use std::collections::HashSet;

use crate::{
    AssembledSchema, Container, DeclarationBody, Document, Name, Payload, Result, TypeExpression,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    fields: Vec<FieldLayout>,
}

impl Layout {
    /// Plan layout from a pre-assembly Document. Imported type names cannot
    /// be resolved through the Document alone, so they fall back to the
    /// conservative variable-width classification (box). For the
    /// post-import classification that lands Magnitude in root (audit 171
    /// §4.3 + §5), prefer `Layout::for_assembled`.
    pub fn for_declaration(document: &Document, declaration: &Name) -> Result<Self> {
        let body = document.declaration_body(declaration).ok_or_else(|| {
            crate::Error::MissingDeclaration {
                name: declaration.clone(),
            }
        })?;
        Ok(Self {
            fields: fields_for_body(&DocumentSource(document), body),
        })
    }

    pub fn for_variant(document: &Document, declaration: &Name, variant: &Name) -> Result<Self> {
        let variant = document.variant(declaration, variant)?;
        let fields = fields_for_payload(&DocumentSource(document), variant.payload());
        Ok(Self { fields })
    }

    /// Plan layout from a post-assembly AssembledSchema. Local bodies
    /// resolve via `AssembledSchema::body`; imported names consult
    /// `AssembledSchema::import_width` for the fixed-width hint (per audit
    /// 171 §4.3 + §5). Without a hint, an imported name defaults to
    /// variable-width (box), matching the legacy Document path's
    /// conservatism.
    pub fn for_assembled(assembled: &AssembledSchema, declaration: &Name) -> Result<Self> {
        let body = assembled
            .body(declaration)
            .ok_or_else(|| crate::Error::MissingDeclaration {
                name: declaration.clone(),
            })?;
        Ok(Self {
            fields: fields_for_body(&AssembledSource(assembled), body),
        })
    }

    /// Plan a variant's payload layout from a post-assembly AssembledSchema.
    pub fn for_assembled_variant(
        assembled: &AssembledSchema,
        declaration: &Name,
        variant_name: &Name,
    ) -> Result<Self> {
        let body = assembled
            .body(declaration)
            .ok_or_else(|| crate::Error::MissingDeclaration {
                name: declaration.clone(),
            })?;
        let DeclarationBody::Enum { variants } = body else {
            return Err(crate::Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant_name.clone(),
            });
        };
        let variant = variants
            .iter()
            .find(|candidate| candidate.name() == variant_name)
            .ok_or_else(|| crate::Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant_name.clone(),
            })?;
        let fields = fields_for_payload(&AssembledSource(assembled), variant.payload());
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

/// Source of declaration bodies + import-width hints for layout planning.
///
/// Two concrete sources: `DocumentSource` (pre-assembly) treats every name
/// without a local body as variable-width; `AssembledSource` (post-assembly)
/// consults the import-width hint table.
trait LayoutSource {
    fn declaration_body(&self, name: &Name) -> Option<&DeclarationBody>;
    fn import_width(&self, name: &Name) -> Option<bool>;
}

struct DocumentSource<'document>(&'document Document);

impl LayoutSource for DocumentSource<'_> {
    fn declaration_body(&self, name: &Name) -> Option<&DeclarationBody> {
        self.0.declaration_body(name)
    }

    fn import_width(&self, _name: &Name) -> Option<bool> {
        None
    }
}

struct AssembledSource<'assembled>(&'assembled AssembledSchema);

impl LayoutSource for AssembledSource<'_> {
    fn declaration_body(&self, name: &Name) -> Option<&DeclarationBody> {
        self.0.body(name)
    }

    fn import_width(&self, name: &Name) -> Option<bool> {
        self.0.import_width(name)
    }
}

fn fields_for_body(source: &dyn LayoutSource, body: &DeclarationBody) -> Vec<FieldLayout> {
    match body {
        DeclarationBody::Enum { .. } => Vec::new(),
        DeclarationBody::Newtype(expression) | DeclarationBody::Alias(expression) => {
            vec![FieldLayout::new(
                0,
                expression.clone(),
                location(source, expression),
            )]
        }
        DeclarationBody::Record(expressions) => fields_for_expressions(source, expressions),
    }
}

fn fields_for_payload(source: &dyn LayoutSource, payload: &Payload) -> Vec<FieldLayout> {
    match payload {
        Payload::Unit => Vec::new(),
        Payload::Type(expression) => vec![FieldLayout::new(
            0,
            expression.clone(),
            location(source, expression),
        )],
        Payload::Fields(expressions) => fields_for_expressions(source, expressions),
    }
}

fn fields_for_expressions(
    source: &dyn LayoutSource,
    expressions: &[TypeExpression],
) -> Vec<FieldLayout> {
    expressions
        .iter()
        .enumerate()
        .map(|(position, expression)| {
            FieldLayout::new(position, expression.clone(), location(source, expression))
        })
        .collect()
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

fn location(source: &dyn LayoutSource, expression: &TypeExpression) -> FieldLocation {
    if is_fixed_width(source, expression, &mut HashSet::new()) {
        FieldLocation::Root
    } else {
        FieldLocation::Box
    }
}

fn is_fixed_width(
    source: &dyn LayoutSource,
    expression: &TypeExpression,
    visited: &mut HashSet<Name>,
) -> bool {
    match expression {
        TypeExpression::Primitive(primitive) => primitive.is_fixed_width(),
        TypeExpression::Container(
            Container::Vector(_) | Container::Optional(_) | Container::Map { .. },
        ) => false,
        TypeExpression::Named(name) => is_fixed_width_declaration(source, name, visited),
    }
}

fn is_fixed_width_declaration(
    source: &dyn LayoutSource,
    name: &Name,
    visited: &mut HashSet<Name>,
) -> bool {
    if !visited.insert(name.clone()) {
        return false;
    }

    let Some(body) = source.declaration_body(name) else {
        // No local body — could be an imported type. Consult the import
        // hint table; fall back to conservative variable-width.
        return source.import_width(name).unwrap_or(false);
    };

    match body {
        DeclarationBody::Enum { variants } => {
            variants.iter().all(|variant| match variant.payload() {
                Payload::Unit => true,
                Payload::Type(expression) => is_fixed_width(source, expression, visited),
                Payload::Fields(expressions) => expressions
                    .iter()
                    .all(|expression| is_fixed_width(source, expression, visited)),
            })
        }
        DeclarationBody::Newtype(expression) | DeclarationBody::Alias(expression) => {
            is_fixed_width(source, expression, visited)
        }
        DeclarationBody::Record(expressions) => expressions
            .iter()
            .all(|expression| is_fixed_width(source, expression, visited)),
    }
}
