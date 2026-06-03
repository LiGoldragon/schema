use crate::{
    Asschema, Declaration, EnumDeclaration, EnumVariant, FieldDeclaration, Name, SchemaError,
    SchemaIdentity, StructDeclaration, TypeDeclaration, TypeReference,
};

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
pub enum SchemaEdit {
    AddField(AddField),
    ChangeFieldType(ChangeFieldType),
    AddVariant(AddVariant),
}

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
pub struct AddField {
    pub target_type: Name,
    pub field_name: Name,
    pub field_type: TypeReference,
    pub default_value: DefaultValue,
}

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
pub struct ChangeFieldType {
    pub target_type: Name,
    pub field_name: Name,
    pub new_type: TypeReference,
    pub migration: FieldMigration,
}

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
pub struct AddVariant {
    pub target_type: Name,
    pub variant_name: Name,
    pub payload: Option<TypeReference>,
}

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
pub enum FieldMigration {
    WrapSingleton,
    SetDefault(DefaultValue),
}

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
pub enum DefaultValue {
    String(String),
    Integer(u64),
    Boolean(bool),
    Unit,
}

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
pub struct MigrationSpec {
    pub target_type: Name,
    pub field_name: Name,
    pub previous_type: Option<TypeReference>,
    pub next_type: TypeReference,
    pub migration: FieldMigration,
}

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
pub struct SchemaEditReceipt {
    pub schema_identity: SchemaIdentity,
    pub migration_spec: Option<MigrationSpec>,
}

pub struct AsschemaEdit {
    asschema: Asschema,
    edit: SchemaEdit,
}

impl SchemaEdit {
    pub fn add_field(
        target_type: impl Into<String>,
        field_name: impl Into<String>,
        field_type: TypeReference,
        default_value: DefaultValue,
    ) -> Self {
        Self::AddField(AddField {
            target_type: Name::new(target_type),
            field_name: Name::new(field_name),
            field_type,
            default_value,
        })
    }

    pub fn change_field_type(
        target_type: impl Into<String>,
        field_name: impl Into<String>,
        new_type: TypeReference,
        migration: FieldMigration,
    ) -> Self {
        Self::ChangeFieldType(ChangeFieldType {
            target_type: Name::new(target_type),
            field_name: Name::new(field_name),
            new_type,
            migration,
        })
    }

    pub fn add_variant(
        target_type: impl Into<String>,
        variant_name: impl Into<String>,
        payload: Option<TypeReference>,
    ) -> Self {
        Self::AddVariant(AddVariant {
            target_type: Name::new(target_type),
            variant_name: Name::new(variant_name),
            payload,
        })
    }

    pub fn apply_to(
        self,
        asschema: Asschema,
    ) -> Result<(Asschema, SchemaEditReceipt), SchemaError> {
        AsschemaEdit::new(asschema, self).apply()
    }
}

impl AsschemaEdit {
    pub fn new(asschema: Asschema, edit: SchemaEdit) -> Self {
        Self { asschema, edit }
    }

    pub fn apply(self) -> Result<(Asschema, SchemaEditReceipt), SchemaError> {
        let Self { asschema, edit } = self;
        match edit {
            SchemaEdit::AddField(operation) => Self::apply_add_field(asschema, operation),
            SchemaEdit::ChangeFieldType(operation) => {
                Self::apply_change_field_type(asschema, operation)
            }
            SchemaEdit::AddVariant(operation) => Self::apply_add_variant(asschema, operation),
        }
    }

    fn apply_add_field(
        asschema: Asschema,
        edit: AddField,
    ) -> Result<(Asschema, SchemaEditReceipt), SchemaError> {
        let field_type = edit.field_type.clone();
        let migration = FieldMigration::SetDefault(edit.default_value);
        let (asschema, previous_type) = AsschemaEditor::new(asschema).update_struct(
            edit.target_type.clone(),
            |declaration| {
                if declaration
                    .fields
                    .iter()
                    .any(|field| field.name == edit.field_name)
                {
                    return Err(SchemaError::SchemaEditDuplicateField {
                        type_name: edit.target_type.to_string(),
                        field_name: edit.field_name.to_string(),
                    });
                }
                let mut fields = declaration.fields.entries().to_vec();
                fields.push(FieldDeclaration {
                    name: edit.field_name.clone(),
                    reference: field_type.clone(),
                });
                Ok((
                    StructDeclaration::new(declaration.name.clone(), fields),
                    None,
                ))
            },
        )?;
        let receipt = asschema.edit_receipt(Some(MigrationSpec {
            target_type: edit.target_type,
            field_name: edit.field_name,
            previous_type,
            next_type: field_type,
            migration,
        }));
        Ok((asschema, receipt))
    }

