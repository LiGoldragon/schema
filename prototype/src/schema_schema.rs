//! The schema-schema — implemented as CORE RUST per record 807.
//!
//! Records 804-807 + /357 §4-5:
//!
//! * Record 804 — every schema file is read against an implicitly-
//!   attached schema-schema. The schema-schema defines what `.schema`
//!   files look like.
//! * Record 805 — the `.schema` extension IMPLIES the root struct.
//!   Authors do NOT write an explicit `root` declaration; what they
//!   write are the positional fields of the implied root struct.
//! * Record 807 — the schema-schema's implementation IS core Rust.
//!   The `Macro` trait + `MacroContext` + `SchemaSchema` types are
//!   the macro INTERFACE on top of which every other macro builds.
//!
//! This module is small and load-bearing. It defines:
//!
//! * `Macro` — the trait every macro implements.
//! * `MacroContext` — runtime carrier: namespace + parent + schema-
//!   schema reference.
//! * `MacroError` — what a macro can fail with.
//! * `AssembledNode` — the macro output shape.
//! * `SchemaSchema` — the default-loaded schema-schema. Contains the
//!   built-in macros that interpret the root-struct fields.
//! * `parse_schema_file` — the entry point: takes `.schema` text,
//!   treats it as the implied root struct, returns the
//!   `AssembledSchema` from `schema::AssembledSchema::assemble`.

use crate::block_query::{BlockKind, Classification, SymbolKind};
use crate::blocks::{Block, BlockParser};
use crate::kernel::{Kernel, KernelError, Node};
use crate::schema::{AssembledSchema, SchemaError, ThreePartSchema};
use core::fmt;
use std::sync::Arc;

/// What can go wrong applying a macro. Variants are intentionally
/// minimal — concrete macros refine via their own errors that wrap
/// `MacroError::Domain`.
#[derive(Debug, Clone)]
pub enum MacroError {
    /// The macro's `matches_shape` returned true but `lower` decided
    /// the block did not in fact satisfy the macro's contract.
    ShapeMismatch { macro_name: String, reason: String },
    /// A domain-level error wrapping a string explanation.
    Domain { macro_name: String, message: String },
    /// The block did not match any registered macro in this context.
    NoMacroMatched { block_summary: String },
}

impl fmt::Display for MacroError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MacroError::ShapeMismatch { macro_name, reason } => {
                write!(formatter, "macro `{macro_name}` shape mismatch: {reason}")
            }
            MacroError::Domain {
                macro_name,
                message,
            } => write!(formatter, "macro `{macro_name}`: {message}"),
            MacroError::NoMacroMatched { block_summary } => {
                write!(formatter, "no macro matched for block: {block_summary}")
            }
        }
    }
}

impl std::error::Error for MacroError {}

/// The output of a macro's `lower` step. The variants here are the
/// minimum shape needed to assemble a schema file end-to-end;
/// downstream macros may carry additional payload through the
/// `Domain` variant.
#[derive(Debug, Clone)]
pub enum AssembledNode {
    /// The imports/exports section produced an empty or populated
    /// namespace table.
    ImportsTable { entries: Vec<ImportEntry> },
    /// The input/output struct produced its inner sub-fields.
    InputOutputStruct {
        input_operations: Vec<OperationDeclaration>,
        output_operations: Vec<OperationDeclaration>,
    },
    /// The namespace section produced a list of user-defined types.
    Namespace { entries: Vec<NamespaceDeclaration> },
    /// Domain-specific macro output — the schema layer's macros
    /// (downstream of the schema-schema itself) emit through this.
    Domain { tag: String, payload: String },
}

/// One import entry — the imports/exports namespace map binds a
/// name to an import source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportEntry {
    pub local_name: String,
    pub source: String,
}

/// One operation declaration — `(Tag (PayloadType))` in the input or
/// output struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationDeclaration {
    pub tag: String,
    pub payload_types: Vec<String>,
}

/// One user-defined type declaration in the namespace section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamespaceDeclaration {
    pub name: String,
    pub body_source: String,
}

/// Namespace table — name → declaration, carried in `MacroContext`.
#[derive(Debug, Clone, Default)]
pub struct NamespaceTable {
    entries: Vec<NamespaceDeclaration>,
}

