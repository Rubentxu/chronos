# Archive Manifest — m9-79-attach-capability-type

## Summary

m9-79 closes the last remaining item of the m9-77..m9-79 attach batch: the
`CapabilitySnapshot::probe_type` value reported after
`session_start{action=attach}` was the literal `"ebpf_user"`, an eBPF label
that describes a different backend. The cycle renames it to `"ptrace_attach"`,
which matches what `chronos-native/src/probe_backend.rs::attach_to_process`
actually runs (nix ptrace via `PtraceTracer::attach`). One source literal,
one matching sandbox assertion, two manual updates (en + es).

The batch is now self-consistent: m9-77 wired `session_start{action=attach}`
through native, services, dispatcher and MCP; m9-78 made `session_stop`
safe to detach from a traced target; m9-79 names the capability snapshot
correctly. Together they put `session_start{action=attach}` on the same
footing as the original `start`/`stop` contract — the only remaining gap is
documentation, and both manuals now describe it accurately in two languages.

No deferred findings introduced. No new tests added (the assertion that
constrains the new literal already existed).

## Cycle

| Campo | Valor |
|---|---|
| Cycle | `m9-79-attach-capability-type` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `009b75037357d069775ad4d7a0661684e27fc9ca` |
| Head SHA | `f41abd4580d078a3f5f1255ffd543573a4fa702d` |
| Branch | `feat/m9-79-attach-capability-type` |
| Tag | `v0.7.81` |
| Route | B-direct |
| Date | 2026-09-14 |

## Deliverables

| Artifact | Kind |
|---|---|
| `crates/chronos-services/src/session_lifecycle.rs` | one literal in the `attach` arm of `start` (line 198) |
| `chronos-sandbox/tests/session_lifecycle.rs` | matching assertion in `test_session_start_attach_to_running_self` (line 376) |
| `docs/manual-ai/en/08-session-management.md` | attach example now states `probe_type: "ptrace_attach"` |
| `docs/manual-ai/es/08-gestion-sesiones.md` | Spanish mirror of the same sentence |
| `.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md` | appended "Session 2026-09-13T22:33Z" handoff section (103 lines) for context between the previous session and this one |

## Evidence bindings

- **`apply-checkpoint.json`**: status CLOSED; verify_status passed; release_status released; archive_status archived
- **`verify-findings.json`**: 0 findings (empty `findings: []`)
- **`verify-report.md`**: Verdict PASS, 0 critical / 0 warnings; `## Files Inventory`, `## Cross-checks`, `## Behavioral Compliance` all present
- **`merge-receipt.md`**: Base SHA `009b750`, merge `f41abd4` (--no-ff), `HEAD == origin/main == f41abd4`
- **`release-receipt.md`**: `Remote tag | v0.7.81`, peel `f41abd4580d078a3f5f1255ffd543573a4fa702d`, local == remote
- **`gate-implementation-complete-35eb8158c41aa3fb-1`** (passed): build phase complete
- **`gate-tests-pass-b4e15865a16e42dd-1`** (passed): verify phase complete
- **`gate-policy-compliant-b4e15865a16e42dd-1`** (passed): verify phase complete
- **`gate-no-pending-effects-1e318b5e3422f853-1`** (passed): release phase complete
- **`gate-release-uat-approved-1e318b5e3422f853-1`** (passed): release phase complete

## Falsification evidence

No new code path is introduced, so falsification reduces to "revert the
literal, observe the test fail, restore, re-run green":

| Reverted | Observed result |
|---|---|
| `probe_type: Some("ebpf_user".to_string())` restored at session_lifecycle.rs:198 | `chronos-sandbox/tests/session_lifecycle::test_session_start_attach_to_running_self` fails with `assertion failed: Some("ebpf_user") == Some("ptrace_attach")` (line 376) |
| `Some("ebpf_user")` restored at session_lifecycle.rs:376 | test fails with the literal mismatch, same assertion |
| `Some("ptrace_attach")` re-applied at session_lifecycle.rs:198 AND 376 | test passes (1/1, 15.26 s) |

The m9-78 detach safety (verified by the same test: child process remains
alive after `session_stop`) is unchanged by this cycle.

## Tangential modifications

- **Vault indexes**: `cycles/index.md` rows for `m9-77`, `m9-78`, `m9-79`
  added; `terms/index.md` gained a "Findings deferred from m9-77..m9-79"
  section (empty: no new findings) and bumped `Last updated` /
  `Last archive`. Total cycles count went 76 → 79.
