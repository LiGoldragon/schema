//! Typed schema-language substrate for Persona signal contracts.

mod assembled;
mod declaration;
mod document;
mod engine;
mod error;
mod expression;
mod feature;
mod header;
mod import;
mod layout;
mod name;
mod parser;
mod reader;
mod section;
mod upgrade;

pub use assembled::{AssembledSchema, AssembledType, Endpoint, Leg, Route, RouteBody};
pub use declaration::{Declaration, DeclarationBody, Engine, Payload, Variant};
pub use document::{Document, Schema};
pub use engine::{
    AssembledFragment, BuiltinMacroVariant, FeatureInput, HeaderEndpointInput, HeaderInput,
    ImportInput, LoweringContext, NodeDefinitionPoint, SchemaMacro, TypeInput, UpgradeRuleInput,
};
pub use error::{Error, Result};
pub use expression::{Container, Primitive, TypeExpression};
pub use feature::{EventFeature, Feature, ObservableFeature};
pub use header::{Header, HeaderRoot};
pub use import::{
    ImportBinding, ImportDirective, ImportResolution, ImportedNames, Imports, SchemaPath,
};
pub use layout::{FieldLayout, FieldLocation, Layout};
pub use name::Name;
pub use reader::LoadedSchema;
pub use section::Namespace;
pub use upgrade::{
    Projection, StandardProjection, Upgrade, UpgradeAnnotation, UpgradePlan, Version,
};
