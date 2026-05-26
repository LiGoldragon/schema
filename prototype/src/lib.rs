//! schema-derived-nota-prototype
//!
//! Empirical proof that the design in /353 (intent records 746-753)
//! lands in code. The crate is intentionally self-contained so the
//! bootstrap circularity stays visible:
//!
//!   1. `kernel`   — minimum hand-authored Rust to lex and parse NOTA
//!                   into tokens + a delimiter tree. This is the only
//!                   piece that is NOT schema-derived; everything else
//!                   compiles against it.
//!   2. `schema`   — reads the three-part schema layout
//!                   (Specifying / Input / Output) using the kernel.
//!   3. `emit`     — uses an assembled schema to re-emit codec rules
//!                   (bracket-string eligibility, bare-identifier
//!                   eligibility, block-form detection). This is the
//!                   "NOTA codec emitted FROM nota.schema" step.
//!   4. `macros`   — macro shape-interpretation: `{ identifier }` and
//!                   `{ k1 t1 k2 t2 ... }` (records 738, 753).
//!   5. `library`  — precompiled schema library: a core namespace
//!                   always implicitly loaded plus per-component
//!                   schemas loaded on demand (records 742, 749).
//!
//! Hard discipline:
//!   - NOTA strings come only from bracket forms; no `"..."` inside
//!     authored NOTA payloads (record 698).
//!   - Schema declares data types only (records 730-732). No
//!     EffectTable / FanOutTargets / StorageDescriptor surface.
//!   - Every function lives on an impl block; free functions are
//!     allowed only in `#[cfg(test)]` and `fn main()` (record 729).

pub mod block_query;
pub mod blocks;
pub mod emit;
pub mod kernel;
pub mod library;
pub mod macros;
pub mod schema;
pub mod schema_schema;

pub use block_query::{BlockKind, BlockReassembler, Classification, LiteralKind, SymbolKind};
pub use blocks::{Block, BlockParser, DelimiterKind, SourcePosition, SourceSpan};
pub use emit::EmittedCodec;
pub use kernel::{Kernel, KernelError, KernelToken, KernelTokenKind, Node, NodeKind};
pub use library::{Library, LibraryError};
pub use macros::{MacroEngine, MacroShape};
pub use schema::{
    AssembledSchema, NamespaceEntry, OperationEntry, SchemaError, SchemaSection, ThreePartSchema,
    TypeBody,
};
pub use schema_schema::{
    AssembledNode, ImportEntry, ImportsSectionMacro, InputOutputStructMacro, Macro, MacroContext,
    MacroError, NamespaceDeclaration, NamespaceSectionMacro, NamespaceTable, OperationDeclaration,
    SchemaSchema, SchemaSchemaTextEntry,
};
