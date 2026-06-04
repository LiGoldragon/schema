use std::path::PathBuf;

use crate::{ImportDeclaration, Name, SchemaEngine, SchemaError, SchemaPackage};

/// A single-colon import target parsed into its three positions.
///
/// The schema author writes an import target as `crate:module:Type`
/// in the Imports brace — the same single-colon namespace shape the
/// rest of the schema stack uses (`signal:public`). `ImportSource`
/// splits that target into the crate, the module inside it, and the imported
/// type, so the resolver can find the target schema file and confirm the type
/// is declared there.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::NotaDecode,
    nota_next::NotaEncode,
    Clone,
    Debug,
    Eq,
    PartialEq,
)]
pub struct ImportSource {
    crate_name: Name,
    module: Name,
    type_name: Name,
}

impl ImportSource {
    pub fn crate_name(&self) -> &Name {
        &self.crate_name
    }

    pub fn module(&self) -> &Name {
        &self.module
    }

    pub fn type_name(&self) -> &Name {
        &self.type_name
    }

    /// The Rust module path the consumer reaches the imported type
    /// through: `<crate_identifier>::schema::<module>`. The crate
    /// identifier is the crate name with hyphens turned into
    /// underscores (Cargo's lib-name normalisation); the module
    /// segments keep the single-colon translation as `::`. Generated
    /// support types the importing module also re-declares (the
    /// per-module `NotaDecodeError`) live here, so the emitter can
    /// bridge them across the crate boundary.
    pub fn module_path(&self) -> String {
        let crate_identifier = self.crate_name.as_str().replace('-', "_");
        let module_path = self.module.as_str().replace('-', "_").replace(':', "::");
        format!("{crate_identifier}::schema::{module_path}")
    }

    /// The Rust path the consumer references to reach the imported
    /// type: `<module_path>::<Type>`.
    pub fn rust_path(&self) -> String {
        format!("{}::{}", self.module_path(), self.type_name.local_part())
    }
}

impl TryFrom<&Name> for ImportSource {
    type Error = SchemaError;

    fn try_from(name: &Name) -> Result<Self, Self::Error> {
        let segments = name.namespace_segments();
        if segments.len() < 3 {
            return Err(SchemaError::MalformedImportSource {
                found: name.as_str().to_owned(),
            });
        }
        let crate_name = Name::new(segments[0]);
        let module = Name::new(segments[1..segments.len() - 1].join(":"));
        let type_name = Name::new(segments[segments.len() - 1]);
        Ok(Self {
            crate_name,
            module,
            type_name,
        })
    }
}

/// An import declaration resolved against a package module schema.
///
/// Resolution confirms the module schema declares the imported name as either
/// an input/output root enum or a namespace type, then carries the local alias
/// plus the parsed source so the Rust emitter can write a `use` aliasing the
/// emitted type to the local name — instead of re-declaring the type.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    nota_next::NotaDecode,
    nota_next::NotaEncode,
    Clone,
    Debug,
    Eq,
    PartialEq,
)]
pub struct ResolvedImport {
    local_name: Name,
    source: ImportSource,
}

impl ResolvedImport {
    pub fn local_name(&self) -> &Name {
        &self.local_name
    }

    pub fn source(&self) -> &ImportSource {
        &self.source
    }

    /// The Rust module path the imported type lives under in the
    /// dependency crate (`<crate>::schema::<module>`). The emitter
    /// bridges each distinct module's generated support types (the
    /// per-module `NotaDecodeError`) across the crate boundary.
    pub fn module_path(&self) -> String {
        self.source.module_path()
    }

    /// The Rust `use` item the consumer emits to reach the imported
    /// type under its local alias: `use <rust_path> as <LocalName>;`.
    pub fn use_item(&self) -> String {
        format!(
            "pub use {} as {};",
            self.source.rust_path(),
            self.local_name.local_part()
        )
    }
}

/// Maps crate names to schema packages that can satisfy imports.
///
/// The consumer's build script reads each dependency's
/// `DEP_<CRATE>_SCHEMA_DIR` environment variable (set by Cargo for any
/// `links`-declaring direct dependency) and registers the crate name against
/// that directory here. Package lowering also registers the current crate so
/// sibling schema files can import each other. During lowering the engine asks
/// the resolver to turn each `ImportDeclaration` into a `ResolvedImport`,
/// loading the target schema file to confirm the imported root or namespace
/// type is actually declared there.
#[derive(Clone, Debug, Default)]
pub struct ImportResolver {
    packages: Vec<SchemaPackage>,
}

impl ImportResolver {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a package that can satisfy imports. Package self-registration
    /// lets `schema/nexus.schema` import `schema/signal.schema` inside the same
    /// daemon crate without pretending each plane is a separate crate.
    pub fn with_package(mut self, package: SchemaPackage) -> Self {
        self.packages.push(package);
        self
    }

    /// Register a dependency crate's schema directory. `schema_dir`
    /// is the directory Cargo exposed through `DEP_<CRATE>_SCHEMA_DIR`
    /// — the dependency's `schema/` folder. The package is rooted at
    /// the directory's parent so `SchemaPackage`'s `schema/` join
    /// lands back on `schema_dir`.
    pub fn with_dependency(
        self,
        crate_name: impl Into<String>,
        schema_dir: impl Into<PathBuf>,
        version: impl Into<String>,
    ) -> Self {
        let schema_dir = schema_dir.into();
        let root = schema_dir
            .parent()
            .map(PathBuf::from)
            .unwrap_or_else(|| schema_dir.clone());
        self.with_package(SchemaPackage::new(root, crate_name, version))
    }

    fn package_for(&self, crate_name: &Name) -> Result<&SchemaPackage, SchemaError> {
        self.packages
            .iter()
            .find(|package| package.crate_name() == crate_name)
            .ok_or_else(|| SchemaError::UnresolvedImportCrate {
                crate_name: crate_name.as_str().to_owned(),
            })
    }

    pub fn resolve(
        &self,
        declaration: &ImportDeclaration,
        engine: &SchemaEngine,
    ) -> Result<ResolvedImport, SchemaError> {
        let source_name =
            declaration
                .source
                .plain_name()
                .ok_or_else(|| SchemaError::MalformedImportSource {
                    found: "collection import source".to_owned(),
                })?;
        let source = ImportSource::try_from(source_name)?;
        let package = self.package_for(source.crate_name())?;
        let module_source = package.load_module(source.module().clone())?;
        let module_schema = module_source.lower_with_resolver(engine, self)?;
        if module_schema
            .declared_type_named(source.type_name().local_part())
            .is_none()
        {
            return Err(SchemaError::ImportedTypeNotFound {
                crate_name: source.crate_name().as_str().to_owned(),
                module: source.module().as_str().to_owned(),
                type_name: source.type_name().local_part().to_owned(),
            });
        }
        Ok(ResolvedImport {
            local_name: declaration.local_name.clone(),
            source,
        })
    }

    pub fn resolve_all(
        &self,
        declarations: &[ImportDeclaration],
        engine: &SchemaEngine,
    ) -> Result<Vec<ResolvedImport>, SchemaError> {
        declarations
            .iter()
            .map(|declaration| self.resolve(declaration, engine))
            .collect()
    }
}
