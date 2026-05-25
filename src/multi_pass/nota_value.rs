//! Generic `NotaValue` tree produced by Pass 1.
//!
//! Per /334 §3.2: variants are `NotaRecord` (parenthesised positional),
//! `NotaList` (square-bracket positional), `NotaMap` (curly-brace
//! name-value), `NotaIdentifier`, `NotaString`, `NotaInteger`. This
//! crate currently does NOT depend on `nota-codec` for this type;
//! `nota-codec` exposes only a streaming `Decoder`. Lifting the tree
//! into `nota-codec` is the natural follow-up (per /334 §6 "in
//! nota-codec: nothing new" turns out to be wrong).

use crate::multi_pass::Span;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotaValue {
    Record(Vec<NotaValue>, Span),
    List(Vec<NotaValue>, Span),
    Map(Vec<(String, NotaValue)>, Span),
    Identifier(String, Span),
    String(String, Span),
    Integer(i128, Span),
}

impl NotaValue {
    pub fn span(&self) -> Span {
        match self {
            Self::Record(_, span)
            | Self::List(_, span)
            | Self::Map(_, span)
            | Self::Identifier(_, span)
            | Self::String(_, span)
            | Self::Integer(_, span) => *span,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Record(..) => "record",
            Self::List(..) => "list",
            Self::Map(..) => "map",
            Self::Identifier(..) => "identifier",
            Self::String(..) => "string",
            Self::Integer(..) => "integer",
        }
    }

    pub fn as_identifier(&self) -> Option<&str> {
        if let Self::Identifier(name, _) = self {
            Some(name)
        } else {
            None
        }
    }

    pub fn as_record(&self) -> Option<&[NotaValue]> {
        if let Self::Record(items, _) = self {
            Some(items)
        } else {
            None
        }
    }

    pub fn as_list(&self) -> Option<&[NotaValue]> {
        if let Self::List(items, _) = self {
            Some(items)
        } else {
            None
        }
    }

    pub fn as_map(&self) -> Option<&[(String, NotaValue)]> {
        if let Self::Map(entries, _) = self {
            Some(entries)
        } else {
            None
        }
    }
}
