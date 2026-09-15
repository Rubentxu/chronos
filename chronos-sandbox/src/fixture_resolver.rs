//! Compile-time fixture resolver for the chronos-sandbox harness.
//!
//! ## Contract
//!
//! [`FixtureResolver::root`] returns the absolute path to the directory
//! where the C fixtures (`test_busyloop`, `test_segfault`, …) live. The
//! directory is published by [`build.rs`] via the compile-time env var
//! `CHRONOS_FIXTURE_DIR`, then read here with `env!("CHRONOS_FIXTURE_DIR")`.
//!
//! Because the path is baked into every test binary at compile time,
//! fixture discovery is identical under:
//!
//!   * `cargo test`
//!   * `cargo tarpaulin --workspace` (does NOT propagate `OUT_DIR`)
//!   * `CARGO_TARGET_DIR=/any/path cargo test`
//!   * CI on a fresh checkout
//!
//! No walk-up by `current_exe()`, no hardcoded `target/` paths, no Cargo
//! target-dir hashes in source. If the build system ever changes how
//! fixtures are materialized (e.g. copies them somewhere else), only
//! `build.rs` changes; tests stay the same.
//!
//! ## Test-only surface
//!
//! This module is part of the sandbox harness and is **never** linked
//! into production binaries (`chronos-mcp`, `chronos-cli`). The compile-
//! time env var is a test fixture, not a production contract.
//!
//! [`build.rs`]: ../build.rs

use std::path::{Path, PathBuf};

/// Resolves the absolute path of compiled C fixtures used by the sandbox
/// harness tests.
///
/// The resolver has no state and no allocation; it is a typed namespace
/// over the compile-time env var so the API can be extended (e.g. with
/// fixtures that have non-default search paths) without changing every
/// call site.
pub struct FixtureResolver;

impl FixtureResolver {
    /// Absolute path to the directory containing every compiled fixture.
    ///
    /// This is the canonical root for the sandbox test harness. Production
    /// code MUST NOT call this — it is only meaningful inside tests.
    pub fn root() -> &'static Path {
        Path::new(env!("CHRONOS_FIXTURE_DIR"))
    }

    /// Absolute path to a single compiled fixture.
    ///
    /// Returns the path without checking existence — callers can decide
    /// whether a missing fixture is fatal (the harness wants this), and the
    /// regression test ([`crate::REQUIRED_FIXTURES`]) does its own
    /// existence check so missing fixtures produce a precise diagnostic
    /// listing every fixture that should exist.
    ///
    /// [`crate::REQUIRED_FIXTURES`]: ../../tests/fixture_resolver.rs
    pub fn fixture(name: &str) -> PathBuf {
        Self::root().join(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_absolute() {
        let root = FixtureResolver::root();
        assert!(
            root.is_absolute(),
            "CHRONOS_FIXTURE_DIR must be an absolute path; got {:?}",
            root
        );
    }

    #[test]
    fn root_points_to_existing_dir() {
        let root = FixtureResolver::root();
        assert!(
            root.is_dir(),
            "CHRONOS_FIXTURE_DIR does not point to a directory: {:?}",
            root
        );
    }

    #[test]
    fn fixture_appends_name() {
        let p = FixtureResolver::fixture("test_segfault");
        assert_eq!(
            p.file_name().and_then(|s| s.to_str()),
            Some("test_segfault"),
            "FixtureResolver::fixture must append the name as the final segment"
        );
        assert!(p.starts_with(FixtureResolver::root()));
    }
}
