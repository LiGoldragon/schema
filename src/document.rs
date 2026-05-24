use std::collections::HashSet;

use crate::{Declaration, DeclarationBody, Error, Name, Payload, Result, TypeExpression};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    declarations: Vec<Declaration>,
}

impl Document {
    pub fn new(declarations: Vec<Declaration>) -> Result<Self> {
        let document = Self { declarations };
        document.validate()?;
        Ok(document)
    }

    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    pub fn declaration(&self, name: &Name) -> Option<&Declaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.name() == name)
    }

    pub fn variant(&self, declaration: &Name, variant: &Name) -> Result<&crate::Variant> {
        let declaration =
            self.declaration(declaration)
                .ok_or_else(|| Error::MissingDeclaration {
                    name: declaration.clone(),
                })?;
        let DeclarationBody::Local { variants } = declaration.body() else {
            return Err(Error::MissingVariant {
                declaration: declaration.name().clone(),
                variant: variant.clone(),
            });
        };
        variants
            .iter()
            .find(|candidate| candidate.name() == variant)
            .ok_or_else(|| Error::MissingVariant {
                declaration: declaration.name().clone(),
                variant: variant.clone(),
            })
    }

    fn validate(&self) -> Result<()> {
        let mut declaration_names = HashSet::new();
        for declaration in &self.declarations {
            if !declaration_names.insert(declaration.name()) {
                return Err(Error::DuplicateDeclaration {
                    name: declaration.name().clone(),
                });
            }
        }

        for declaration in &self.declarations {
            let DeclarationBody::Local { variants } = declaration.body() else {
                continue;
            };
            let mut variant_names = HashSet::new();
            for variant in variants {
                if !variant_names.insert(variant.name()) {
                    return Err(Error::DuplicateVariant {
                        declaration: declaration.name().clone(),
                        variant: variant.name().clone(),
                    });
                }
                self.validate_payload(variant.payload())?;
            }
        }

        Ok(())
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
                if self.declaration(name).is_some() {
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
