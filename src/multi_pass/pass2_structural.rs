//! Pass 2 — Structural. Asserts the six-position shape per /334 §3.3.
//!
//! Input: a generic `NotaValue` from Pass 1. The .schema file's root
//! `NotaValue` is — by current intent — a "tuple of six positions"
//! laid out at the top level WITHOUT an enclosing record/list.
//!
//! In other words a `.schema` file looks like:
//!
//! ```text
//! { imports } [ ordinary ] [ owner ] [ sema ] { namespace } [ features ]
//! ```
//!
//! Six values at the top level — not one record containing six. This
//! is a tension the design report glosses over: Pass 1 returns ONE
//! NotaValue, but the file shape is SIX. Resolved here by treating the
//! "schema document" as a synthesised top-level record. We change
//! `parse` to read six values in sequence.

use crate::multi_pass::{NotaValue, Span};
use crate::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiPassDocument {
    pub imports: NotaValue,
    pub ordinary_header: NotaValue,
    pub owner_header: NotaValue,
    pub sema_header: NotaValue,
    pub namespace: NotaValue,
    pub features: NotaValue,
}

/// Pass 1 returns one value; Pass 2 needs six. We work around it by
/// re-parsing as a sequence of six values. The proper fix is to change
/// the .schema file root to be a single 6-element record — but that
/// breaks every existing schema. So we accept the file shape as-is.
pub fn structural(value: NotaValue) -> Result<MultiPassDocument> {
    // If Pass 1 was wrapped (single record/list), unwrap it. Otherwise
    // require a synthesised top-level (handled by the caller — see
    // parse_six_values below).
    if let NotaValue::Record(items, _) = &value {
        if items.len() == 6 {
            return six_to_document(items);
        }
    }
    Err(Error::InvalidSchemaText {
        context: "pass2",
        message: format!(
            "expected 6-position schema document (synthesised record), got {}",
            value.kind_name()
        ),
    })
}

/// Public entry point — read six values from text without requiring an
/// outer record. This is what the multi-pass runner calls.
pub fn parse_six_values(input: &str) -> Result<MultiPassDocument> {
    let mut parser = super::pass1_parser::SequenceParser::new(input);
    let imports = parser.next_value("imports")?;
    let ordinary = parser.next_value("ordinary header")?;
    let owner = parser.next_value("owner header")?;
    let sema = parser.next_value("sema header")?;
    let namespace = parser.next_value("namespace")?;
    let features = parser.next_value("features")?;
    parser.finish()?;

    enforce_kind("imports", &imports, "map")?;
    enforce_kind("ordinary header", &ordinary, "list")?;
    enforce_kind("owner header", &owner, "list")?;
    enforce_kind("sema header", &sema, "list")?;
    enforce_kind("namespace", &namespace, "map")?;
    enforce_kind("features", &features, "list")?;

    Ok(MultiPassDocument {
        imports,
        ordinary_header: ordinary,
        owner_header: owner,
        sema_header: sema,
        namespace,
        features,
    })
}

fn six_to_document(items: &[NotaValue]) -> Result<MultiPassDocument> {
    enforce_kind("imports", &items[0], "map")?;
    enforce_kind("ordinary header", &items[1], "list")?;
    enforce_kind("owner header", &items[2], "list")?;
    enforce_kind("sema header", &items[3], "list")?;
    enforce_kind("namespace", &items[4], "map")?;
    enforce_kind("features", &items[5], "list")?;

    Ok(MultiPassDocument {
        imports: items[0].clone(),
        ordinary_header: items[1].clone(),
        owner_header: items[2].clone(),
        sema_header: items[3].clone(),
        namespace: items[4].clone(),
        features: items[5].clone(),
    })
}

fn enforce_kind(name: &'static str, value: &NotaValue, expected: &str) -> Result<()> {
    if value.kind_name() == expected {
        Ok(())
    } else {
        let _ = Span::empty();
        Err(Error::InvalidSchemaText {
            context: "pass2",
            message: format!(
                "position `{name}` expected {expected}, got {}",
                value.kind_name()
            ),
        })
    }
}
