# REC-C0.5-C — Investigation Notes: Sandbox fixture discovery under tarpaulin

> **Cycle**: REC-C0.5-C (issue #30)
> **Author**: REC-C0 closure session
> **Date**: 2026-09-15
> **Status**: Investigation complete; design + fix in follow-up commits.

## TL;DR

`cargo tarpaulin --workspace` does NOT propagate the `OUT_DIR` environment variable to
the test binaries it executes. `cargo test` does. The current
`McpSession::fixture_path` resolution in `chronos-sandbox/src/client/tools.rs` reads
`OUT_DIR` via `env::var("OUT_DIR")` — which returns `None` under tarpaulin. The fallback
walk-up by `current_exe()` parent also fails because tarpaulin places test binaries in
`debug/deps/` and the fixtures are in `debug/build/chronos-sandbox-<HASH>/out/` (a
sibling-tree the walk-up doesn't reach).

This is the **root cause** of why sandbox tests panic with `fixture not found - run
cargo build first` under the Coverage workflow.

## Reproduction

I added a temporary probe test `chronos-sandbox/tests/fixture_env_probe.rs` (removed
after the investigation) that writes `OUT_DIR`, `CARGO_MANIFEST_DIR`, `CARGO_TARGET_DIR`,
`current_exe`, and the result of walking up the parent dirs to look for
`test_segfault` to a file next to the test binary.

### Under `cargo test`

```
=== ENVIRONMENT CHARACTERIZATION ===
OUT_DIR          = Some("/var/home/rubentxu/cargo-targets/debug/build/chronos-sandbox-65a70ee614d37d78/out")
CARGO_MANIFEST_DIR = Some("/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos/chronos-sandbox")
CARGO_TARGET_DIR   = None
current_exe        = Some("/var/home/rubentxu/cargo-targets/debug/deps/fixture_env_probe-fec04ffbcf7cfe2f")
current_exe.parent = Some("/var/home/rubentxu/cargo-targets/debug/deps")
OUT_DIR/test_segfault exists = true (/var/home/rubentxu/cargo-targets/debug/build/chronos-sandbox-65a70ee614d37d78/out/test_segfault)
Walk[0/5] /var/home/rubentxu/cargo-targets/debug/deps/test_segfault exists=false
Walk[1/5] /var/home/rubentxu/cargo-targets/debug/test_segfault exists=false
Walk[2/5] /var/home/rubentxu/cargo-targets/test_segfault exists=false
Walk[3/5] /var/home/rubentxu/cargo-targets/test_segfault exists=false
Walk[4/5] /var/home/rubentxu/test_segfault exists=false
Walk[5/5] /var/home/rubentxu/test_segfault exists=false
===================================
```

`OUT_DIR` is set; the fixture is reachable via `OUT_DIR/test_segfault`. The
`current_exe()` walk-up never finds it.

### Under `cargo tarpaulin -p chronos-sandbox --test fixture_env_probe --out Stdout`

```
=== ENVIRONMENT CHARACTERIZATION ===
OUT_DIR          = None
CARGO_MANIFEST_DIR = Some("/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos/chronos-sandbox")
CARGO_TARGET_DIR   = None
current_exe        = Some("/var/home/rubentxu/cargo-targets/debug/deps/fixture_env_probe-2fa2eee77814d2a7")
current_exe.parent = Some("/var/home/rubentxu/cargo-targets/debug/deps")
OUT_DIR is unset
Walk[0/5] /var/home/rubentxu/cargo-targets/debug/deps/test_segfault exists=false
Walk[1/5] /var/home/rubentxu/cargo-targets/debug/test_segfault exists=false
Walk[2/5] /var/home/rubentxu/cargo-targets/test_segfault exists=false
Walk[3/5] /var/home/rubentxu/cargo-targets/test_segfault exists=false
Walk[4/5] /var/home/rubentxu/cargo-targets/test_segfault exists=false
Walk[5/5] /var/home/rubentxu/test_segfault exists=false
===================================
```

`OUT_DIR = None`. Tarpaulin did NOT propagate it. The walk-up still fails.

### But the fixture IS built

After the tarpaulin run, two `chronos-sandbox-<HASH>/out/test_segfault` directories
exist:

```
/var/home/rubentxu/cargo-targets/debug/build/chronos-sandbox-00464defc83215f9/out/test_segfault   <- tarpaulin's build
/var/home/rubentxu/cargo-targets/debug/build/chronos-sandbox-65a70ee614d37d78/out/test_segfault   <- cargo test's build
```

So tarpaulin **does run the build script** (it has to — it can't link a test binary
without first compiling the crate and its build artifacts). It produces a different
build-script output dir (hash differs because tarpaulin compiles with
`--cfg=tarpaulin -Clink-dead-code`, which changes the build script's metadata hash).
But it does **not export `OUT_DIR`** to the test binary's environment.

## Answers to the 6 characterization questions

1. **Value of `OUT_DIR` in normal `cargo build` / `cargo test`**:
   `/var/home/rubentxu/cargo-targets/debug/build/chronos-sandbox-65a70ee614d37d78/out`
   (the build script writes to `OUT_DIR`, Cargo exports it to dependent crates and
   test binaries).

2. **Value / absence of `OUT_DIR` under `cargo tarpaulin --workspace`**:
   `OUT_DIR = None`. Tarpaulin wraps cargo, runs the build script (build artifacts
   are produced — confirmed by `chronos-sandbox-00464defc83215f9/out/test_segfault`
   existing), but does not propagate `OUT_DIR` to the spawned test binary's
   environment.

3. **Real location of the fixture binaries**:
   `target/debug/build/chronos-sandbox-<HASH>/out/test_segfault`. The `<HASH>`
   differs between cargo test (no `--cfg=tarpaulin`) and tarpaulin runs (with
   `--cfg=tarpaulin`).

4. **How `McpSession::fixture_path` obtains the path**: it reads `OUT_DIR` first
   (`if let Ok(out_dir) = std::env::var("OUT_DIR") { ... }`). If unset, it tries to
   find the fixture relative to `current_exe()`. Under tarpaulin, both branches
   fail — `OUT_DIR` is None, and the walk-up doesn't reach the build output dir.

5. **Was the path baked at compile time?** No. `OUT_DIR` is read via `env::var()` at
   runtime. So the path is correctly resolved at runtime **under cargo test**, but
   missing at runtime **under cargo tarpaulin**.

6. **Does tarpaulin recompile / instrument the crate with a different `OUT_DIR`?**
   Yes: tarpaulin recompiles every crate with `--cfg=tarpaulin -Clink-dead-code`,
   which changes the build script's fingerprint, which makes Cargo allocate a new
   `OUT_DIR` (`chronos-sandbox-00464defc83215f9/out`). Tarpaulin then runs the test
   binary but does NOT propagate the new `OUT_DIR` to its environment.

## Connascence analysis

`McpSession::fixture_path` has:

- **Connascence of Execution** with `OUT_DIR`: it relies on Cargo's runtime contract
  of exporting `OUT_DIR` to the test binary. Tarpaulin violates this contract.
- **Connascence of Execution** with `current_exe()` location: it relies on the test
  binary being a fixed number of levels away from the build output dir. The walk-up
  is fragile and depends on Cargo's directory layout.
- **Connascence of Name** with Cargo's target dir layout: `target/debug/deps/` vs
  `target/debug/build/`. Cargo doesn't promise these locations are stable.

This is fragile. The user's design eliminates the connascence by replacing
discovery-from-environment with a single deterministic contract.

## Recommended design

A `FixtureResolver` (or equivalent) that:

1. Uses `env!("CHRONOS_FIXTURE_DIR")` set by `build.rs` via `cargo:rustc-env=`. This
   is the **most robust** option because the value is baked into the test binary at
   compile time and does not depend on runtime environment variables.

   `build.rs` writes:
   ```rust
   println!("cargo:rustc-env=CHRONOS_FIXTURE_DIR={}", out_dir);
   ```

   Test code uses:
   ```rust
   let dir = env!("CHRONOS_FIXTURE_DIR");
   let fixture = Path::new(dir).join(name);
   ```

   Tarpaulin **does** preserve `cargo:rustc-env` directives when re-compiling (it
   re-runs the build script and propagates the env vars it sets). This is documented
   behavior.

2. As a defense in depth, the `FixtureResolver` also accepts a `CHRONOS_FIXTURE_DIR`
   environment variable override at runtime (e.g. `CHRONOS_FIXTURE_DIR=/some/path
   cargo test ...`), for the case where someone moves the fixture out of the
   build output dir.

3. **No walk-up by `current_exe()`**. It is fragile and untestable.

### Regression test

A new integration test `chronos-sandbox/tests/fixture_resolver.rs` asserts:

```text
1. Default resolution: env!("CHRONOS_FIXTURE_DIR")/test_segfault exists.
2. With CARGO_TARGET_DIR=/tmp/alt-target-dir cargo test:
   the fixture resolves under /tmp/alt-target-dir/.../chronos-sandbox-<HASH>/out/.
3. With CHRONOS_FIXTURE_DIR=/explicit/path:
   the fixture resolves to /explicit/path/test_segfault.
```

Item 2 requires spawning a subprocess with `CARGO_TARGET_DIR` set, since the value
of `env!()` is compile-time. This is acceptable because the regression test itself
is allowed to be slow.

## Out-of-scope for this investigation

- Capability-aware probe_inject split: issue #29 (REC-C0.5-B).
- Vault Drift: issue #28 (REC-C0.5-A).
- The tarpaulin pin (`0.37.2`) and the `--output-dir` flag are correct; not
  changed by this work item.

## Conclusion

**Root cause confirmed**: tarpaulin does not propagate `OUT_DIR` to test binaries.
**Fix**: switch to `env!("CHRONOS_FIXTURE_DIR")` via `build.rs` `cargo:rustc-env=`,
with a runtime override via `CHRONOS_FIXTURE_DIR` env var. Drop the
`current_exe()` walk-up. Add the regression test.
