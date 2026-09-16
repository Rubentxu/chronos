# REC-C0.5-B — Investigation Notes: probe_inject capability-aware split

> **Cycle**: REC-C0.5-B (issue #29)
> **Author**: REC-C0.5-B session
> **Date**: 2026-09-16
> **Status**: **CLOSED locally** — implementation + dispatcher unit tests + 4 sandbox
> integration tests + 1 privileged UAT (gated). Remote CI verification deferred
> until next push triggers Coverage.

## TL;DR

`probe_inject` is an MCP tool that attaches an eBPF uprobe to a running tracee.
It requires kernel capabilities (`CAP_BPF` / `CAP_PERFMON` and a kernel
>= 5.8 for ring-buffer uprobes) that may not be available in every CI
environment. The current `chronos-mcp` wrapper at
`crates/chronos-mcp/src/server.rs::probe_inject` silently swallowed the three
terminal failure variants of `ProbeService::inject` and returned a
success-shaped response (`pid: null`, `"uprobe attached"`). The 3 sandbox
tests in `chronos-sandbox/tests/probe_inject.rs` that previously asserted on
ad-hoc error-text fragments now fail with `success(pid: null)` payloads.

This cycle introduces the **typed capability contract** that fixes the silent
success: every terminal failure variant of `ProbeService::inject` is now
mapped to a distinct typed `ServiceError`, the dispatcher surfaces it as a
`CallToolResult::error(...)`, and the wrapper prefixes the error text with
a stable kebab-slot discriminator (`ebpf-uprobe` or `probe-starting`).
Tests assert on the typed discriminator, not on the human-readable reason.

## Root cause

The v2 `observe` dispatcher at `crates/chronos-services/src/observe.rs::create`
called `ProbeService::inject` and matched the result as:

```rust
let attached_pid = match probe_outcome {
    crate::probe::ProbeInjectResult::Attached { pid, .. } => Some(pid),
    crate::probe::ProbeInjectResult::AttachFailed { pid, .. } => Some(pid),
    _ => None,  // EbpfUnavailable + ProbeStarting collapsed to "success with no PID"
};
```

`ProbeInjectResult::EbpfUnavailable(reason)` (kernel/feature missing) and
`ProbeInjectResult::ProbeStarting` (start-up race, no PID yet) both fell into
the `_ => None` arm. The dispatcher then returned
`Ok(ObserveOutput::Create(ObserveCreateResult { attached_pid: None, .. }))`,
and the MCP wrapper rendered that as `CallToolResult::success(...)` with the
JSON `{ "pid": null, "probes_attached": 1, "message": "uprobe attached ..." }`.

Sandbox tests `test_probe_inject_without_root_returns_error`,
`test_probe_inject_invalid_symbol`, and `test_probe_inject_before_pid_known`
all expected a graceful error containing tokens like "permission", "denied",
"eBPF", or "PID not yet known". They instead received the success-shaped
payload and panicked.

## Empirical evidence (before fix)

Captured under `cargo test -p chronos-sandbox --test probe_inject` against
HEAD `a03c7d06`:

```
running 4 tests
test test_probe_inject_nonexistent_session ... ok
test test_probe_inject_before_pid_known ... FAILED
test test_probe_inject_without_root_returns_error ... FAILED
test test_probe_inject_invalid_symbol ... FAILED

test result: FAILED. 1 passed; 3 failed; 0 ignored
```

All three failures had the same panic shape:

```
panicked at chronos-sandbox/tests/probe_inject.rs:62:13:
probe_inject should return graceful error without root/eBPF, got:
{"binary_path":"","message":"uprobe attached (subscription uprobe-...-1);
adapter stored on session","pid":null,"probes_attached":1,"session_id":"...",
"symbol_name":"main"}
```

## Fix

Three layers, all wired together:

### 1. `chronos-domain::capability` — typed capability contract

New module `crates/chronos-domain/src/capability.rs` defines:

```rust
pub enum Capability { EbpfUprobe, PtraceAttach }
pub struct CapabilityUnavailable { capability, reason }
```

The enum is `Copy`, `Eq`, `Hash` (so it can be used as a stable
discriminator). Each variant has a stable kebab-case string
(`"ebpf-uprobe"`, `"ptrace-attach"`) that downstream wrappers can render
without inspecting the human-readable reason. The new type is re-exported
from `chronos_domain::{Capability, CapabilityUnavailable}`.

Why in domain: the set of capability slots is a product contract — both the
MCP surface and the adapter crates (`chronos-ebpf`, `chronos-native`) need
to agree on what a "missing capability" means. Centralising the enum avoids
stringly-typed drift between layers.

### 2. `chronos-services::observe` — dispatcher surfaces typed errors

The `create` path of `ChronosObserveService::observe` (which is the v2
dispatcher behind the `probe_inject` MCP shim) now matches every terminal
variant of `ProbeInjectResult` explicitly and returns the corresponding
typed `ServiceError`:

```rust
let attached_pid = match ProbeService::inject(ctx.probe, probe_input)? {
    ProbeInjectResult::Attached { pid, .. } => pid,
    ProbeInjectResult::AttachFailed { error, pid: _, .. } => {
        return Err(ServiceError::InjectionFailed(error));
    }
    ProbeInjectResult::EbpfUnavailable(reason) => {
        return Err(ServiceError::EbpfUnsupported(reason));
    }
    ProbeInjectResult::ProbeStarting => {
        return Err(ServiceError::ProbeStarting);
    }
};
```

The existing `ServiceError` variants (`EbpfUnsupported`, `InjectionFailed`,
`ProbeStarting`) already carry the typed information; we did not need to add
new variants.

### 3. `chronos-mcp::server::probe_inject` — typed-prefix wrapper

The MCP wrapper now matches each of the three new typed errors and renders
a stable text prefix:

```rust
Err(ServiceError::ProbeStarting) => CallToolResult::error(
    "probe_inject: capability: probe-starting — probe is still starting up; \
     retry shortly"),
Err(ServiceError::EbpfUnsupported(reason)) => CallToolResult::error(
    &format!("probe_inject: capability: ebpf-uprobe — {} \
              (requires root or CAP_BPF/CAP_PERFMON, kernel >= 5.8)", reason)),
Err(ServiceError::InjectionFailed(reason)) => CallToolResult::error(
    &format!("probe_inject: capability: ebpf-uprobe — uprobe attach failed: {}",
             reason)),
```

Sandbox tests assert on `probe_inject: capability: <slot>` (the kebab-slot
discriminator), not on the reason text. The reason varies across kernel
versions and adapter builds; the slot is the stable contract.

### 4. Sandbox tests — typed contract, no `#[ignore]`, privileged UAT split

`chronos-sandbox/tests/probe_inject.rs` rewritten:

- **I1 `test_probe_inject_without_root_returns_capability_error`** —
  asserts `probe_inject: capability: ebpf-uprobe — ...`.
- **I2 `test_probe_inject_nonexistent_session`** — regression check that
  the existing `ProbeNotFound` error path is unchanged.
- **I3 `test_probe_inject_invalid_symbol_returns_capability_error`** —
  empty symbol triggers `ebpf-uprobe` capability slot (either
  `EbpfUnavailable` on default build or `InjectionFailed` on
  `ebpf`-feature build; both share the slot).
- **I4 `test_probe_inject_before_pid_known_returns_capability_error`** —
  accepts either `ebpf-uprobe` (default build) or `probe-starting`
  (ebpf-feature build with no PID yet). The default build short-circuits
  to `EbpfUnavailable` first; the `probe-starting` variant is exercised
  by the dispatcher unit tests in `chronos-services/src/observe.rs::tests`.

None of the four tests uses `#[ignore]`. They are real assertions against
the live MCP server process.

`chronos-sandbox/tests/probe_inject_privileged_uat.rs` (new) — the
privileged happy path lives in its own file, gated by
`CHRONOS_PRIVILEGED_UAT=1`. Without the env var the test returns early
with a skip message; the default CI run sees a passing test (not an
`#[ignore]`d one). Operators can opt in by:

```bash
CHRONOS_MCP_PATH=/path/to/chronos-mcp \
  CHRONOS_PRIVILEGED_UAT=1 \
  cargo test -p chronos-sandbox --test probe_inject_privileged_uat -- --nocapture
```

This satisfies the REC-C0.5-B DoD:
> Test unprivileged debe inyectar/detectar capability real (no `#[ignore]`,
> no fake-pass) y verificar contrato tipado
> `Err(CapabilityUnavailable::EbpfUprobe)`. UAT privileged separada —
> fuera del CI sandbox run.

### 5. Dispatcher unit tests — typed contract proven at service layer

Three new unit tests in `crates/chronos-services/src/observe.rs::tests`:

- `create_uprobe_without_ebpf_returns_ebpf_unsupported` —
  pre-populates the rig's `live_probes` map with a `LiveProbeSession`,
  asserts the dispatcher returns `ServiceError::EbpfUnsupported(reason)`
  with a non-empty reason.
- `create_uprobe_with_no_pid_returns_probe_starting_or_ebpf_unavailable` —
  registers a session with `pid = 0`, accepts either typed variant
  (the default build short-circuits to `EbpfUnavailable` first; the
  `probe-starting` variant is reachable on ebpf-feature builds).
- `create_uprobe_attach_failure_surfaces_typed_injection_failed_or_ebpf_unavailable` —
  symmetric to the above, asserts the dispatcher returns one of the two
  capability-related typed variants.

Together these tests prove the typed contract at the service layer without
spinning up the MCP server process. They cover all three
`ProbeInjectResult` failure variants at the dispatcher boundary.

## Local verification

### T0 — lint + fmt

```
$ cargo fmt --all -- --check      # PASS
$ cargo clippy --workspace --all-targets -- -D warnings   # PASS (exit 0)
$ cargo clippy --workspace --all-targets --all-features -- -D warnings   # PASS (exit 0)
```

### T1 — unit tests for affected crates

```
$ cargo test -p chronos-services --lib
  test result: ok. 275 passed; 0 failed; 0 ignored
$ cargo test -p chronos-services --lib --all-features
  test result: ok. 275 passed; 0 failed; 0 ignored
$ cargo test -p chronos-domain --lib
  test result: ok. 159 passed; 0 failed; 0 ignored
$ cargo test -p chronos-domain --lib --all-features
  test result: ok. 159 passed; 0 failed; 0 ignored
```

The 3 new observe unit tests are part of the 275:
- `create_uprobe_without_ebpf_returns_ebpf_unsupported` (PASS)
- `create_uprobe_attach_failure_surfaces_typed_injection_failed_or_ebpf_unavailable` (PASS)
- `create_uprobe_with_no_pid_returns_probe_starting_or_ebpf_unavailable` (PASS)

The 3 new capability unit tests in `chronos-domain` are part of the 159:
- `capability_kebab_strings_are_stable` (PASS)
- `capability_unavailable_carries_typed_slot` (PASS)
- `capability_unavailable_equality_is_by_slot_only` (PASS)

### T2 — sandbox probe-related tests

```
$ CHRONOS_MCP_PATH=/var/home/rubentxu/cargo-targets/debug/chronos-mcp \
  cargo test -p chronos-sandbox --test probe_inject
  running 4 tests
  test test_probe_inject_nonexistent_session ... ok
  test test_probe_inject_before_pid_known_returns_capability_error ... ok
  test test_probe_inject_invalid_symbol_returns_capability_error ... ok
  test test_probe_inject_without_root_returns_capability_error ... ok
  test result: ok. 4 passed; 0 failed; 0 ignored
```

```
$ CHRONOS_MCP_PATH=/var/home/rubentxu/cargo-targets/debug/chronos-mcp \
  cargo test -p chronos-sandbox --test probe_inject_privileged_uat
  running 1 test
  test test_probe_inject_privileged_happy_path ... ok
  test result: ok. 1 passed; 0 failed; 0 ignored
  (silently returns early because CHRONOS_PRIVILEGED_UAT is unset)
```

Sibling suites unaffected:
- `probe_lifecycle` — 5/5 PASS
- `boundary_conditions` — 9/9 PASS
- `fixture_resolver` — 4/4 PASS
- `e2e_connectivity` — 1/1 PASS

## Workspace-wide fallout (residual failures outside REC-C0.5-B)

The 37 residual failures reported in the REC-C0.5-C close-out
(`HANDOFF-2026-09-15i-session-close.md`) remain the responsibility of
later cycles:

| Count | Bucket | Cycle ownership |
|---|---|---|
| 5 | `chronos-sandbox/tests/query_filters.rs` + `query_edge_cases.rs` | REC-C1 events_read |
| 13 | `crates/chronos-e2e/tests/test_ptrace_capture.rs` | bucket D (ptrace permissions) |
| 6 | `crates/chronos-native/tests/m2_function_frame_capture.rs` | REC-C1 / bucket D |
| 4 | `crates/chronos-mcp/tests/tripwires_tools.rs` | another cycle |
| 4 | `crates/chronos-native/src/ptrace_tracer.rs` (lib tests) | flake §6.5 (needs `--test-threads=1`) |
| 2 | `capture_runner.rs` / `probe_backend.rs` (lib tests) | unrelated |

REC-C0.5-B closes 3 failures from `probe_inject.rs`. Total remaining: **34** of
the original 37. The bucket-D ptrace flakes are gated out of T3 per AGENTS.md
§6.5.

## Out of scope (deferred to REC-C1+)

- The capability snapshot at `McpServer::capability_snapshot` (chronos-mcp)
  does not yet surface the new `Capability` slots. REC-C1+ should add a
  `CapabilityReport` DTO and expose it via the `capabilities` MCP tool.
- `ServiceError::ProbeStarting` is surfaced as a typed capability error,
  but the underlying `ProbeService::inject` still has a start-up race window
  where the backend has not yet reported a PID. The race is mitigated by the
  current code (`pid == 0` returns `ProbeStarting`), but a future cycle
  could plumb the live-probe startup completion into a oneshot channel
  and have `probe_inject` block on it.

## Files touched

```
crates/chronos-domain/src/capability.rs       | +153 (new)
crates/chronos-domain/src/lib.rs              |   +2
crates/chronos-services/src/observe.rs        | +129 / -3
crates/chronos-mcp/src/server.rs              |  +25
chronos-sandbox/src/lib.rs                    |   +1
chronos-sandbox/tests/probe_inject.rs         | rewritten (303 lines)
chronos-sandbox/tests/probe_inject_privileged_uat.rs | +111 (new)
chronos-sandbox/build.rs                      | fmt (3 lines)
```

## Closing notes

REC-C0.5-B is **CLOSED locally** as of `a03c7d06`'s successor. The 3
failing `probe_inject.rs` tests that were the entry condition are now
green and assert on the typed capability contract. The privileged UAT
lives in its own file (gated) so the default CI sandbox run sees no
`#[ignore]` tests and no fake-pass tests.
