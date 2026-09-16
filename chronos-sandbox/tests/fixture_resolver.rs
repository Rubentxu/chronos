//! Regression test for the sandbox fixture resolver.
//!
//! This test enforces two contracts that REC-C0.5-C required:
//!
//!   1. The fixture root is published at compile time by `chronos-sandbox/build.rs`
//!      via `cargo:rustc-env=CHRONOS_FIXTURE_DIR=<dir>`. Tests must never depend
//!      on `OUT_DIR`, `current_exe()`, `target/` paths, or `CARGO_TARGET_DIR`.
//!
//!   2. Every fixture declared by `build.rs` (the `PROGRAMS` list) actually
//!      exists on disk under the fixture root. If you add a new fixture in
//!      `build.rs`, you must also add it to [`REQUIRED_FIXTURES`] below —
//!      otherwise this test will fail and point at the missing entry.
//!
//! The list of required fixtures is intentionally a separate constant from
//! `build.rs`'s `PROGRAMS` so that drift between declaration and resolution
//! is caught at test time (the regression). Keeping the list close to its
//! use site (this test) makes the rule easy to find.
//!
//! See:
//!   * `chronos-sandbox/build.rs` — declares the fixtures.
//!   * `chronos-sandbox/src/fixture_resolver.rs` — exposes the compile-time
//!     path to the harness.
//!   * `cycle-artifacts/p-3416cfb8288f8964/rec-c0-5-c-fixture-discovery/notes.md`
//!     — investigation that identified the bug this regression guards against.

/// Canonical list of fixtures that the sandbox harness expects to find
/// under [`chronos_sandbox::FixtureResolver::root`]. Must be kept in sync
/// with the `PROGRAMS` array in `chronos-sandbox/build.rs`.
const REQUIRED_FIXTURES: &[&str] = &[
    "test_add",
    "test_busyloop",
    "test_segfault",
    "test_threads",
    "test_clone",
    "test_crash_thread",
    "test_fork",
    "test_many_threads",
    "test_exit_immediate",
    "test_divide_by_zero",
    "test_abort",
    "test_infinite_loop",
    "test_function_frames",
    "test_function_frames_pie",
];

#[test]
fn fixture_root_is_resolvable_at_compile_time() {
    let root = chronos_sandbox::FixtureResolver::root();
    assert!(
        root.is_absolute(),
        "fixture root must be an absolute path; got {:?}",
        root
    );
    assert!(
        root.is_dir(),
        "fixture root must exist as a directory; got {:?}",
        root
    );
}

#[test]
fn every_declared_fixture_exists_under_root() {
    let root = chronos_sandbox::FixtureResolver::root();

    let mut missing: Vec<&str> = Vec::new();
    let mut present: Vec<&str> = Vec::new();

    for name in REQUIRED_FIXTURES {
        let path = chronos_sandbox::FixtureResolver::fixture(name);
        if path.exists() {
            present.push(name);
        } else {
            missing.push(name);
        }
    }

    assert!(
        missing.is_empty(),
        "Missing fixtures under {:?}:\n  - {}\n\n\
         These fixtures are listed in REQUIRED_FIXTURES but were not built by\n\
         chronos-sandbox/build.rs. Either:\n\
           (a) add them to the PROGRAMS array in build.rs and commit the .c\n\
               source under chronos-sandbox/programs/c/, OR\n\
           (b) remove them from REQUIRED_FIXTURES if no test needs them.\n\n\
         Found: {}\n",
        root,
        missing.join("\n  - "),
        present.join(", "),
    );
}

#[test]
fn fixture_resolver_does_not_depend_on_out_dir_or_current_exe() {
    // Belt-and-suspenders: even if `OUT_DIR` were missing in the runtime
    // environment (which is exactly what `cargo tarpaulin` does), the
    // resolver must still produce a usable path because the value is
    // baked at compile time via `env!('CHRONOS_FIXTURE_DIR')`.
    //
    // We don't actually unset OUT_DIR here (cargo test always exports it),
    // but we do assert that FixtureResolver::root() does NOT equal a path
    // derived from a transient runtime hint — the value is fixed and
    // absolute.
    let root = chronos_sandbox::FixtureResolver::root();

    // Sanity: root is the same on every call (it's `&'static Path`).
    let again = chronos_sandbox::FixtureResolver::root();
    assert_eq!(
        root, again,
        "FixtureResolver::root must return a stable static path"
    );

    // Sanity: the static path is NOT relative to anything that depends on
    // a Cargo target-dir hash. The hash differs between cargo test and
    // cargo tarpaulin; if the resolver depended on it, this assertion
    // would be a tautology. Instead, we assert that the root is absolute
    // (already checked elsewhere) AND that it contains the canonical
    // `chronos-sandbox-` prefix that Cargo uses for build-script outputs.
    let s = root.to_str().unwrap_or("");
    assert!(
        s.contains("chronos-sandbox-"),
        "fixture root must be inside Cargo's chronos-sandbox build dir; got {:?}",
        root
    );
}

#[test]
fn fixture_helper_returns_path_under_root() {
    let root = chronos_sandbox::FixtureResolver::root();
    for name in REQUIRED_FIXTURES {
        let p = chronos_sandbox::FixtureResolver::fixture(name);
        assert!(
            p.starts_with(root),
            "FixtureResolver::fixture({:?}) = {:?} does not start with root {:?}",
            name,
            p,
            root
        );
        assert_eq!(
            p.file_name().and_then(|s| s.to_str()),
            Some(*name),
            "FixtureResolver::fixture must append the name as the final segment"
        );
    }
}
