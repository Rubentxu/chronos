//! `EventSeq` (stub) — the canonical definition lives in
//! `chronos_domain::seq::EventSeq` as of REC-C3.3.1.
//!
//! This module kept a duplicate definition historically so that
//! `chronos_log` could compile without depending on `chronos_domain`'s
//! seq module. ADR-0015 D3 lifts the type to domain; the duplicate
//! is deleted. The `pub mod seq` declaration is kept only because
//! some internal code in `chronos_log` imports via `crate::seq::…` —
//! those paths now go through `crate::EventSeq` (the re-export from
//! `lib.rs`) or `crate::seq::EventSeq` resolves to the domain type
//! via this module's re-export.
//!
//! This file is intentionally minimal: it exists so the module path
//! remains valid; the actual type lives in the domain crate.

pub use chronos_domain::seq::EventSeq;
