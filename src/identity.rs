//! Content identity for semantic schema values.
//!
//! A schema's identity is the blake3 hash of its canonical rkyv bytes:
//! any edit to the semantic schema changes the address, and the address
//! is the version the version-control layer consumes. Two hash domains
//! exist — the whole-schema value and a per-family declaration closure —
//! and each is domain-separated through its own blake3 `derive_key`
//! context so the two kinds can never collide.
//!
//! Coverage boundaries the version-control layer must know: a family
//! closure covers what is reachable FROM the declaration — struct
//! fields, variant payloads, alias/newtype targets, collection inner
//! references, and stream relations. Relation declarations point AT
//! declarations rather than being reachable from them, so a relation
//! edit moves only the whole-schema hash, never a family hash. The
//! whole-schema hash covers the full semantic value including
//! `SchemaIdentity` (component name + authored version string) and
//! resolved imports, so it is not a pure-structure address; the family
//! hashes are.

use std::collections::BTreeMap;
use std::fmt;

use crate::{
    SchemaError,
    schema::{
        Declaration, EnumDeclaration, ImportDeclaration, Name, Schema, StreamDeclaration,
        TypeDeclaration, TypeReference,
    },
};

/// The hash domains content identity is derived under. Each domain
/// carries its own blake3 `derive_key` context string, so a
/// whole-schema hash and a family-closure hash over identical bytes
/// are structurally distinct values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HashDomain {
    Schema,
    FamilyClosure,
}

impl HashDomain {
    fn context(self) -> &'static str {
        match self {
            Self::Schema => "schema-next 2026-06-12 whole-schema content identity",
            Self::FamilyClosure => "schema-next 2026-06-12 family-closure content identity",
        }
    }
}

/// A 32-byte blake3 content address over canonical rkyv bytes.
///
/// The hash is computed over the semantic value's serialized bytes,
/// never over `.schema` source text, so formatting-only source edits
/// (whitespace, comments) do not move the address.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct ContentHash([u8; 32]);

impl ContentHash {
    fn derive(domain: HashDomain, bytes: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new_derive_key(domain.context());
        hasher.update(bytes);
        Self(*hasher.finalize().as_bytes())
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

impl fmt::Debug for ContentHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ContentHash({})", self.to_hex())
    }
}

/// The transitive declaration closure of one named record family.
///
/// The closure holds the family's root name plus every declaration
/// reachable from it through type references — struct fields, enum
/// variant payloads, newtype/alias references, `Vec`/`Map`/`Optional`/
/// `ScopeOf` element references, stream-relation stream declarations —
/// each group sorted canonically by name so the closure's bytes do not
/// depend on walk order. A reachable cross-crate import contributes its
/// stable identity (the local alias plus its `crate:module:Type`
/// source), not the dependency's own declarations.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct FamilyClosure {
    root: Name,
    declarations: Vec<Declaration>,
    imports: Vec<ImportDeclaration>,
    streams: Vec<StreamDeclaration>,
}

impl FamilyClosure {
    pub fn root(&self) -> &Name {
        &self.root
    }

    pub fn declarations(&self) -> &[Declaration] {
        &self.declarations
    }

    pub fn imports(&self) -> &[ImportDeclaration] {
        &self.imports
    }

    pub fn streams(&self) -> &[StreamDeclaration] {
        &self.streams
    }

    /// The family's content address: blake3 over the closure's
    /// canonical rkyv bytes, under the family-closure hash domain.
    pub fn content_hash(&self) -> Result<ContentHash, SchemaError> {
        let bytes =
            rkyv::to_bytes::<rkyv::rancor::Error>(self).map_err(|_| SchemaError::ArchiveEncode)?;
        Ok(ContentHash::derive(HashDomain::FamilyClosure, &bytes))
    }
}

impl Schema {
    /// The whole-schema content address: blake3 over the semantic
    /// schema value's canonical rkyv bytes, under the whole-schema
    /// hash domain. Any edit to the semantic schema moves this address.
    pub fn content_hash(&self) -> Result<ContentHash, SchemaError> {
        let bytes = self.to_binary_bytes()?;
        Ok(ContentHash::derive(HashDomain::Schema, &bytes))
    }

    /// The declaration closure of the named family. The name must be a
    /// namespace declaration or an input/output root enum of this
    /// schema; every type name reachable from it must resolve to a
    /// namespace declaration, a root enum, or a declared import.
    pub fn family_closure(&self, family_name: &str) -> Result<FamilyClosure, SchemaError> {
        ClosureWalk::new(self, family_name).into_closure()
    }
}

/// The state of one closure walk: the schema being read, the family
/// being closed over, plus the reachable members keyed by name so
/// revisits terminate and the finished closure comes out sorted
/// canonically.
struct ClosureWalk<'schema> {
    schema: &'schema Schema,
    family: &'schema str,
    declarations: BTreeMap<String, Declaration>,
    imports: BTreeMap<String, ImportDeclaration>,
    streams: BTreeMap<String, StreamDeclaration>,
}

