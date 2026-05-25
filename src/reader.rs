use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{
    AssembledSchema, Error, ImportDirective, ImportResolution, ModuleName, Name, Result, Schema,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoadedSchema {
    path: PathBuf,
    schema: Schema,
    assembled: AssembledSchema,
}

impl LoadedSchema {
    pub fn read_path(path: impl AsRef<Path>) -> Result<Self> {
        Reader::default().read(path.as_ref())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn schema(&self) -> &Schema {
        &self.schema
    }

    pub fn assembled(&self) -> &AssembledSchema {
        &self.assembled
    }

    pub fn module(&self) -> Option<&ModuleName> {
        self.assembled.module()
    }

    fn exported_names(&self) -> Vec<Name> {
        self.assembled
            .types()
            .map(|schema_type| schema_type.name().clone())
            .collect()
    }

    fn exports(&self, name: &Name) -> bool {
        self.assembled
            .types()
            .any(|schema_type| schema_type.name() == name)
    }
}

#[derive(Default)]
struct Reader {
    visiting: BTreeSet<PathBuf>,
}

impl Reader {
    fn read(&mut self, path: &Path) -> Result<LoadedSchema> {
        let path = canonical_path(path)?;
        if !self.visiting.insert(path.clone()) {
            return Err(Error::SchemaImportCycle {
                path: path.display().to_string(),
            });
        }

        let result = self.read_without_cycle_check(&path);
        self.visiting.remove(&path);
        result
    }

    fn read_without_cycle_check(&mut self, path: &Path) -> Result<LoadedSchema> {
        let text = fs::read_to_string(path).map_err(|error| Error::SchemaReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        let schema = Schema::parse_str(&text)?;
        let resolutions = self.import_resolutions(path, &schema)?;
        let assembled = schema
            .assemble(&resolutions)?
            .with_module(ModuleName::from_schema_path(path)?);

        Ok(LoadedSchema {
            path: path.to_path_buf(),
            schema,
            assembled,
        })
    }

    fn import_resolutions(
        &mut self,
        path: &Path,
        schema: &Schema,
    ) -> Result<Vec<ImportResolution>> {
        let mut resolutions = Vec::new();
        for (binding, directive) in schema.imports().entries() {
            let imported_path = resolve_import_path(path, directive.path().as_str())?;
            let imported = self.read(&imported_path)?;
            match directive {
                ImportDirective::Import { names, .. } => {
                    for name in names {
                        if !imported.exports(name) {
                            return Err(Error::MissingImportedName {
                                binding: binding.clone(),
                                name: name.clone(),
                                path: imported.path.display().to_string(),
                            });
                        }
                    }
                }
                ImportDirective::ImportAll { .. } => {
                    resolutions.push(ImportResolution::new(
                        binding.clone(),
                        imported.exported_names(),
                    )?);
                }
            }
        }
        Ok(resolutions)
    }
}

fn canonical_path(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|error| Error::SchemaReadFailed {
            path: path.display().to_string(),
            message: error.to_string(),
        })
}

fn resolve_import_path(source_path: &Path, import: &str) -> Result<PathBuf> {
    let import_path = Path::new(import);
    let path = if import_path.is_absolute() {
        import_path.to_path_buf()
    } else {
        source_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(import_path)
    };
    canonical_path(&path)
}
