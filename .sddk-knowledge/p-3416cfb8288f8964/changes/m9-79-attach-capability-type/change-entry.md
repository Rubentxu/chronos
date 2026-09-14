# Change: m9-79 attach capability type

## Summary

Distinguish the ptrace attach path in `CapabilitySnapshot::probe_type`. Before
this cycle, every `ChronosSessionLifecycleService::start` arm — including the
one produced by `session_start{action=attach}` — set `probe_type` to
`"ebpf_user"`, a literal that describes an eBPF user-probe, which is not what
an attach session actually runs. The new value is `"ptrace_attach"`, which
matches the actual backend: `chronos-native/src/probe_backend.rs::attach_to_process`
calls `PtraceTracer::attach` (nix ptrace).

The change is one literal, one matching test assertion, and two manual updates
(English + Spanish). `CapabilitySnapshot::probe_type` is `Option<String>`; no
type or wire change. Downstream readers compare the string, so the rename is
total: every attach session after this commit reports `ptrace_attach`.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-79-attach-capability-type` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `009b75037357d069775ad4d7a0661684e27fc9ca` |
| Head SHA | `f41abd4580d078a3f5f1255ffd543573a4fa702d` |
| Tag | `v0.7.81` |

## Subject

- base_sha: `009b75037357d069775ad4d7a0661684e27fc9ca`
- head_sha: `f41abd4580d078a3f5f1255ffd543573a4fa702d`
- diff_digest: `sha256:74cf3c5328402689af856e791208851334203dbe065dacc400e395b511000f2d`
- source commits: `917fcb0` (code + tests + manuals), `eba71a1` (handoff docs), `f41abd4` (merge)
- cycle: m9-79
- branch: `feat/m9-79-attach-capability-type`
- date: `2026-09-14T06:28Z`
- tag: `v0.7.81`
- findings_closed: 0
- findings_introduced: 0
- new tests: 0 (the cycle modified the assertion in the existing `test_session_start_attach_to_running_self`)

## Files changed

| Status | Path | Change |
|---|---|---|
| modified | `crates/chronos-services/src/session_lifecycle.rs` | `probe_type: Some("ebpf_user".to_string())` → `Some("ptrace_attach".to_string())` in the `attach` arm of `start` (line 198). One literal. |
| modified | `chronos-sandbox/tests/session_lifecycle.rs` | `test_session_start_attach_to_running_self` assertion updated to expect `Some("ptrace_attach")` (line 376). |
| modified | `docs/manual-ai/en/08-session-management.md` | Attach example now states the snapshot carries `probe_type: "ptrace_attach"`. |
| modified | `docs/manual-ai/es/08-gestion-sesiones.md` | Spanish mirror of the same sentence. |
| modified | `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` | "Session 2026-09-13T22:33Z" handoff section appended (103 lines) describing m9-77..m9-79 state and SDDK recovery instructions; included for traceability between the previous session and this one. |

## Cross-checks

- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean.
- T2: `cargo test -p chronos-services --lib --no-fail-fast` → 264 passed / 0 failed / 0 ignored.
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported):
  - `chronos-sandbox/tests/e2e_connectivity::test_mcp_server_starts_and_responds` → 1/1 (5.65 s)
  - `chronos-sandbox/tests/session_lifecycle::test_session_start_attach_to_running_self` → 1/1 (15.26 s)
- CC#12: `main_sha == head_sha == remote_tag_peel == f41abd4580d078a3f5f1255ffd543573a4fa702d` (verified on origin: `c3c69f6abf1cd21c0dbe72e98128043cc95097cd` → `f41abd4580d078a3f5f1255ffd543573a4fa702d`).
- CC#24 / CC#31: `verify-report.md` carries the `## Subject` table at the top; this manifest carries `## Cross-checks`.
- CC#33: `## Subject` is present in `change-entry.md`, `verify-report.md` and `verify-findings.json`.
- CC#39: `cycles/index.md` Total cycles updated 76 → 79 (m9-77..m9-79 added in the same archival sweep).
- CC#42: peel-match recorded in `release-receipt.md`.
- CC#51: `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/` exists with this cycle's full artifact set.
- CC#55: `verify-report.md` carries `## Files Inventory`.
- Vault drift gate (`scripts/check_vault_drift.sh`): pending — the regen tool regenerates manifest SHAs after this commit lands; CC#4's `affected set` for this cycle is m9-02 (index rows), m9-67, m9-68, m9-70, m9-71, m9-72, m9-73, m9-74, m9-75, m9-76 (source/index rows from cycles this batch shares files with) plus the new m9-79 manifest.

## Falsification evidence

The focused sandbox test `test_session_start_attach_to_running_self` covers
both m9-78 (detach safety: child process remains alive after `session_stop`) and
m9-79 (capability value: `probe_type == "ptrace_attach"`). The test passed in
isolation (`--test-threads=1`, 15.26 s) against a binary built from this commit.
`e2e_connectivity` confirms the server still starts and responds.

The cycle introduced zero new tests because the assertion that constrains the
new literal already exists; reverting the literal to `ebpf_user` would fail
the test on line 376.

## Follow-ups (deferred)

None new from this cycle. m9-77..m9-79 closed three attach-cycle items in a
single batch without introducing debt. The preserved low-severity follow-ups
remain unchanged:

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low)
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low)
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low)
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated)
- **Sandbox warm-up ordering** and **5+19 not-merged branches triage**: preserved.

Next roadmap candidate unchanged: **MS-PROPERTY-POLICY** on a fresh branch from
current `origin/main` — ownership of the four `observation_log` property-policy
functions in `chronos-domain`, A-min, T2.