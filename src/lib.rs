mod asschema;
mod declarative;
mod engine;
mod macros;
mod module;
mod raw;
mod resolution;
mod source;
mod store;
mod syntax;
mod upgrade;

pub use asschema::{
    AliasDeclaration, Asschema, AsschemaArtifact, Declaration, EnumDeclaration, EnumVariant,
    FieldDeclaration, ImportDeclaration, Name, NewtypeDeclaration, SchemaNode, SchemaNodeData,
    SchemaNodePair, SchemaNodeValue, StructDeclaration, StructFieldMap, SymbolPath,
    TypeDeclaration, TypeReference, Visibility,
};
pub use declarative::{
    MacroDelimiter, MacroLibrary, MacroLibraryArtifact, MacroLibrarySourceEntry, MacroPattern,
    MacroPatternDelimited, MacroPatternObject, MacroTemplate, MacroTemplateDelimited,
    MacroTemplateObject, SchemaMacro,
};
pub use engine::{SchemaEngine, SchemaError, SchemaIdentity, SemaDatabaseOperation};
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
pub use source::{
    SchemaSource, SchemaSourceArtifact, SourceDeclarationValue, SourceEnumBody, SourceField,
    SourceFieldValue, SourceImport, SourceImports, SourceNamespace, SourceNamespaceEntry,
    SourceReference, SourceRootEnum, SourceStructBody, SourceVariantPayload,
    SourceVariantSignature,
};
pub use store::{AsschemaStore, AsschemaStoreKey};
pub use syntax::{
    SyntaxDatatype, SyntaxDeclaration, SyntaxEnumDeclaration, SyntaxField, SyntaxReference,
    SyntaxSchema, SyntaxStructDeclaration, SyntaxVariant,
};
pub use upgrade::{
    AddField, AddVariant, AsschemaEdit, ChangeFieldType, DefaultValue, FieldMigration,
    MigrationSpec, SchemaEdit, SchemaEditReceipt, UpgradeObject, UpgradeReceipt,
};