    fn apply_change_field_type(
        asschema: Asschema,
        edit: ChangeFieldType,
    ) -> Result<(Asschema, SchemaEditReceipt), SchemaError> {
        let next_type = edit.new_type.clone();
        let (asschema, previous_type) = AsschemaEditor::new(asschema).update_struct(
            edit.target_type.clone(),
            |declaration| {
                let mut fields = declaration.fields.entries().to_vec();
                let Some(field) = fields
                    .iter_mut()
                    .find(|field| field.name == edit.field_name)
                else {
                    return Err(SchemaError::SchemaEditFieldNotFound {
                        type_name: edit.target_type.to_string(),
                        field_name: edit.field_name.to_string(),
                    });
                };
                let previous_type = field.reference.clone();
                field.reference = next_type.clone();
                Ok((
                    StructDeclaration::new(declaration.name.clone(), fields),
                    Some(previous_type),
                ))
            },
        )?;
        let receipt = asschema.edit_receipt(Some(MigrationSpec {
            target_type: edit.target_type,
            field_name: edit.field_name,
            previous_type,
            next_type,
            migration: edit.migration,
        }));
        Ok((asschema, receipt))
    }

    fn apply_add_variant(
        asschema: Asschema,
        edit: AddVariant,
    ) -> Result<(Asschema, SchemaEditReceipt), SchemaError> {
        let asschema =
            AsschemaEditor::new(asschema).update_enum(edit.target_type.clone(), |declaration| {
                if declaration
                    .variants
                    .iter()
                    .any(|variant| variant.name == edit.variant_name)
                {
                    return Err(SchemaError::SchemaEditDuplicateVariant {
                        type_name: edit.target_type.to_string(),
                        variant_name: edit.variant_name.to_string(),
                    });
                }
                let mut variants = declaration.variants.clone();
                variants.push(EnumVariant {
                    name: edit.variant_name.clone(),
                    payload: edit.payload.clone(),
                });
                Ok(EnumDeclaration::new(declaration.name.clone(), variants))
            })?;
        let receipt = asschema.edit_receipt(None);
        Ok((asschema, receipt))
    }
}

struct AsschemaEditor {
    identity: SchemaIdentity,
    imports: Vec<crate::ImportDeclaration>,
    resolved_imports: Vec<crate::ResolvedImport>,
    input: crate::EnumDeclaration,
    output: crate::EnumDeclaration,
    namespace: Vec<Declaration>,
}

impl AsschemaEditor {
    fn new(asschema: Asschema) -> Self {
        Self {
            identity: asschema.identity().clone(),
            imports: asschema.imports().to_vec(),
            resolved_imports: asschema.resolved_imports().to_vec(),
            input: asschema.input().clone(),
            output: asschema.output().clone(),
            namespace: asschema.namespace().to_vec(),
        }
    }

    fn update_struct(
        mut self,
        target_type: Name,
        update: impl FnOnce(
            &StructDeclaration,
        ) -> Result<(StructDeclaration, Option<TypeReference>), SchemaError>,
    ) -> Result<(Asschema, Option<TypeReference>), SchemaError> {
        let Some(index) = self
            .namespace
            .iter()
            .position(|declaration| declaration.name() == &target_type)
        else {
            return Err(SchemaError::SchemaEditTargetNotFound {
                type_name: target_type.to_string(),
            });
        };
        let visibility = self.namespace[index].visibility();
        let TypeDeclaration::Struct(declaration) = self.namespace[index].value() else {
            return Err(SchemaError::SchemaEditExpectedStruct {
                type_name: target_type.to_string(),
            });
        };
        let (declaration, previous_type) = update(declaration)?;
        self.namespace[index] = match visibility {
            crate::Visibility::Public => Declaration::public(TypeDeclaration::Struct(declaration)),
            crate::Visibility::Private => {
                Declaration::private(TypeDeclaration::Struct(declaration))
            }
        };
        Ok((self.into_asschema(), previous_type))
    }

    fn update_enum(
        mut self,
        target_type: Name,
        update: impl FnOnce(&EnumDeclaration) -> Result<EnumDeclaration, SchemaError>,
    ) -> Result<Asschema, SchemaError> {
        let Some(index) = self
            .namespace
            .iter()
            .position(|declaration| declaration.name() == &target_type)
        else {
            return Err(SchemaError::SchemaEditTargetNotFound {
                type_name: target_type.to_string(),
            });
        };
        let visibility = self.namespace[index].visibility();
        let TypeDeclaration::Enum(declaration) = self.namespace[index].value() else {
            return Err(SchemaError::SchemaEditExpectedEnum {
                type_name: target_type.to_string(),
            });
        };
        let declaration = update(declaration)?;
        self.namespace[index] = match visibility {
            crate::Visibility::Public => Declaration::public(TypeDeclaration::Enum(declaration)),
            crate::Visibility::Private => Declaration::private(TypeDeclaration::Enum(declaration)),
        };
        Ok(self.into_asschema())
    }

