use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Asschema, Name, SchemaEngine, SchemaError, SchemaIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaPackage {
    root: PathBuf,
    crate_name: Name,
    version: String,
}

impl SchemaPackage {
    pub fn new(
        root: impl Into<PathBuf>,
        crate_name: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        Self {
            root: root.into(),
            crate_name: Name::new(crate_name),
            version: version.into(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn crate_name(&self) -> &Name {
        &self.crate_name
    }

    pub fn schema_directory(&self) -> PathBuf {
        self.root.join("schema")
    }

    pub fn lib_schema_path(&self) -> PathBuf {
        self.schema_directory().join("lib.schema")
    }

    pub fn module_schema_path(&self, module: &Name) -> PathBuf {
        self.schema_directory()
            .join(format!("{}.schema", module.as_str().replace(':', "/")))
    }

    pub fn load_lib(&self) -> Result<SchemaModuleSource, SchemaError> {
        self.load_path(Name::new("lib"), self.lib_schema_path())
    }

    pub fn load_module(&self, module: Name) -> Result<SchemaModuleSource, SchemaError> {
        self.load_path(module.clone(), self.module_schema_path(&module))
    }

    pub fn lower_lib(&self, engine: &SchemaEngine) -> Result<Asschema, SchemaError> {
        self.load_lib()?.lower(engine)
    }

    fn load_path(
        &self,
        module_name: Name,
        path: impl Into<PathBuf>,
    ) -> Result<SchemaModuleSource, SchemaError> {
        let path = path.into();
        let source = fs::read_to_string(&path).map_err(|error| SchemaError::Io {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
        Ok(SchemaModuleSource {
            identity: SchemaIdentity::new(
                format!("{}:{}", self.crate_name, module_name),
                self.version.clone(),
            ),
            path,
            source,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemaModuleSource {
    identity: SchemaIdentity,
    path: PathBuf,
    source: String,
}

impl SchemaModuleSource {
    pub fn identity(&self) -> &SchemaIdentity {
        &self.identity
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn lower(&self, engine: &SchemaEngine) -> Result<Asschema, SchemaError> {
        engine.lower_source(&self.source, self.identity.clone())
    }
}
