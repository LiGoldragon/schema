mod declarative;
mod engine;
mod identity;
mod macros;
mod module;
mod raw;
mod resolution;
mod schema;
mod source;
mod upgrade;

pub use declarative::{
    MacroDelimiter, MacroLibrary, MacroLibraryArtifact, MacroLibrarySourceEntry, MacroPattern,
    MacroPatternDelimited, MacroPatternObject, MacroTemplate, MacroTemplateDelimited,
    MacroTemplateObject, SchemaMacro, TypeTemplate,
};
pub use engine::{SchemaEngine, SchemaError, SchemaIdentity};
pub use identity::{ContentHash, FamilyClosure};
pub use macros::{
    MacroContext, MacroDispatch, MacroNodeDefinition, MacroObject, MacroOutput, MacroPair,
    MacroPosition, MacroRegistry, SchemaMacroHandler,
};
pub use module::{SchemaModuleSource, SchemaPackage};
pub use nota_next::{
    AtomCase, AtomShape, CaptureName, DelimitedShape, MacroCandidate,
    MacroDelimiter as NotaMacroDelimiter, MacroNodeDefinition as NotaMacroNodeDefinition,
    MacroObjectCount, Pattern, PatternElement, PositionPredicate, SigilPosition, SigilSpec,
};
pub use raw::{RawDatatypeEntry, RawDatatypeMap, RawNotaDatatype, RawNotaSequence, RawSchemaFile};
pub use resolution::{ImportResolver, ImportSource, ResolvedImport};
pub use schema::{
    Declaration, EnumDeclaration, EnumVariant, FieldDeclaration, ImportDeclaration, Name,
    NewtypeDeclaration, RelationDeclaration, RelationValue, Schema, SchemaDeclaredType, SchemaNode,
    SchemaNodeData, SchemaNodePair, SchemaNodeValue, StreamDeclaration, StreamRelation,
    StructDeclaration, StructFieldMap, SymbolPath, SymbolPathPosition, TypeDeclaration,
    TypeReference, Visibility,
};
pub use source::{
    SchemaSource, SchemaSourceArtifact, SourceDeclarationValue, SourceEnumBody, SourceField,
    SourceFieldValue, SourceImport, SourceImports, SourceNamespace, SourceNamespaceEntry,
    SourceReference, SourceRelation, SourceRelationValue, SourceRelations, SourceRootEnum,
    SourceStructBody, SourceVariantName, SourceVariantPayload, SourceVariantSignature,
    StreamRelationKeyword,
};
pub use upgrade::{
    AddField, AddVariant, ChangeFieldType, DefaultValue, FieldMigration, MigrationSpec, SchemaEdit,
    SchemaEditApplication, SchemaEditReceipt, UpgradeObject, UpgradeReceipt,
};
