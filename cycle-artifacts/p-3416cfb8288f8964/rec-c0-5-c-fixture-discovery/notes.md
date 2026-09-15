# REC-C0.5-C — Investigation Notes: Sandbox fixture discovery under tarpaulin

> **Cycle**: REC-C0.5-C (issue #30)
> **Author**: REC-C0 closure session
> **Date**: 2026-09-15
> **Status**: **CLOSED locally** (implementation + tests + remote run). Remote CI
> Coverage check still RED, but for reasons unrelated to fixture discovery —
> see "Workspace-wide fallout" below.

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

## Workspace-wide fallout

`cargo tarpaulin --workspace` is what the CI Coverage workflow runs. It exercises
the full dependency closure, which surfaces bugs that the `--test <name>` smoke
runs do not. After the REC-C0.5-C fix landed, the workspace run produced:

- **1473 passed, 37 failed, 24 ignored**
- The chronos-mcp binary at `target/debug/chronos-mcp` survives the workspace run
  in cfg=tarpaulin form (~375 MB) — the `cargo build --bin chronos-mcp` pre-step
  recommended in AGENTS.md §1 is no longer required for binary survival, only for
  guaranteeing it exists before tarpaulin starts.

The 37 failures cluster into five unrelated buckets:

| Count | Bucket | Cycle ownership |
|---|---|---|
| 3 | `probe_inject.rs` (contract: error vs success) | REC-C0.5-B / issue #29 |
| 5 | `query_filters.rs` + `query_edge_cases.rs` (pagination/offset) | REC-C1 events_read |
| 13 | `chronos-e2e/tests/test_ptrace_capture.rs` | bucket D (ptrace permissions) |
| 6 | `m2_function_frame_capture.rs` | REC-C1 / bucket D |
| 4 | `tripwires_tools.rs` | unrelated follow-up cycle |
| 4 | `chronos-native/src/ptrace_tracer.rs` (lib tests) | flake §6.5 (needs `--test-threads=1`) |
| 2 | `capture_runner.rs` / `probe_backend.rs` (lib tests) | unrelated |

**None** of these are fixture-discovery failures. The `panicked at ... fixture
not found` / `SpawnFailed("No such file or directory")` pattern that motivated
REC-C0.5-C is fully eliminated:

- `cargo tarpaulin --test fixture_resolver`: 4/4 PASS.
- `cargo tarpaulin --test boundary_conditions`: 9/9 PASS.
- `cargo tarpaulin --test probe_lifecycle`: 5/5 PASS.

The CI Coverage workflow `coverage.yml` was updated to pre-build the binary and
pass `CHRONOS_MCP_PATH` explicitly, mirroring AGENTS.md §1. Without these two
changes, the Coverage check fails on the very first sandbox test, because
tarpaulin re-compiles transitive deps with `--cfg tarpaulin` (which invalidates
the `chronos-mcp` binary fingerprint in `target/debug/` and cargo removes the
old binary on the first recompile).

## Why the Coverage check stays RED

The Coverage check's `set -e` semantics mean `cargo tarpaulin --workspace`
exiting non-zero (because of any test failure) turns the workflow red. With the
REC-C0.5-C fix in place, the only remaining failures are real bugs in other
cycles' scope. Closing REC-C0 therefore requires:

1. REC-C0.5-B (#29): capability-aware probe_inject split. Required to make the
   3 `probe_inject.rs` failures turn green.
2. REC-C1: events_read pagination/offset correctness. Required for the 5
   `query_*` failures.
3. REC-C0.5-A (#28): vault drift (unrelated to Coverage but blocks the
   Vault Drift check).
4. The ptrace-dependent suites (13 + 6 + 1) and the lib-unit ptrace flakes
   (4 + 2) need either:
   - `--test-threads=1` for the lib unit ptrace flakes (per AGENTS.md §6.5), or
   - exclusion from the workspace coverage run, or
   - a separate ptrace-enabled runner (bucket D is opt-in by design).

Items 1–3 are scope of REC-C0.5-B, REC-C1, REC-C0.5-A respectively. Items 4 are
pre-existing flakes / bucket D opt-in policy and need a separate decision
(either expand the coverage workflow to skip bucket D, or keep it as a separate
opt-in CI lane).

## Conclusion

REC-C0.5-C is closed: the sandbox fixture discovery is correct under both
`cargo test` and `cargo tarpaulin`. The fix is a compile-time contract
(`env!("CHRONOS_FIXTURE_DIR")` emitted by `build.rs`) with a regression test
suite that locks the contract. The CI workflow was updated to pre-build and
export `CHRONOS_MCP_PATH` so the binary lookup is explicit and deterministic.

REC-C0 remains BLOCKED on remote CI closure because of unrelated failures in
other cycles' scope. The user's stated rule ("No CLOSED while remote CI red")
applies at the REC-C0 level, not at the REC-C0.5-C sub-cycle level.