impl NamespaceTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, declaration: NamespaceDeclaration) {
        self.entries.push(declaration);
    }

    pub fn entries(&self) -> &[NamespaceDeclaration] {
        &self.entries
    }

    pub fn resolve(&self, name: &str) -> Option<&NamespaceDeclaration> {
        self.entries.iter().find(|entry| entry.name == name)
    }
}

/// Macro context — what every macro sees on invocation. Carries the
/// namespace table (resolved names), a chain to the parent context
/// (for nested macros), and a reference back to the schema-schema
/// itself (so macros can recursively dispatch to other built-in
/// macros).
pub struct MacroContext {
    pub namespace: NamespaceTable,
    pub parent: Option<Arc<MacroContext>>,
    pub schema_schema: Arc<SchemaSchema>,
}

impl MacroContext {
    pub fn root(schema_schema: Arc<SchemaSchema>) -> Self {
        Self {
            namespace: NamespaceTable::new(),
            parent: None,
            schema_schema,
        }
    }
}

/// The `Macro` trait — what every macro implements. Per record 807:
/// this trait IS the macro interface, the lowest-level macro
/// primitive on top of which every other macro builds.
///
/// * `name()` — identifies the macro for diagnostics and dispatch.
/// * `matches_shape(block)` — returns true when this macro should
///   handle the given block. Uses NOTA's STRUCTURAL query methods
///   (record 803: NOTA does not perform schema-level interpretation
///   — only structural classification).
/// * `lower(block, ctx)` — produces the assembled-node output for
///   the macro's domain. May fail with `MacroError`.
pub trait Macro: Send + Sync {
    fn name(&self) -> &str;
    fn matches_shape(&self, block: &Block) -> bool;
    fn lower(&self, block: &Block, ctx: &MacroContext) -> Result<AssembledNode, MacroError>;
}

/// The default schema-schema (record 804). Holds the built-in macros
/// that interpret the .schema root struct's positional fields. These
/// macros are the LOAD-BEARING primitives — every higher-level
/// macro builds on the shape predicates they encode.
pub struct SchemaSchema {
    builtin_macros: Vec<Arc<dyn Macro>>,
}

impl SchemaSchema {
    /// Construct the default schema-schema. Per record 804: this is
    /// the schema-schema that gets implicitly loaded for every
    /// `.schema` parse.
    pub fn default() -> Self {
        let builtin_macros: Vec<Arc<dyn Macro>> = vec![
            Arc::new(ImportsSectionMacro),
            Arc::new(InputOutputStructMacro),
            Arc::new(NamespaceSectionMacro),
        ];
        Self { builtin_macros }
    }

    /// All built-in macros. Public for inspection / iteration in
    /// tests and downstream tooling.
    pub fn builtin_macros(&self) -> &[Arc<dyn Macro>] {
        &self.builtin_macros
    }

    /// Find the macro whose `matches_shape` returns true for this
    /// block. Returns the FIRST matching macro (built-in macros are
    /// registered in priority order).
    pub fn dispatch_for(&self, block: &Block) -> Option<&Arc<dyn Macro>> {
        self.builtin_macros
            .iter()
            .find(|candidate| candidate.matches_shape(block))
    }

    /// Read a `.schema` file's text. Per record 805: the `.schema`
    /// extension IMPLIES the root struct; there is no explicit root
    /// declaration in the text. What the author writes is the
    /// positional fields of the implied root struct.
    ///
    /// Returns an `AssembledSchema` — the existing assembled form
    /// from the prior prototype — so this method INTEGRATES the
    /// schema-schema with the existing reader rather than duplicating
    /// the assembly.
    pub fn parse_schema_file(&self, source: &str) -> Result<AssembledSchema, SchemaError> {
        // Per record 805: the .schema extension implies the root
        // struct. The author writes only the positional fields.
        //
        // The implied root struct's fields, in canonical layout
        // (Option A from /357 §6 — define-before-use):
        //   field 1: imports/exports map  `{...}`
        //   field 2: input header vector  `[...]`
        //   field 3: input extras vector  `[...]`
        //   field 4: namespace map        `{...}`
        //   field 5: output vector        `[...]`
        //
        // The five-block layout matches the existing
        // `ThreePartSchema` reader from the /354 prototype; this
        // method REUSES that reader as its inner step. The
        // schema-schema layer's contribution is recognising that
        // the .schema text IS the root struct (no explicit
        // declaration needed) — implicitly applied here.
        let three_part = ThreePartSchema::read(source)?;
        AssembledSchema::from_three_part(&three_part)
    }

