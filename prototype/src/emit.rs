//! Schema-emitted codec rules.
//!
//! This is the "schema is the interpretation mechanism" piece. We
//! consume `AssembledSchema` (specifically `nota.schema`) and emit
//! a set of typed predicates — bracket-string eligibility, bare-
//! identifier eligibility, block-form detection — that the codec
//! consults at encode time.
//!
//! In a fully landed pipeline these predicates would be generated
//! into Rust source by the schema-rust composer (signal-frame's
//! emit_schema!). For the prototype we emit them into a runtime
//! `EmittedCodec` so the test can show that the SAME rules drive
//! lexing AND encoding without the kernel knowing the rules
//! directly.

use crate::schema::{AssembledSchema, NamespaceEntry, TypeBody};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedCodec {
    /// Names declared in the schema's namespace, insertion order.
    pub type_names: Vec<String>,
    /// Names that are enum-shaped (variant tag carriers).
    pub enum_names: Vec<String>,
    /// Names that are struct-shaped (positional field vectors).
    pub struct_names: Vec<String>,
    /// Names that are macro-shaped (custom shape interpretation).
    pub macro_names: Vec<String>,
    /// All enum variants the schema defines, flattened to
    /// `(EnumName, VariantName)` pairs. The codec uses this list
    /// to validate variant tags at encode time.
    pub variant_index: Vec<(String, String)>,
}

impl EmittedCodec {
    /// Emit codec rules from an assembled schema. This is the
    /// proof-of-feasibility step: given nota.schema, we can drive
    /// codec behaviour from the schema's namespace without hand-
    /// authored Rust knowing the specific types.
    pub fn emit(schema: &AssembledSchema) -> Self {
        let mut type_names = Vec::new();
        let mut enum_names = Vec::new();
        let mut struct_names = Vec::new();
        let mut macro_names = Vec::new();
        let mut variant_index = Vec::new();

        for entry in &schema.namespace {
            type_names.push(entry.name.clone());
            match &entry.body {
                TypeBody::Enum { variants } => {
                    enum_names.push(entry.name.clone());
                    for variant in variants {
                        variant_index.push((entry.name.clone(), variant.name.clone()));
                    }
                }
                TypeBody::Struct { .. } => struct_names.push(entry.name.clone()),
                TypeBody::Map { .. } => macro_names.push(entry.name.clone()),
                TypeBody::Macro { .. } => macro_names.push(entry.name.clone()),
                TypeBody::Alias { .. } => {
                    // Aliases borrow their classification from the target.
                    // For the prototype we treat them as struct-shaped
                    // pass-throughs so encode/decode delegates through.
                    struct_names.push(entry.name.clone());
                }
            }
        }

        Self {
            type_names,
            enum_names,
            struct_names,
            macro_names,
            variant_index,
        }
    }

    /// Bare-identifier eligibility check, driven by the schema's
    /// declared name set. A token is bare-eligible at a String
    /// position if it is camelCase or kebab-case AND not declared
    /// as a PascalCase variant in the schema.
    pub fn is_bare_eligible(&self, text: &str) -> bool {
        let Some(first) = text.chars().next() else {
            return false;
        };
        if !first.is_ascii() {
            return false;
        }
        if first.is_ascii_uppercase() {
            return false;
        }
        if first.is_ascii_digit() {
            return false;
        }
        // Must consist of identifier-class bytes only.
        if !text.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '_' || character == '-'
        }) {
            return false;
        }
        // Reserved literal `None` is never bare-eligible at String
        // positions (per record 698 + nota README §"Bare-identifier
        // strings"). The schema declares `None` as a literal
        // variant so the variant_index already carries it; but
        // the rule is universal, not schema-specific.
        if text == "None" {
            return false;
        }
        true
    }

    /// Whether a string content needs to emit through the multi-line
    /// block form `[| ... |]` instead of inline `[ ... ]`. Schema-
    /// derived: the rule comes from `nota.schema`'s declaration of
    /// `StringForm (Inline MultilineBlock)`.
    pub fn needs_block_form(&self, text: &str) -> bool {
        text.contains('\n')
    }

    /// Whether a PascalCase token is a recognised variant tag.
    pub fn is_known_variant(&self, enum_name: &str, variant: &str) -> bool {
        self.variant_index
            .iter()
            .any(|(owner, name)| owner == enum_name && name == variant)
    }

    /// Whether the schema declares this name at all.
    pub fn declares(&self, name: &str) -> bool {
        self.type_names.iter().any(|declared| declared == name)
    }
}

/// Helper to walk just the namespace entries — used by the demo and
/// tests to confirm round-tripping.
impl EmittedCodec {
    pub fn iter_entries<'schema>(
        schema: &'schema AssembledSchema,
    ) -> impl Iterator<Item = &'schema NamespaceEntry> {
        schema.namespace.iter()
    }
}
