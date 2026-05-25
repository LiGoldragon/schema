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
pub mod multi_pass;
mod name;
mod node_shape;
mod parser;
mod reader;
mod section;
mod shape_parser;
mod upgrade;

pub use assembled::{AssembledSchema, AssembledType, Endpoint, Leg, Route, RouteBody};
pub use declaration::{Declaration, DeclarationBody, Engine, Field, Payload, Variant};
pub use document::{Document, Schema};
pub use engine::{
    AssembledFragment, BuiltinMacroVariant, FeatureInput, HeaderEndpointInput, HeaderInput,
    ImportInput, LoweringContext, NodeDefinitionPoint, SchemaMacro, TypeInput, UniversalUnknownMacro,
    UpgradeRuleInput,
};
pub use error::{Error, Result};
pub use expression::{Container, Primitive, TypeExpression};
pub use feature::{
    EffectTableEntry, EffectTableFeature, EventFeature, FanOutOutputDeclaration, FanOutTargetsEntry,
    FanOutTargetsFeature, Feature, ObservableFeature, StorageDescriptorEntry,
    StorageDescriptorFeature,
};
pub use header::{Header, HeaderRoot};
pub use import::{
    ImportBinding, ImportDirective, ImportResolution, ImportedNames, Imports, SchemaPath,
};
pub use layout::{FieldLayout, FieldLocation, Layout};
pub use name::{FieldName, ModuleName, Name, QualifiedName};
pub use node_shape::{NamespaceValueShape, NodeDefinitionShape};
pub use reader::LoadedSchema;
pub use section::Namespace;
pub use upgrade::{
    Projection, StandardProjection, Upgrade, UpgradeAnnotation, UpgradePlan, Version,
};