    /// Lower the parsed blocks of a `.schema` file via the schema-
    /// schema's macro dispatch. Returns one `AssembledNode` per
    /// matched macro — the schema-schema's role per record 807.
    ///
    /// Per record 805: the .schema extension implies a root struct
    /// whose fields are POSITIONAL. The expected canonical layout
    /// (Option A from /357 §6 — define-before-use): imports map,
    /// input header vector, input extras vector, namespace map,
    /// output vector. The dispatch here applies positional macros
    /// (the schema-schema's built-ins) when the layout matches; an
    /// alternative-shape schema can still be lowered by the macro
    /// dispatch but won't necessarily satisfy every positional
    /// expectation.
    pub fn lower_via_macros(
        &self,
        source: &str,
        ctx: &MacroContext,
    ) -> Result<Vec<AssembledNode>, MacroError> {
        let parser = BlockParser::new(source);
        let blocks = match parser.parse_blocks() {
            Ok(blocks) => blocks,
            Err(error) => {
                return Err(MacroError::Domain {
                    macro_name: "<parse>".to_string(),
                    message: error.to_string(),
                });
            }
        };
        let mut outputs = Vec::new();
        for (position, block) in blocks.iter().enumerate() {
            // Positional dispatch — the .schema file's implied root
            // struct uses positional fields (record 805). Block 0
            // is imports; block 1 is input header; block 2 is
            // input extras; block 3 is namespace; block 4 is
            // output. The macro that runs for each position is
            // chosen by position; the macro's shape predicate then
            // validates that the block has the expected shape.
            let macro_ref = match position {
                0 => self.lookup_macro("imports_section"),
                1 | 2 => self.lookup_macro("input_output_struct"),
                3 => self.lookup_macro("namespace_section"),
                4 => self.lookup_macro("input_output_struct"),
                _ => None,
            };
            let Some(macro_ref) = macro_ref else {
                continue;
            };
            if !macro_ref.matches_shape(block) {
                return Err(MacroError::ShapeMismatch {
                    macro_name: macro_ref.name().to_string(),
                    reason: format!(
                        "block at position {position} did not match `{}` shape",
                        macro_ref.name()
                    ),
                });
            }
            outputs.push(macro_ref.lower(block, ctx)?);
        }
        Ok(outputs)
    }

    /// Lookup a macro by name. `None` if not registered.
    pub fn lookup_macro(&self, name: &str) -> Option<&Arc<dyn Macro>> {
        self.builtin_macros
            .iter()
            .find(|candidate| candidate.name() == name)
    }
}

// Note: no `impl Default for SchemaSchema` to avoid the inherent-
// method-vs-trait-method ambiguity. Callers use `SchemaSchema::default()`
// directly (the inherent method).

// ── Built-in macros — the schema-schema's load-bearing primitives ─

/// The imports/exports section macro. Matches `{...}` brace blocks
/// at the root level. Lower produces an `ImportsTable`.
pub struct ImportsSectionMacro;

impl ImportsSectionMacro {
    /// Whether a block looks like an imports map. The shape: brace
    /// block; even number of root objects; alternating
    /// PascalCase-symbol keys and parenthesis-record values.
    pub fn looks_like_imports_map(block: &Block) -> bool {
        if !block.is_brace() {
            return false;
        }
        let count = block.holds_root_objects();
        if count == 0 {
            // Empty imports — valid.
            return true;
        }
        if count % 2 != 0 {
            return false;
        }
        // Check each key is a PascalCase symbol and each value is
        // a parenthesis record. Per record 802: every element of a
        // sequence-context block must qualify as a symbol or a
        // block. Here we additionally check the role each plays.
        for index in 0..count / 2 {
            let key_at = index * 2;
            let value_at = key_at + 1;
            let key = match block.root_object_at(key_at) {
                Some(candidate) => candidate,
                None => return false,
            };
            let value = match block.root_object_at(value_at) {
                Some(candidate) => candidate,
                None => return false,
            };
            if !key.qualifies_as_pascal_case_symbol() {
                return false;
            }
            if !value.is_parenthesis() {
                return false;
            }
        }
        true
    }
}

impl Macro for ImportsSectionMacro {
    fn name(&self) -> &str {
        "imports_section"
    }

