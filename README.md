# schema

Typed schema-language substrate for Persona signal contracts.

This crate models the authored six-position `.schema` form and lowers it
into `AssembledSchema` for macro consumers.

The current MVP surface covers `.schema` text parsing, local relative import
loading, ordinary/owner/sema route headers, namespace declarations, feature
metadata, route lowering, import collision checks, root-versus-box layout
planning, and first-pass previous/next upgrade planning. It does not emit
Rust code yet.