- **Archive-manifest CC#4 affected set**: m9-02 (index rows), m9-67,
  m9-68, m9-70, m9-71, m9-72, m9-73, m9-74, m9-75, m9-76 (source/index rows
  for shared files), plus the new m9-79 manifest. Regenerated to a fixpoint
  by `scripts/regen_manifest_index_shas.py` after the artifacts commit
  lands; the self-referential rows of pre-m9-11 manifests and m9-67/m9-68
  are preserved by design.

## Cross-checks

- CC#1..CC#56: pass (48 python + 7 bash, counts unchanged).
- CC#4: artifact index SHA-256 consistency is satisfied by the regen tool
  (`scripts/regen_manifest_index_shas.py`); the post-release commit runs
  it to fixpoint.
- CC#12: `main_sha == head_sha == remote_tag_peel == f41abd4580d078a3f5f1255ffd543573a4fa702d`.
- CC#21 (`## Evidence bindings` in archive-manifest): this section.
- CC#24 / CC#31 (`## Cross-checks`): present in `change-entry.md`,
  `verify-report.md`, and this manifest.
- CC#33 (`## Subject`): present in `change-entry.md`, `verify-report.md`
  and `verify-findings.json`.
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path`, `## Files
  Inventory` and the canonical summary table.
- CC#42: peel-match recorded in `release-receipt.md`.
- CC#51: cycle-artifacts folder exists for the m9-79 row.
- T0: `cargo fmt --all -- --check` clean; `cargo clippy --workspace
  --all-targets -- -D warnings` clean.
- T2: `cargo test -p chronos-services --lib --no-fail-fast` → 264 passed.
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported):
  - `chronos-sandbox/tests/e2e_connectivity::test_mcp_server_starts_and_responds` → 1/1 (5.65 s)
  - `chronos-sandbox/tests/session_lifecycle::test_session_start_attach_to_running_self` → 1/1 (15.26 s)

## Follow-ups (deferred)

None new. m9-77..m9-79 closed three attach-cycle items in one batch with no
debt. Preserved low-severity items:

- **FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE** (low)
- **FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL** (low)
- **FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION** (low)
- **FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION** (low, mitigated)
- **Sandbox warm-up ordering** (preserved)
- **5+19 not-merged branches triage** (preserved): human review needed.

Next roadmap candidate: **MS-PROPERTY-POLICY** (A-min, T2) — ownership of
the four `observation_log` property-policy functions in `chronos-domain`,
one domain owner/re-export or replacement callsites plus a shared
chronos-domain fixture. Branch `feat/ms-property-policy` from
`origin/main`.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-79-attach-capability-type/archive-manifest.md` | `8ab56f6c363795e1187546388665e34eef17cfaeec814f3e15d21e945c6d2c48` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/implementation-receipt.md` | `a908ad3ded6fc9c1876aaf398416bf6bf2bdd48e85ba05b6541996a2057984ff` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/verify-report.md` | `f3dc81ca8ae0ddb978c2da1d7fc788a1e9b35ed7e8b1dc158fe0f573df2f9144` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/verify-findings.json` | `903af88120af21aaac8e98b87298719b43457ae7fd9a641e8c31edf9078eb6ee` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/release-receipt.md` | `d36c302ff497de56bb62d9e63e8cbe3fb174c63c4a2ea4ad6fdeff6e52fa6ec9` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-79-attach-capability-type/merge-receipt.md` | `8a8df2b6c0c0163d9d3a3089b351b9253448f6418642fc6fca03c478c8d7dc99` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-79-attach-capability-type/change-entry.md` | `248dd43621039b4ff71807718c52fcc60f311d3ba02a05503340c1ec7743d772` |
| vault index (cycles) | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `2abe826f997082a6423ebc4f487782b92630a82ab695d9a3e698868e82313e4e` |
| vault index (terms) | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `03221130e4cfca65f7b145f2907767c3c01a4e27a966af20dfec38249774de8d` |
| source (renamed literal) | `crates/chronos-services/src/session_lifecycle.rs` | (1 line changed at line 198) |
| test (updated assertion) | `chronos-sandbox/tests/session_lifecycle.rs` | (1 line changed at line 376) |
| docs (manual EN) | `docs/manual-ai/en/08-session-management.md` | (4 lines rewrapped) |
| docs (manual ES) | `docs/manual-ai/es/08-gestion-sesiones.md` | (6 lines) |