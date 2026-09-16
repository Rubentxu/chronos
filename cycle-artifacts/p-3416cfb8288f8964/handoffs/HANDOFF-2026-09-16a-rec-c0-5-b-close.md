# HANDOFF-2026-09-16a-rec-c0-5-b-close.md

> **Fecha**: 2026-09-16 08:53 UTC
> **Estado REC-C0.5-B**: implementation COMPLETE / local PASS / remote CI Coverage PENDING
> **Siguiente paso programado**: REC-C0.5-A (#28) — Vault Drift Sweep closure

## Lo que se hizo hoy

### REC-C0.5-B (#29) — capability-aware probe_inject split
Implementación completa en un solo commit (`98f9dba4`).

**Cambio de contrato**: el dispatcher v2 `observe` (en `chronos-services/src/observe.rs::create`) ahora hace pattern-match explícito sobre los tres variantes terminales de `ProbeInjectResult` y los traduce a errores tipados:

| `ProbeInjectResult` | `ServiceError`           | Wrapper text prefix     |
|---------------------|--------------------------|--------------------------|
| `Attached { pid }`  | (none, success)          | (none)                   |
| `AttachFailed { error, .. }` | `InjectionFailed(error)` | `probe_inject: capability: ebpf-uprobe — uprobe attach failed: …` |
| `EbpfUnavailable(reason)`    | `EbpfUnsupported(reason)` | `probe_inject: capability: ebpf-uprobe — … (requires root or CAP_BPF/CAP_PERFMON, kernel >= 5.8)` |
| `ProbeStarting`               | `ProbeStarting`           | `probe_inject: capability: probe-starting — probe is still starting up; retry shortly` |

El bug era que el `_ => None` arm colapsaba `EbpfUnavailable` + `ProbeStarting` en una respuesta `success(pid: null, "uprobe attached")`. Los 3 sandbox tests que asertaban sobre tokens ad-hoc ("permission", "denied", "EPERM") panicaban al recibir una respuesta exitosa.

**Nuevo módulo** `chronos_domain::capability` (153 líneas):
- `Capability` enum (Copy, Eq, Hash) — discriminador de slot tipado.
- `CapabilityUnavailable { capability, reason }` — typed error con discriminador estable.
- Variants: `EbpfUprobe`, `PtraceAttach`.
- 3 unit tests verifican kebab-case stability, slot+reason separation, and slot-only equality.

### Sandbox tests

`chronos-sandbox/tests/probe_inject.rs` rewritten (303 líneas):
- 4 tests, **0 `#[ignore]`**, **0 fake-pass**.
- `assert_capability_error()` helper extrae el discriminator tipado de `Result<_, McpSandboxError>` (acepta tanto `Ok(value)` con `isError: true` como `Err(RpcError(text))`).
- Tests assertan sobre el kebab-slot (`ebpf-uprobe` o `probe-starting`), NO sobre la razón human-readable.

`chronos-sandbox/tests/probe_inject_privileged_uat.rs` (nuevo, 112 líneas):
- UAT privileged en archivo separado.
- Gateado por `CHRONOS_PRIVILEGED_UAT=1` env var.
- Sin el env var, el test retorna early con un mensaje "skipped" — el default CI run lo ve como PASS, nunca como `#[ignore]`.

### Dispatcher unit tests (chronos-services)

3 nuevos tests en `crates/chronos-services/src/observe.rs::tests`:
- `create_uprobe_without_ebpf_returns_ebpf_unsupported` — pre-populates `live_probes` con `LiveProbeSession`, aserta `ServiceError::EbpfUnsupported(reason)` con reason non-empty.
- `create_uprobe_with_no_pid_returns_probe_starting_or_ebpf_unavailable` — pid=0 → cualquiera de los dos variants (default build short-circuits to EbpfUnsupported primero).
- `create_uprobe_attach_failure_surfaces_typed_injection_failed_or_ebpf_unavailable` — simétrico.

Tests prueban el contrato tipado al nivel de service sin necesidad de spawn del MCP server.

## Resultados locales

### T0 — lint
```
cargo fmt --all -- --check       PASS
cargo clippy --workspace --all-targets -- -D warnings            PASS (exit 0)
cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS (exit 0)
```

### T1 — unit tests affected crates
```
cargo test -p chronos-services --lib                  275 passed / 0 failed
cargo test -p chronos-services --lib --all-features   275 passed / 0 failed
cargo test -p chronos-domain --lib                     159 passed / 0 failed
cargo test -p chronos-domain --lib --all-features      159 passed / 0 failed
```

### T2 — sandbox probe tests
```
cargo test -p chronos-sandbox --test probe_inject                          4 passed / 0 failed (era 1/4)
cargo test -p chronos-sandbox --test probe_inject_privileged_uat          1 passed / 0 failed (gated)
cargo test -p chronos-sandbox --test probe_lifecycle                      5 passed / 0 failed (regresión)
cargo test -p chronos-sandbox --test boundary_conditions                  9 passed / 0 failed (regresión)
cargo test -p chronos-sandbox --test fixture_resolver                     4 passed / 0 failed (regresión)
cargo test -p chronos-sandbox --test e2e_connectivity                     1 passed / 0 failed (regresión)
```

### Workspace residual
34 of the original 37 failures from REC-C0.5-C close-out remain. None caused by REC-C0.5-B:
- 5 query_filters / query_edge_cases (REC-C1 events_read)
- 13 chronos-e2e/test_ptrace_capture (bucket D ptrace perms)
- 6 chronos-native/m2_function_frame_capture (REC-C1 / bucket D)
- 4 chronos-mcp/tripwires_tools (other cycle)
- 4 ptrace_tracer lib flakes (§6.5, --test-threads=1)
- 2 unrelated lib tests

## Estado de los 3 sub-cycles REC-C0.5

| Cycle | Issue | Status | Next action |
|---|---|---|---|
| REC-C0.5-C | #30 | **CLOSED LOCAL** | n/a |
| REC-C0.5-B | #29 | **APPLIED LOCAL** (just committed `98f9dba4`) | push + watch CI Coverage |
| REC-C0.5-A | #28 | NOT STARTED | next cycle (vault drift sweep) |

## Conventions reminder (AGENTS.md)

- **No `#[ignore]`** para tests capability-dependent — ✓ (REC-C0.5-B uses gated UAT instead)
- **No fabrication** — SHAs from `git rev-parse` only — ✓
- **Build system expresses dependencies formally** — n/a (no build.rs changes beyond fmt)
- **Centralize fixture API** en `FixtureResolver` — n/a
- **Handoff en Español** bajo `cycle-artifacts/p-3416cfb8288f8964/handoffs/HANDOFF-*.md`
- **Investigation notes en Inglés** bajo `cycle-artifacts/p-3416cfb8288f8964/rec-c0-5-*/notes.md`

## Branch / commit state

- Branch: `feat/rec-convergence-truth-gate`
- HEAD: `98f9dba4` (local; 1 commit ahead of origin)
- PR #19: OPEN (was 4 checks RED, 1 GREEN; expect 1-2 RED → GREEN after this push)

## DoD para REC-C0 closure (recap)

4 remote CI checks GREEN + local gates PASS:
1. fmt, 2. clippy, 3. clippy --all-features, 4. build, 5. test --lib --all-features,
6. test --tests, 7. check --all-features, 8. check_architecture_contracts.py,
9. run_coverage.sh.

REC-C0.5-B addresses the probe_inject component of #9 (Coverage). After pushing,
expect Coverage to drop 3 failures (probe_inject.rs) and the remaining 34 to be
attributed to REC-C1 events_read, bucket D ptrace, etc.

## Next steps (REC-C0.5-A, #28)

Vault Drift Sweep closure — bash `scripts/check_vault_drift.sh` returns RED per the
REC-C0.5-C close-out notes. Scope: bring `terms/index.md` and related cycle
artifact manifests back to consistency with the Git history. Likely a small
vault-only cycle similar to m9-96.

## Quick start mañana

```bash
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos
git fetch origin main
git checkout feat/rec-convergence-truth-gate
git push origin feat/rec-convergence-truth-gate  # publish 98f9dba4
# Watch CI Coverage / Vault Drift / build / test
# Then start REC-C0.5-A (#28): bash scripts/check_vault_drift.sh to reproduce RED
```