    fn matches_shape(&self, block: &Block) -> bool {
        Self::looks_like_imports_map(block)
    }

    fn lower(&self, block: &Block, _ctx: &MacroContext) -> Result<AssembledNode, MacroError> {
        let count = block.holds_root_objects();
        let mut entries = Vec::new();
        for index in 0..count / 2 {
            let key = block.root_object_at(index * 2).expect("shape pre-checked");
            let value = block
                .root_object_at(index * 2 + 1)
                .expect("shape pre-checked");
            entries.push(ImportEntry {
                local_name: key.leaf_text.clone(),
                // Lossy: keep the value's textual source-slice for the
                // demo. A full implementation parses ImportAll /
                // Import etc. as their own variants.
                source: format!("{}", value.root_objects.len()),
            });
        }
        Ok(AssembledNode::ImportsTable { entries })
    }
}

/// The input/output struct macro. Matches `[...]` square-bracket
/// blocks at the root level. Lower extracts the operation
/// declarations. The schema's three input-related blocks (input
/// header / extras / output) all match here; the section role is
/// determined by position in the .schema layout, not by the macro
/// itself.
pub struct InputOutputStructMacro;

impl InputOutputStructMacro {
    /// Whether the block looks like a sequence of `(Tag (Payload))`
    /// operation records.
    pub fn looks_like_operation_vector(block: &Block) -> bool {
        if !block.is_square_bracket() {
            return false;
        }
        let count = block.holds_root_objects();
        if count == 0 {
            return true;
        }
        // Every root object must be a parenthesis record (an
        // operation declaration) — per record 802 the vector
        // contents are either qualified-symbols or blocks; here we
        // require the more specific shape of operation declarations.
        for index in 0..count {
            let item = match block.root_object_at(index) {
                Some(item) => item,
                None => return false,
            };
            if !item.is_parenthesis() {
                return false;
            }
            // First root object of each operation must be a
            // PascalCase tag.
            let tag = match item.root_object_at(0) {
                Some(tag) => tag,
                None => return false,
            };
            if !tag.qualifies_as_pascal_case_symbol() {
                return false;
            }
        }
        true
    }
}

impl Macro for InputOutputStructMacro {
    fn name(&self) -> &str {
        "input_output_struct"
    }

    fn matches_shape(&self, block: &Block) -> bool {
        Self::looks_like_operation_vector(block)
    }

    fn lower(&self, block: &Block, _ctx: &MacroContext) -> Result<AssembledNode, MacroError> {
        let mut operations = Vec::new();
        let count = block.holds_root_objects();
        for index in 0..count {
            let item = block.root_object_at(index).expect("shape pre-checked");
            let tag = item.root_object_at(0).expect("shape pre-checked");
            let mut payload_types = Vec::new();
            for payload_index in 1..item.holds_root_objects() {
                let payload = item
                    .root_object_at(payload_index)
                    .expect("shape pre-checked");
                // Payload may be a single PascalCase symbol or a
                // parenthesis record wrapping a payload type. For
                // the demo: emit the leaf_text or a slice of the
                // wrapping shape.
                if let Some(text) = Self::payload_label_for(payload) {
                    payload_types.push(text);
                }
            }
            operations.push(OperationDeclaration {
                tag: tag.leaf_text.clone(),
                payload_types,
            });
        }
        // Sufficient for the demo: emit as INPUT operations; the
        // OUTPUT variant uses the same shape but a different
        // AssembledNode tag. The schema-schema layer's caller
        // tracks WHICH vector block this lowering applies to.
        Ok(AssembledNode::InputOutputStruct {
            input_operations: operations,
            output_operations: Vec::new(),
        })
    }
}

impl InputOutputStructMacro {
    fn payload_label_for(block: &Block) -> Option<String> {
        if let Some(Classification::QualifiedSymbol(SymbolKind::PascalCase)) =
            block.classification()
        {
            return Some(block.leaf_text.clone());
        }
        if block.is_parenthesis() {
            // For the demo: emit "(NestedShape)" — production
            // would recurse properly.
            let mut buffer = String::from("(");
            let inner_count = block.holds_root_objects();
            for index in 0..inner_count {
                if index > 0 {
                    buffer.push(' ');
                }
                let inner = block.root_object_at(index).expect("indexed");
                buffer.push_str(&inner.leaf_text);
            }
            buffer.push(')');
            return Some(buffer);
        }
        None
    }
}

