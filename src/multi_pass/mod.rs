//! Multi-pass NOTA-first schema reader — proof of concept per
//! `reports/designer/334-multi-pass-nota-first-schema-reader.md`.
//!
//! Six numbered passes from text to canonical `AssembledSchema`. Each
//! pass has ONE responsibility and only the typed artifact crosses pass
//! boundaries.
//!
//! - Pass 0: lexical (text -> tokens). Uses `nota_codec::Lexer`.
//! - Pass 1: syntactic (tokens -> generic `NotaValue` tree). Lives here
//!   because `nota-codec` does NOT yet expose this — the design claim
//!   that "Pass 0 + Pass 1 already exist" in `nota-codec` is wrong;
//!   `nota-codec` is streaming-only. This is the first big design flaw
//!   surfaced by the proof-of-concept.
//! - Pass 2: structural (NotaValue -> `MultiPassDocument` with six
//!   semantic positions).
//! - Pass 3: macro identification (walk positions, classify by syntactic
//!   shape, build typed `MacroVariantInstance` per position).
//! - Pass 4: macro application (run each variant's lowerer; imports
//!   first, then types, headers, features).
//! - Pass 5: assembly (`LoweringContext.finish()` -> `AssembledSchema`).

pub mod nota_value;
pub mod pass1_parser;
pub mod pass2_structural;
pub mod pass3_identify;
pub mod pass4_lower;
pub mod pass5_assemble;
pub mod span;

use std::path::Path;

use crate::AssembledSchema;

pub use nota_value::NotaValue;
pub use pass2_structural::MultiPassDocument;
pub use pass3_identify::MacroVariantInstance;
pub use span::Span;

/// Run the six-pass pipeline against schema text and return the
/// canonical `AssembledSchema`.
pub fn read_str(input: &str) -> crate::Result<AssembledSchema> {
    read_str_with_imports(input, &|_| Err(MissingResolver))
}

/// Resolver callback for sibling-schema imports. Returns the imported
/// `AssembledSchema` for the given path. Side-effectful — this is the
/// ONLY pass-4 effect surface (per /334 §8 Q1).
pub type ImportResolver<'a> = dyn Fn(&str) -> Result<AssembledSchema, MissingResolver> + 'a;

/// Error returned by a resolver that doesn't know how to resolve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingResolver;

pub fn read_str_with_imports(
    input: &str,
    resolver: &ImportResolver<'_>,
) -> crate::Result<AssembledSchema> {
    // Pass 0 + Pass 1 + Pass 2 merged: read six top-level values.
    let document = pass2_structural::parse_six_values(input)?;
    // Pass 3: identify macro variants per position.
    let identified = pass3_identify::identify(&document)?;
    // Pass 4 + 5: lower + assemble.
    let assembled = pass4_lower::lower_and_assemble(identified, resolver)?;
    Ok(assembled)
}

/// File-path entry point. Composes a path-relative resolver.
pub fn read_path(path: &Path) -> crate::Result<AssembledSchema> {
    let text = std::fs::read_to_string(path).map_err(|error| crate::Error::SchemaReadFailed {
        path: path.display().to_string(),
        message: error.to_string(),
    })?;
    let base = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let resolver = move |import_path: &str| {
        let resolved = if Path::new(import_path).is_absolute() {
            Path::new(import_path).to_path_buf()
        } else {
            base.join(import_path)
        };
        read_path(&resolved).map_err(|_| MissingResolver)
    };
    read_str_with_imports(&text, &resolver)
}
