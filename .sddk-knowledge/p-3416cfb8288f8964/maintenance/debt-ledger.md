# Pre-existing debt ledger (from REC-C4 verify, 2026-09-20)

Debt items found during REC-C4 verification that reproduce on `origin/main`
(75d04447) and are therefore NOT regressions of this cycle. Recorded per rule
B5 (no waivers — pre-existing failures get debt entries).

| ID | Source | Description | Repro |
|---|---|---|---|
| DEBT-C4-01 | vault drift CC#11 | `rec-c3.3-train-b` suspended cycle stuck in `apply_complete_pending_capture_session`, expected `CLOSED`. Suspended artifact dirs (`cycle-artifacts/_suspended-rec-c3.3-train-b/`) are untracked leftovers. | `bash scripts/check_vault_drift.sh` |
| DEBT-C4-02 | vault drift CC#18 | Missing `verify-findings.json` for cycles rec-c3-hexagonal-closure, rec-c3.3-train-b, rec-c3.3.4-native, rec-c3.5-residual-inversion, rec-c4-solid-connascence | same |
| DEBT-C4-03 | vault drift CC#22 | `rec-c3.3-train-b/release-receipt.md` missing Head SHA / Remote tag / tag_peel / Peel match fields | same |
| DEBT-C4-04 | vault drift CC#56 | `chronos-sandbox/src/client/identity.rs:151-164` mutates process env (`std::env::set_var`/`remove_var` for `CHRONOS_MCP_EXPECTED_SHA`). Test-only helper; pre-exists on main. **Closed in G0.3** (commit pending): refactored to `verify_expected_sha_value(Option<&str>)` explicit parameter; tests call it directly. | same |
| DEBT-G0.4-01 | C5.2 sandbox-client migration gap | Sandbox client `query_events` wrapper was reading `events` at inner JSON root, but server v2 publishes `events` nested in `result` envelope (refactor C5.2). **Closed in G0.4** (commit `b55efb8f`): wrapper now mirrors V2 envelope and reads `v2.result.events`. | n/a (closed) |
| DEBT-G0.4-02 | G0.2 regression in `tests/observe_uprobe.rs` | 2 tests added in G0.2 assumed `Ok(response_with_error_body)` but `RpcClient::call_tool` (`rpc.rs:153-165`) converts both JSON-RPC error envelopes AND MCP `result.isError=true` to `Err(RpcError)`. **Closed in G0.4** (commit `b55efb8f`): tests now `match Err(McpSandboxError::RpcError)` and assert error TYPE plus discriminator fragments. | n/a (closed) |
| DEBT-G0.5-01 | Legacy `offset_*` pagination tests | 3 tests in `chronos-sandbox/tests/query_filters.rs` (`test_query_events_offset_pagination`, `test_query_events_offset_beyond_total`, `test_query_events_limit_exact_pagination`) use `QueryFilter.offset > 0` which the wrapper at `tools.rs:670-674` explicitly rejects (pre-C5.2 pagination contract was replaced by opaque cursors in C5.2). **Ignored in G0.4** (commit `b55efb8f`, bodies preserved verbatim §0.4): `#[ignore = "G0.4: legacy pre-C5.2 offset pagination; migrate to cursor next_cursor (M1+)"]`. | `cargo test -p chronos-sandbox --test query_filters` (3 tests marked ignored) |
| DEBT-M7-02-01 | `probe_inject` legacy error prefix | 4 tests in `chronos-sandbox/tests/probe_inject.rs` expect pre-C5.2 error prefix `probe_inject: capability: ebpf-uprobe`, but the m7-02 wrapper migration emits v2 `observe: probe still starting up`. **Pre-existing failure** (verified on `main @ c81ca08c` before G0.4 merge, NOT a regression). m7-02 debt. **not_run per directiva** (M1+ follow-up). | `cargo test -p chronos-sandbox --test probe_inject` (4 failed pre-G0.4, persists post-G0.4) |
| DEBT-G0-04 | UAT-G0-04 (privileged uprobe in real host) | UAT-G0-04 requires root + ptrace kernel in the running environment. Cannot be executed in this sandbox. **not_run per directiva** (M1+ follow-up or external CI). | n/a (env-locked) |
| DEBT-VAULT-CC-PERMANENT | CC#11/CC#18/CC#22 in `rec-c3.3-train-b` | 3 vault drift CCs (CC#11 status check, CC#18 verify-findings, CC#22 release-receipt fields) report drift on `rec-c3.3-train-b` only. Suspended per directive del operador (m9-89 cycle, 2026-09-19). **not_run per directiva** — `rec-c3.3-train-b` is in `apply_complete_pending_capture_session` and requires a política decision distinta (NOT a code fix). | `bash scripts/check_vault_drift.sh` (1 line CC#11, 4 lines CC#22, 1 line CC#18 from this cycle only) |
| DEBT-VAULT-CC-REC-C3 | CC#18 in 4 `rec-c3.*` cycles | CC#18 reports 4 missing `verify-findings.json` in `rec-c3-hexagonal-closure`, `rec-c3.3.4-native`, `rec-c3.5-residual-inversion`, and `rec-c3.3-train-b` (already counted in DEBT-VAULT-CC-PERMANENT). The 3 non-train-b are out-of-scope for vault drift (WIP/local). **not_run per directiva**. | same |

These do not gate REC-C4 (verified pre-existing on main). DEBT-C4-04 closed
in G0.3 (commit pending). DEBT-G0.4-01 + DEBT-G0.4-02 closed in G0.4
(merged `b44504ed`). DEBT-G0.5-01 ignored in G0.4 (bodies preserved §0.4).
DEBT-M7-02-01 + DEBT-G0-04 + DEBT-VAULT-CC-PERMANENT + DEBT-VAULT-CC-REC-C3
stay `not_run` per directiva — visible in STATE, not deleted.

Candidate follow-up cycles:
- `m-rec-c4.5-vault-debt`: close DEBT-C4-01..03 (now DEBT-VAULT-CC-PERMANENT + DEBT-VAULT-CC-REC-C3).
- `m1-offset-cursor-migration`: close DEBT-G0.5-01 (3 ignored `offset_*` tests).
- `m1-probe-inject-error-prefix`: close DEBT-M7-02-01 (4 probe_inject tests with stale prefix).
- `m1-uat-g0-04-privileged`: close DEBT-G0-04 (requires privileged environment).
