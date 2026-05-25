//! Pass 5 — Assembly.
//!
//! The hard work is done inside `LoweringContext::finish()` (the existing
//! engine machinery). This module is the seam where future
//! AssembledSchema-level validation (cross-reference checks, layout-
//! after-assemble per /334 §3.6) would land.
//!
//! Today the existing `LoweringContext::finish()` returns
//! `AssembledSchema::new(imports, routes, types, features)` which is
//! pure. So Pass 5 is effectively a no-op pass-through. This is itself
//! a finding: there is no separate "assemble" pass with real logic in
//! the current crate — everything fans into the LoweringContext.
