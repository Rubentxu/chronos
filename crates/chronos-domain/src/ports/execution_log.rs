//! `ExecutionLogProvider` — domain-side port for the execution log storage.
//!
//! ## Status: deferred to REC-C3.3
//!
//! The underlying `ExecutionLogBackend` trait already lives in
//! `chronos-log/src/backend.rs` and `chronos-log` already depends on
//! `chronos-domain` for `InvocationId` and `SymbolId`. Adding
//! `chronos-log` as a dependency of `chronos-domain` would create a
//! Cargo.toml cyclic-dependency error.
//!
//! Therefore the re-export pattern proposed in AD-3 of the design
//! doc is NOT possible today. The port shape will be introduced
//! during REC-C3.3 (invert services → concrete adapters), where the
//! concrete `SegmentedExecutionLog` import in services is replaced
//! with a domain-side alias. That refactor opens the door to either:
//!
//! - Splitting `ExecutionLogBackend` and `NewExecutionRecord` into
//!   domain (so the trait has no domain-types in its signature), then
//!   re-exporting here; OR
//! - Moving the trait wholesale to domain (depends on the split).
//!
//! For C3.1, this module re-exports the *capability* surface from
//! `chronos-domain::capability::Capability` as a placeholder, plus a
//! no-op `NoopExecutionLogProvider` for test composition. The real
//! storage port lands in C3.3.

use crate::capability::Capability;

/// Placeholder type alias for the storage port. Real definition in C3.3.
pub type ExecutionLogProviderShape = Capability;

/// No-op implementation used in tests and the "log-disabled"
/// composition-root fallback. C3.3 will replace this with a real
/// default that delegates to `ExecutionLogBackend`.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopExecutionLogProvider;

impl NoopExecutionLogProvider {
    pub fn new() -> Self {
        Self
    }
}

#[cfg(test)]
mod tests {
    //! Compile-only checks. Behavioral tests live under
    //! `crates/chronos-domain/tests/ports/execution_log.rs`.
    use super::*;

    fn _shape_compiles(_: ExecutionLogProviderShape) {}
}
