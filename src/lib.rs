//! Typed schema-language substrate for Persona signal contracts.

mod declaration;
mod document;
mod error;
mod expression;
mod layout;
mod name;

pub use declaration::{Declaration, DeclarationBody, Engine, Payload, Reference, Variant};
pub use document::Document;
pub use error::{Error, Result};
pub use expression::{Container, Primitive, TypeExpression};
pub use layout::{FieldLayout, FieldLocation, Layout};
pub use name::Name;
