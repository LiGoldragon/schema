mod asschema;
mod declarative;
mod engine;
mod macros;
mod module;
mod raw;
mod resolution;
mod store;
mod syntax;

pub use asschema::{
    Asschema, AsschemaArtifact, Declaration, EnumDeclaration, EnumVariant, FieldDeclaration,
    ImportDeclaration, Name, NewtypeDeclaration, SchemaNode, SchemaNodeData, SchemaNodePair,
    SchemaNodeValue, StructDeclaration, StructFieldMap, TypeDeclaration, TypeReference, Visibility,
};
pub use declarative::{
    DeclarativeMacroLibrary, MacroDelimiter, MacroLibraryArtifact, MacroLibraryData,
    MacroLibrarySourceEntry, MacroPatternData, MacroPatternDelimitedData, MacroPatternObjectData,
    MacroTemplateData, MacroTemplateDelimitedData, MacroTemplateObjectData, SchemaMacro,
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
pub use store::{AsschemaStore, AsschemaStoreKey};
pub use syntax::{
    SyntaxDatatype, SyntaxDeclaration, SyntaxEnumDeclaration, SyntaxField, SyntaxReference,
    SyntaxSchema, SyntaxStructDeclaration, SyntaxVariant,
};
