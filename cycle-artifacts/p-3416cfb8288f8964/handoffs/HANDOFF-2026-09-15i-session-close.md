# HANDOFF-2026-09-15i-session-close.md

> **Fecha**: 2026-09-15 22:05 UTC
> **Estado REC-C0**: implementation COMPLETE / local PASS / remote BLOCKED
> **Siguiente paso programado**: REC-C0.5-B (#29) — capability-aware probe_inject split

## Lo que se hizo hoy

### REC-C0 first wave (ya pushed: 7fbd66c1..685b2847)
chronos-ebpf + TraceDiff::compare, ChromeLocator, tarpaulin --output-dir fix,
fetch-depth:0, rec-c0-* cycle backfill + validation gate, HANDOFF-REC-C0-CLOSURE.md.

### Documentary state correction (`61e6a79a`)
HANDOFF actualizado de "CLOSED" a `implementation COMPLETE / local PASS / remote BLOCKED`.
Issues #28/#29/#30 reescritos con scope/DoD explícito.

### REC-C0.5-C implementation (`fcf58a94`..`02c2a552`, pushed)
Investigation notes + fix completo en 3 commits.

## REC-C0.5-C outcome — CLOSED LOCAL

### Root cause (confirmado empíricamente)
`cargo tarpaulin` no propaga `OUT_DIR` a los test binaries. El fallback
`current_exe()` walk-up tampoco llega a `target/debug/build/chronos-sandbox-<HASH>/out/`.

### Fix
- `chronos-sandbox/build.rs` ahora declara `PROGRAMS` const + emite
  `cargo:rustc-env=CHRONOS_FIXTURE_DIR=<canonical_out_dir>` después de construir fixtures.
  `cargo:rerun-if-changed={build.rs,src/...}` para refresh.
- `chronos-sandbox/src/fixture_resolver.rs` (nuevo): `FixtureResolver::root()` (vía
  `env!("CHRONOS_FIXTURE_DIR")`) + `FixtureResolver::fixture(name)`.
- `chronos-sandbox/src/lib.rs`: re-exporta `FixtureResolver`.
- `chronos-sandbox/src/client/tools.rs::McpSession::fixture_path` delega a
  `FixtureResolver::fixture(name)`. Sin `OUT_DIR` runtime, sin `current_exe()` walk-up.
- `chronos-sandbox/tests/fixture_resolver.rs` (nuevo): 4 regresiones —
  `REQUIRED_FIXTURES` synced con `PROGRAMS`, root resolvable en compile-time,
  every fixture exists, no dep on OUT_DIR/current_exe.
- `.github/workflows/coverage.yml`: pre-build `chronos-mcp` + `CHRONOS_MCP_PATH` env.

### Local verification
- `cargo test -p chronos-sandbox --test fixture_resolver`: **4/4 PASS**
- `cargo test -p chronos-sandbox --test boundary_conditions`: **9/9 PASS**
- `cargo test -p chronos-sandbox --test probe_lifecycle`: **5/5 PASS**
- `cargo tarpaulin -p chronos-sandbox --test fixture_resolver`: **4/4 PASS**
- `cargo tarpaulin -p chronos-sandbox --test boundary_conditions`: **9/9 PASS**
- `cargo tarpaulin --workspace`: **1473 passed / 37 failed / 24 ignored**
  - 0 fallos de fixture discovery. Los 37 son bugs reales en otros ciclos.

### Remote CI (run 35027238172)
- Architecture Contracts ✓ GREEN
- **Coverage ✗ RED — pero fixture discovery cerrado** (falla por probe_inject,
  events_read pagination, ptrace permissions, lib unit ptrace flakes)
- Vault Drift Sweep ✗ RED — scope de #28
- CI ✗ RED — desconocido

## Categorización de los 37 fallos restantes en `cargo tarpaulin --workspace`

| Count | Bucket | Cycle ownership |
|---|---|---|
| 3 | `chronos-sandbox/tests/probe_inject.rs` | REC-C0.5-B (#29) |
| 5 | `chronos-sandbox/tests/query_filters.rs` + `query_edge_cases.rs` | REC-C1 events_read |
| 13 | `crates/chronos-e2e/tests/test_ptrace_capture.rs` | bucket D (ptrace permissions) |
| 6 | `crates/chronos-native/tests/m2_function_frame_capture.rs` | REC-C1 / bucket D |
| 4 | `crates/chronos-mcp/tests/tripwires_tools.rs` | otro ciclo |
| 4 | `crates/chronos-native/src/ptrace_tracer.rs` (lib tests) | flake §6.5 (needs `--test-threads=1`) |
| 2 | `capture_runner.rs` / `probe_backend.rs` (lib tests) | unrelated |

## Estado de los 3 sub-cycles REC-C0.5

| Cycle | Issue | Status | Next action |
|---|---|---|---|
| REC-C0.5-C | #30 | **CLOSED LOCAL** | n/a |
| REC-C0.5-B | #29 | NOT STARTED | **mañana**, primero del día |
| REC-C0.5-A | #28 | NOT STARTED | tercero, último |

## REC-C0.5-B scope (mañana)

**Issue #29**: probe_inject capability-aware split. DoD:

- Test unprivileged debe inyectar/detectar capability real (no `#[ignore]`,
  no fake-pass) y verificar contrato tipado `Err(CapabilityUnavailable::EbpfUprobe)`.
- UAT privileged separada — fuera del CI sandbox run.
- 3 tests falling in CI ahora mismo:
  - `test_probe_inject_without_root_returns_error` (sin root)
  - `test_probe_inject_invalid_symbol` (símbolo vacío)
  - `test_probe_inject_before_pid_known` (sin PID)

## Branch / commit state

- Branch: `feat/rec-convergence-truth-gate`
- HEAD: `02c2a552` (pushed)
- Working tree: clean
- PR #19: OPEN, 4 checks RED, 1 GREEN

## Conventions reminder (AGENTS.md)

- **No `#[ignore]`** para tests capability-dependent.
- **No fabrication.** SHAs from `git rev-parse` only.
- **Build system expresses dependencies formally.** `build.rs` + `cargo:rerun-if-changed` + `cargo:rustc-env=`.
- **Centralize fixture API** en `FixtureResolver` (root + fixture).
- **Handoff en Español** bajo `cycle-artifacts/p-3416cfb8288f8964/handoffs/HANDOFF-*.md`.
- **Investigation notes en Inglés** bajo `cycle-artifacts/p-3416cfb8288f8964/rec-c0-5-*/notes.md`.
- **Issue #30 attack order**: #30 ✓ → #29 (mañana) → #28.
- **Pre-flight gate**: `sddk adopt status` (si aplica) + `git fetch origin main && git checkout main && git pull --ff-only`.

## DoD para REC-C0 closure (recap)

4 remote CI checks GREEN + local gates PASS:
1. fmt, 2. clippy, 3. clippy --all-features, 4. build, 5. test --lib --all-features,
6. test --tests, 7. check --all-features, 8. check_architecture_contracts.py,
9. run_coverage.sh.

## Quick start mañana

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos
git fetch origin main
git checkout feat/rec-convergence-truth-gate
git pull --ff-only
git log --oneline -5  # verify HEAD = 02c2a552
# Read chronos-sandbox/tests/probe_inject.rs to understand the 3 failing tests
# Read crates/chronos-ebpf + chronos-services for CapabilityUnavailable type
# Implement capability-aware split per issue #29 DoD
# Local verification: cargo test -p chronos-sandbox --test probe_inject
# Commit + push; verify remote CI Coverage moves toward green
# Then REC-C0.5-A (#28)
```