    fn into_asschema(self) -> Asschema {
        Asschema::new(
            self.identity,
            self.imports,
            self.resolved_imports,
            self.input,
            self.output,
            self.namespace,
        )
    }
}

impl Asschema {
    fn edit_receipt(&self, migration_spec: Option<MigrationSpec>) -> SchemaEditReceipt {
        SchemaEditReceipt {
            schema_identity: self.identity().clone(),
            migration_spec,
        }
    }
}

/// A complete schema upgrade — the durable record the schema daemon stores
/// in SEMA and the schema-rust-next emitter reads to produce migration
/// code. The wrapper holds the previous/next identity pair (mints the
/// version bump explicitly) and the ordered list of `SchemaEdit`
/// operations.
///
/// Per designer 447 §"Block 1": this is the typed object an
/// `UpgradeSchema(UpgradeObject)` signal payload carries. Applying it to
/// the stored asschema returns the new asschema + the receipts every
/// operation produced, in the order applied.
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
pub struct UpgradeObject {
    pub previous_identity: SchemaIdentity,
    pub next_identity: SchemaIdentity,
    pub edits: Vec<SchemaEdit>,
}

impl UpgradeObject {
    pub fn new(
        previous_identity: SchemaIdentity,
        next_identity: SchemaIdentity,
        edits: Vec<SchemaEdit>,
    ) -> Self {
        Self {
            previous_identity,
            next_identity,
            edits,
        }
    }

    pub fn previous_identity(&self) -> &SchemaIdentity {
        &self.previous_identity
    }

    pub fn next_identity(&self) -> &SchemaIdentity {
        &self.next_identity
    }

    pub fn edits(&self) -> &[SchemaEdit] {
        &self.edits
    }

    /// Apply every edit in order against `previous`, returning the new
    /// asschema stamped with `next_identity` and the receipts every edit
    /// produced.
    ///
    /// Identity mismatch is a typed failure — if `previous.identity()` is
    /// not equal to `self.previous_identity`, the upgrade is rejected
    /// rather than applied against a schema it was not authored against.
    pub fn apply(&self, previous: &Asschema) -> Result<(Asschema, UpgradeReceipt), SchemaError> {
        if previous.identity() != &self.previous_identity {
            return Err(SchemaError::SchemaEditIdentityMismatch {
                expected: format!(
                    "{}@{}",
                    self.previous_identity.component().as_str(),
                    self.previous_identity.version()
                ),
                found: format!(
                    "{}@{}",
                    previous.identity().component().as_str(),
                    previous.identity().version()
                ),
            });
        }
        let mut asschema = previous.clone();
        let mut edit_receipts = Vec::with_capacity(self.edits.len());
        for edit in &self.edits {
            let (next, receipt) = AsschemaEdit::new(asschema, edit.clone()).apply()?;
            asschema = next;
            edit_receipts.push(receipt);
        }
        let asschema = asschema.with_identity(self.next_identity.clone());
        let upgrade_receipt = UpgradeReceipt {
            previous_identity: self.previous_identity.clone(),
            next_identity: self.next_identity.clone(),
            edit_receipts,
        };
        Ok((asschema, upgrade_receipt))
    }
}

/// The aggregated receipt the schema daemon records when an `UpgradeObject`
/// applies. Carries the identity transition plus each edit's per-edit
/// receipt for later emission and audit.
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
pub struct UpgradeReceipt {
    pub previous_identity: SchemaIdentity,
    pub next_identity: SchemaIdentity,
    pub edit_receipts: Vec<SchemaEditReceipt>,
}

impl UpgradeReceipt {
    pub fn previous_identity(&self) -> &SchemaIdentity {
        &self.previous_identity
    }

    pub fn next_identity(&self) -> &SchemaIdentity {
        &self.next_identity
    }

    pub fn edit_receipts(&self) -> &[SchemaEditReceipt] {
        &self.edit_receipts
    }
}

impl Asschema {
    /// Replace this asschema's identity with a new version stamp without
    /// changing its declarations. `UpgradeObject::apply` calls this once
    /// at the end of applying every edit, so the stored asschema records
    /// the new version.
    pub fn with_identity(self, identity: SchemaIdentity) -> Self {
        let imports = self.imports().to_vec();
        let resolved_imports = self.resolved_imports().to_vec();
        let input = self.input().clone();
        let output = self.output().clone();
        let namespace = self.namespace().to_vec();
        Self::new(
            identity,
            imports,
            resolved_imports,
            input,
            output,
            namespace,
        )
    }
}