impl<'schema> ClosureWalk<'schema> {
    fn new(schema: &'schema Schema, family: &'schema str) -> Self {
        Self {
            schema,
            family,
            declarations: BTreeMap::new(),
            imports: BTreeMap::new(),
            streams: BTreeMap::new(),
        }
    }

    fn into_closure(mut self) -> Result<FamilyClosure, SchemaError> {
        let root =
            self.family_root(self.family)
                .ok_or_else(|| SchemaError::FamilyRootNotFound {
                    name: self.family.to_owned(),
                })?;
        self.visit_declaration(root.clone())?;
        Ok(FamilyClosure {
            root: root.name().clone(),
            declarations: self.declarations.into_values().collect(),
            imports: self.imports.into_values().collect(),
            streams: self.streams.into_values().collect(),
        })
    }

    /// A family root is a namespace declaration or a root enum. A root
    /// enum enters the closure as a public enum declaration: the
    /// closure is its own value, and the root's input/output position
    /// is the version-control layer's concern, not the closure's.
    fn family_root(&self, family_name: &str) -> Option<Declaration> {
        self.namespace_declaration(family_name)
            .cloned()
            .or_else(|| {
                self.schema
                    .root_named(family_name)
                    .cloned()
                    .map(TypeDeclaration::Enum)
                    .map(Declaration::public)
            })
    }

    fn namespace_declaration(&self, name: &str) -> Option<&'schema Declaration> {
        self.schema
            .namespace()
            .iter()
            .find(|declaration| declaration.name().as_str() == name)
    }

    fn visit_declaration(&mut self, declaration: Declaration) -> Result<(), SchemaError> {
        let name = declaration.name().as_str().to_owned();
        if self.declarations.contains_key(&name) {
            return Ok(());
        }
        let value = declaration.value().clone();
        self.declarations.insert(name, declaration);
        match value {
            TypeDeclaration::Struct(body) => {
                for field in body.fields.iter() {
                    self.visit_reference(&field.reference)?;
                }
            }
            TypeDeclaration::Newtype(body) => {
                self.visit_reference(&body.reference)?;
            }
            TypeDeclaration::Enum(body) => self.visit_enum(&body)?,
        }
        Ok(())
    }

    fn visit_enum(&mut self, declaration: &EnumDeclaration) -> Result<(), SchemaError> {
        for variant in &declaration.variants {
            if let Some(payload) = &variant.payload {
                self.visit_reference(payload)?;
            }
            if let Some(relation) = &variant.stream_relation {
                self.visit_stream(relation.stream_name())?;
            }
        }
        Ok(())
    }

    fn visit_stream(&mut self, stream_name: &Name) -> Result<(), SchemaError> {
        if self.streams.contains_key(stream_name.as_str()) {
            return Ok(());
        }
        let stream = self
            .schema
            .streams()
            .iter()
            .find(|stream| &stream.name == stream_name)
            .ok_or_else(|| SchemaError::FamilyReferenceNotFound {
                family: self.family.to_owned(),
                name: stream_name.as_str().to_owned(),
            })?
            .clone();
        self.streams
            .insert(stream_name.as_str().to_owned(), stream.clone());
        self.visit_reference(&stream.token)?;
        self.visit_reference(&stream.opened)?;
        self.visit_reference(&stream.event)?;
        self.visit_reference(&stream.close)
    }

    fn visit_reference(&mut self, reference: &TypeReference) -> Result<(), SchemaError> {
        match reference {
            TypeReference::String
            | TypeReference::Integer
            | TypeReference::Boolean
            | TypeReference::Path
            | TypeReference::Bytes
            | TypeReference::FixedBytes(_) => Ok(()),
            TypeReference::Plain(name) => self.visit_name(name),
            TypeReference::Vector(inner)
            | TypeReference::Optional(inner)
            | TypeReference::ScopeOf(inner) => self.visit_reference(inner),
            TypeReference::Map(key, value) => {
                self.visit_reference(key)?;
                self.visit_reference(value)
            }
        }
    }

    fn visit_name(&mut self, name: &Name) -> Result<(), SchemaError> {
        if self.declarations.contains_key(name.as_str()) || self.imports.contains_key(name.as_str())
        {
            return Ok(());
        }
        if let Some(declaration) = self.namespace_declaration(name.as_str()) {
            return self.visit_declaration(declaration.clone());
        }
        if let Some(root) = self.schema.root_named(name.as_str()) {
            let declaration = Declaration::public(TypeDeclaration::Enum(root.clone()));
            return self.visit_declaration(declaration);
        }
        if let Some(import) = self
            .schema
            .imports()
            .iter()
            .find(|import| &import.local_name == name)
        {
            self.imports
                .insert(name.as_str().to_owned(), import.clone());
            return Ok(());
        }
        Err(SchemaError::FamilyReferenceNotFound {
            family: self.family.to_owned(),
            name: name.as_str().to_owned(),
        })
    }
}
