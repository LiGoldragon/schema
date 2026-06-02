use std::{
    fs,
    path::{Path, PathBuf},
};

use redb::{Database, ReadableTableMetadata, TableDefinition};

use crate::{
    Asschema, AsschemaArtifact, SchemaEdit, SchemaEditReceipt, SchemaError, SchemaIdentity,
    engine::SemaDatabaseOperation,
};

const ASSEMBLED_SCHEMAS: TableDefinition<&str, &[u8]> = TableDefinition::new("assembled-schemas");

#[derive(Debug)]
pub struct AsschemaStore {
    database: Database,
    path: PathBuf,
}

impl AsschemaStore {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self, SchemaError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| SchemaError::Io {
                path: parent.display().to_string(),
                reason: error.to_string(),
            })?;
        }
        let database = if path.exists() {
            Database::open(&path)
        } else {
            Database::create(&path)
        }
        .map_err(|error| SchemaError::SemaDatabase {
            operation: SemaDatabaseOperation::Open,
            reason: error.to_string(),
        })?;
        let store = Self { database, path };
        store.ensure_tables()?;
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn put_asschema(&self, asschema: &Asschema) -> Result<(), SchemaError> {
        let key = AsschemaStoreKey::from_identity(asschema.identity());
        let bytes = asschema.to_binary_bytes()?;
        self.put_binary_bytes(&key, bytes.as_slice())
    }

    pub fn put_artifact(&self, artifact: &AsschemaArtifact) -> Result<(), SchemaError> {
        let key = AsschemaStoreKey::from_identity(artifact.asschema().identity());
        let bytes = artifact.to_binary_bytes()?;
        self.put_binary_bytes(&key, bytes.as_slice())
    }

    fn put_binary_bytes(&self, key: &AsschemaStoreKey, bytes: &[u8]) -> Result<(), SchemaError> {
        let transaction =
            self.database
                .begin_write()
                .map_err(|error| SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::BeginWrite,
                    reason: error.to_string(),
                })?;
        {
            let mut table = transaction.open_table(ASSEMBLED_SCHEMAS).map_err(|error| {
                SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::OpenTable,
                    reason: error.to_string(),
                }
            })?;
            table
                .insert(key.as_str(), bytes)
                .map_err(|error| SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::Write,
                    reason: error.to_string(),
                })?;
        }
        transaction
            .commit()
            .map_err(|error| SchemaError::SemaDatabase {
                operation: SemaDatabaseOperation::Commit,
                reason: error.to_string(),
            })
    }

    pub fn get_artifact(
        &self,
        identity: &SchemaIdentity,
    ) -> Result<Option<AsschemaArtifact>, SchemaError> {
        let key = AsschemaStoreKey::from_identity(identity);
        let transaction =
            self.database
                .begin_read()
                .map_err(|error| SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::BeginRead,
                    reason: error.to_string(),
                })?;
        let table = transaction.open_table(ASSEMBLED_SCHEMAS).map_err(|error| {
            SchemaError::SemaDatabase {
                operation: SemaDatabaseOperation::OpenTable,
                reason: error.to_string(),
            }
        })?;
        let Some(bytes) = table
            .get(key.as_str())
            .map_err(|error| SchemaError::SemaDatabase {
                operation: SemaDatabaseOperation::Read,
                reason: error.to_string(),
            })?
        else {
            return Ok(None);
        };
        AsschemaArtifact::from_binary_bytes(bytes.value()).map(Some)
    }

    pub fn get_asschema(&self, identity: &SchemaIdentity) -> Result<Option<Asschema>, SchemaError> {
        self.get_artifact(identity)
            .map(|artifact| artifact.map(AsschemaArtifact::into_asschema))
    }

    pub fn apply_edit(
        &self,
        identity: &SchemaIdentity,
        edit: SchemaEdit,
    ) -> Result<SchemaEditReceipt, SchemaError> {
        let key = AsschemaStoreKey::from_identity(identity);
        let asschema = self
            .get_asschema(identity)?
            .ok_or_else(|| SchemaError::MissingAsschema {
                key: key.as_str().to_owned(),
            })?;
        let (updated, receipt) = edit.apply_to(asschema)?;
        self.put_asschema(&updated)?;
        Ok(receipt)
    }

    pub fn export_nota_file(
        &self,
        identity: &SchemaIdentity,
        path: impl AsRef<Path>,
    ) -> Result<(), SchemaError> {
        let key = AsschemaStoreKey::from_identity(identity);
        let artifact =
            self.get_artifact(identity)?
                .ok_or_else(|| SchemaError::MissingAsschema {
                    key: key.as_str().to_owned(),
                })?;
        artifact.write_nota_file(path)
    }

    pub fn len(&self) -> Result<u64, SchemaError> {
        let transaction =
            self.database
                .begin_read()
                .map_err(|error| SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::BeginRead,
                    reason: error.to_string(),
                })?;
        let table = transaction.open_table(ASSEMBLED_SCHEMAS).map_err(|error| {
            SchemaError::SemaDatabase {
                operation: SemaDatabaseOperation::OpenTable,
                reason: error.to_string(),
            }
        })?;
        table.len().map_err(|error| SchemaError::SemaDatabase {
            operation: SemaDatabaseOperation::Read,
            reason: error.to_string(),
        })
    }

    pub fn is_empty(&self) -> Result<bool, SchemaError> {
        self.len().map(|length| length == 0)
    }

    fn ensure_tables(&self) -> Result<(), SchemaError> {
        let transaction =
            self.database
                .begin_write()
                .map_err(|error| SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::BeginWrite,
                    reason: error.to_string(),
                })?;
        {
            transaction.open_table(ASSEMBLED_SCHEMAS).map_err(|error| {
                SchemaError::SemaDatabase {
                    operation: SemaDatabaseOperation::OpenTable,
                    reason: error.to_string(),
                }
            })?;
        }
        transaction
            .commit()
            .map_err(|error| SchemaError::SemaDatabase {
                operation: SemaDatabaseOperation::Commit,
                reason: error.to_string(),
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AsschemaStoreKey {
    value: String,
}

impl AsschemaStoreKey {
    pub fn from_identity(identity: &SchemaIdentity) -> Self {
        Self {
            value: format!("{}@{}", identity.component().as_str(), identity.version()),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }
}
