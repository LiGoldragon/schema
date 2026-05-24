use std::collections::HashSet;

use crate::{DeclarationBody, Error, Name, Payload, Result, Section, TypeExpression};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    sections: Vec<Section>,
}

impl Document {
    pub fn new(sections: Vec<Section>) -> Result<Self> {
        let document = Self { sections };
        document.validate()?;
        Ok(document)
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    pub fn declaration_body(&self, name: &Name) -> Option<&DeclarationBody> {
        for section in &self.sections {
            match section {
                Section::Messaging(declarations) => {
                    if let Some(declaration) = declarations
                        .iter()
                        .find(|declaration| declaration.name() == name)
                    {
                        return Some(declaration.body());
                    }
                }
                Section::Namespace(namespace) => {
                    if let Some(body) = namespace.body(name) {
                        return Some(body);
                    }
                }
            }
        }
        None
    }

    pub fn variant(&self, declaration: &Name, variant: &Name) -> Result<&crate::Variant> {
        let body = self
            .declaration_body(declaration)
            .ok_or_else(|| Error::MissingDeclaration {
                name: declaration.clone(),
            })?;
        let DeclarationBody::Local { variants } = body else {
            return Err(Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant.clone(),
            });
        };
        variants
            .iter()
            .find(|candidate| candidate.name() == variant)
            .ok_or_else(|| Error::MissingVariant {
                declaration: declaration.clone(),
                variant: variant.clone(),
            })
    }

    fn validate(&self) -> Result<()> {
        let mut declaration_names = HashSet::new();
        for (name, _) in self.declaration_entries() {
            if !declaration_names.insert(name) {
                return Err(Error::DuplicateDeclaration { name: name.clone() });
            }
        }

        for (name, body) in self.declaration_entries() {
            let DeclarationBody::Local { variants } = body else {
                continue;
            };
            let mut variant_names = HashSet::new();
            for variant in variants {
                if !variant_names.insert(variant.name()) {
                    return Err(Error::DuplicateVariant {
                        declaration: name.clone(),
                        variant: variant.name().clone(),
                    });
                }
                self.validate_payload(variant.payload())?;
            }
        }

        Ok(())
    }

    fn declaration_entries(&self) -> Vec<(&Name, &DeclarationBody)> {
        let mut entries = Vec::new();
        for section in &self.sections {
            match section {
                Section::Messaging(declarations) => {
                    for declaration in declarations {
                        entries.push((declaration.name(), declaration.body()));
                    }
                }
                Section::Namespace(namespace) => entries.extend(namespace.entries()),
            }
        }
        entries
    }

    fn validate_payload(&self, payload: &Payload) -> Result<()> {
        match payload {
            Payload::Unit => Ok(()),
            Payload::Type(expression) => self.validate_expression(expression),
            Payload::Fields(expressions) => {
                for expression in expressions {
                    self.validate_expression(expression)?;
                }
                Ok(())
            }
        }
    }

    fn validate_expression(&self, expression: &TypeExpression) -> Result<()> {
        match expression {
            TypeExpression::Primitive(_) => Ok(()),
            TypeExpression::Named(name) => {
                if self.declaration_body(name).is_some() {
                    Ok(())
                } else {
                    Err(Error::UnknownType { name: name.clone() })
                }
            }
            TypeExpression::Container(container) => match container {
                crate::Container::Vector(inner) | crate::Container::Optional(inner) => {
                    self.validate_expression(inner)
                }
            },
        }
    }
}