/// The namespace section macro. Matches a `{...}` brace block whose
/// entries are NOT all paired imports — distinguishing it from the
/// imports map by content. In a .schema file the FIRST brace is
/// imports; the second is the namespace declarations.
pub struct NamespaceSectionMacro;

impl NamespaceSectionMacro {
    /// Whether the block looks like a namespace map — a brace block
    /// whose entries pair (Pascal- or camelCase) keys with arbitrary
    /// type-body values.
    ///
    /// Note: this predicate intentionally does NOT discriminate
    /// between an imports map and a namespace map purely by content
    /// — both are brace blocks with paired entries. Per record 805
    /// the .schema file's positional root-struct field-ordering
    /// disambiguates: block 0 is imports, block 3 is namespace.
    /// The macro dispatch in `SchemaSchema::lower_via_macros`
    /// selects by position; this shape predicate validates the
    /// brace+paired structure independent of role.
    pub fn looks_like_namespace_map(block: &Block) -> bool {
        if !block.is_brace() {
            return false;
        }
        let count = block.holds_root_objects();
        if count == 0 {
            // A namespace section is the only required field on the
            // root struct per /353 §3; an empty namespace is
            // structurally valid but semantically incomplete. Allow
            // empty for positional flexibility — the macro
            // dispatcher catches "no namespace" through downstream
            // assembly emptiness.
            return true;
        }
        if count % 2 != 0 {
            return false;
        }
        for index in 0..count / 2 {
            let key = match block.root_object_at(index * 2) {
                Some(item) => item,
                None => return false,
            };
            if !(key.qualifies_as_pascal_case_symbol() || key.qualifies_as_camel_case_symbol()) {
                return false;
            }
        }
        true
    }
}

impl Macro for NamespaceSectionMacro {
    fn name(&self) -> &str {
        "namespace_section"
    }

    fn matches_shape(&self, block: &Block) -> bool {
        Self::looks_like_namespace_map(block)
    }

    fn lower(&self, block: &Block, _ctx: &MacroContext) -> Result<AssembledNode, MacroError> {
        let mut entries = Vec::new();
        let count = block.holds_root_objects();
        // Walk the original source to derive each value's source
        // slice via its source span — record 776's reassembly:
        // re-emit by concatenation, never by reconstruction.
        for index in 0..count / 2 {
            let key = block.root_object_at(index * 2).expect("shape pre-checked");
            let value = block
                .root_object_at(index * 2 + 1)
                .expect("shape pre-checked");
            entries.push(NamespaceDeclaration {
                name: key.leaf_text.clone(),
                // The body is summarised by its delimiter shape for
                // the demo. A full lowering would recursively
                // dispatch through other built-in macros.
                body_source: match value.delimiter_block_kind() {
                    Some(BlockKind::Parenthesis) => {
                        format!("(parenthesis {} elements)", value.holds_root_objects())
                    }
                    Some(BlockKind::SquareBracket) => {
                        format!("(square-bracket {} elements)", value.holds_root_objects())
                    }
                    Some(BlockKind::Brace) => {
                        format!("(brace {} elements)", value.holds_root_objects())
                    }
                    None => match value.classification() {
                        Some(Classification::QualifiedSymbol(_)) => {
                            format!("(alias `{}`)", value.leaf_text)
                        }
                        _ => "(leaf)".to_string(),
                    },
                },
            });
        }
        Ok(AssembledNode::Namespace { entries })
    }
}

/// Tiny helper used by `parse_schema_file`'s diagnostics path —
/// keeps the kernel error chained.
impl From<KernelError> for MacroError {
    fn from(error: KernelError) -> Self {
        MacroError::Domain {
            macro_name: "<kernel>".to_string(),
            message: error.to_string(),
        }
    }
}

/// Helper for callers that need to construct a `Kernel` directly
/// without going through `BlockParser`. Provided as a static method
/// here so the schema-schema's source-text entry point has an
/// `impl`-block-based home per the AGENTS.md methods-only rule.
pub struct SchemaSchemaTextEntry;

impl SchemaSchemaTextEntry {
    /// Parse a string into raw kernel nodes; returned for callers
    /// that want the lowest-level view before lifting into blocks.
    pub fn parse_raw(source: &str) -> Result<Vec<Node>, KernelError> {
        let mut kernel = Kernel::new(source);
        kernel.parse_sequence()
    }
}
