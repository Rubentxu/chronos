//! Build script for `chronos-sandbox`.
//!
//! Responsibilities:
//!   1. Compile each C fixture (`programs/c/*.c`) into the Cargo-provided
//!      `OUT_DIR`. The fixtures are small standalone programs that the
//!      sandbox tests `fork`/`exec` and probe at runtime.
//!   2. Publish the absolute path of `OUT_DIR` to dependent code (the
//!      library itself and every integration test that links against it)
//!      via the compile-time env var `CHRONOS_FIXTURE_DIR`. Tests read it
//!      with `env!("CHRONOS_FIXTURE_DIR")` via the centralized
//!      [`FixtureResolver`] helper.
//!
//! Why `cargo:rustc-env=` (compile-time) instead of `OUT_DIR` (runtime):
//!   * `cargo test` exports `OUT_DIR` to the test binary's environment, but
//!     `cargo tarpaulin --workspace` does NOT. Switching to a compile-time
//!     contract eliminates the dependence on the runtime environment and
//!     makes the fixture path stable across both invocations.
//!   * No need for tests to walk up from `current_exe()` or hardcode target
//!     dir hashes. Cargo's layout can change between versions; the build
//!     script's contract is the canonical source of truth.
//!
//! The fixtures themselves remain owned exclusively by the sandbox harness
//! and tests — `CHRONOS_FIXTURE_DIR` is NEVER consulted by production code
//! (it is exposed only through the test-only `FixtureResolver` API; see
//! `src/fixture_resolver.rs`).
//!
//! [`FixtureResolver`]: ../src/fixture_resolver.rs

use std::path::PathBuf;
use std::process::Command;

/// Programs compiled into the fixture root. Each is a small C harness used
/// by sandbox tests to exercise the live probe pipeline. The list is the
/// canonical declaration: if you add a fixture here, you must also add it
/// to `REQUIRED_FIXTURES` in `tests/fixture_resolver.rs` so the regression
/// test enforces its presence.
const PROGRAMS: &[&str] = &[
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

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR set by Cargo");
    let out_path = PathBuf::from(&out_dir);

    for prog in PROGRAMS {
        let src = format!("programs/c/{}.c", prog);
        let out = out_path.join(prog);
        let out_str = out
            .to_str()
            .unwrap_or_else(|| panic!("fixture path is not valid UTF-8: {:?}", out));

        // test_function_frames needs -no-pie -O0 -fno-inline so the
        // SymbolResolver finds size-bearing text symbols whose entry
        // addresses the live-probe frame-capture pipeline can INT3-breakpoint
        // and single-step through.
        // test_function_frames_pie needs the same flags except -pie -fPIE so
        // the runtime addresses require ASLR load-bias relocation; this
        // exercises the non-zero-bias branch of `Int3Injector::compute_load_bias`.
        let extra_flags: &[&str] = if *prog == "test_function_frames" {
            &["-no-pie", "-O0", "-fno-inline"]
        } else if *prog == "test_function_frames_pie" {
            &["-pie", "-fPIE", "-O0", "-fno-inline"]
        } else {
            &[]
        };

        let status = Command::new("gcc")
            .args(["-g", "-O0", "-pthread", &src, "-o", out_str])
            .args(extra_flags)
            .status();
        match status {
            Ok(s) if s.success() => {}
            _ => {
                // Fall back without extra flags if -no-pie is rejected by the
                // toolchain (older gccs / clang). The default -O0 -g still
                // keeps enough symbols for the live probe to resolve at least
                // `main`; full FunctionEntry assertions live in
                // chronos-native/tests/m2_function_frame_capture.rs.
                Command::new("gcc")
                    .args(["-g", "-O0", "-pthread", &src, "-o", out_str])
                    .status()
                    .unwrap_or_else(|_| panic!("Failed to compile {}", prog));
            }
        }

        // Tell cargo to rerun if source changes.
        println!("cargo:rerun-if-changed={}", src);
    }

    // Publish the fixture root as a compile-time constant. Every integration
    // test that depends on chronos-sandbox will pick this up at compile time,
    // so the path is resolved before the test binary is spawned — independent
    // of `OUT_DIR` being propagated to the test process's environment.
    //
    // Canonicalize so the path is stable across symlinks (e.g. /var/home vs
    // /home/rubentxu on the same machine) and so the assertion in the
    // regression test compares exactly the path build.rs wrote.
    let canonical = out_path
        .canonicalize()
        .unwrap_or_else(|_| out_path.clone());
    let canonical_str = canonical.to_str().expect("fixture root is valid UTF-8");
    println!("cargo:rustc-env=CHRONOS_FIXTURE_DIR={}", canonical_str);

    // Rerun this build script if any of the listed programs change. The
    // PROGRAMS list itself is recompiled only when build.rs changes (its
    // default behavior), so the regression test enforces the list.
    println!("cargo:rerun-if-changed=build.rs");
}
