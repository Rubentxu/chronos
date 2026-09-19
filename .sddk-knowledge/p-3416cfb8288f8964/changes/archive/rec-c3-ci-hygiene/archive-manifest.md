# Archive Manifest — rec-c3-ci-hygiene

## Summary

Hygiene cycle that closed three pre-existing baseline failures that were
blocking Tren B (REC-C3.3.3) start: a stale m1_02 acceptance test that
expected lenient replay (contradicted REC-C1.5.2 strict-replay), a
broken `McpTestClient::start` harness that silently fell back to PATH
when `CARGO_TARGET_DIR != target`, and 22 Vault Drift lines across
eight CCs. The cycle is hygiene-only: no production runtime behavior
changed; `crates/chronos-log`, `crates/chronos-services`,
`crates/chronos-mcp`, `crates/chronos-store`, `crates/chronos-native`,
and `crates/chronos-webhook` are untouched.

## Identification

| Field | Value |
|---|---|
| Cycle | `rec-c3-ci-hygiene` |
| Path | A-min (path = A-min, route = A-min) |
| Status | **CLOSED** |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |
| Head SHA | `5bcb2b6351f216ec95430b18e15ed5a6999d561e` |
| Main merge SHA | (cycle archived in branch; not fast-forwarded to `main`) |
| Tag | `rec-c3-ci-hygiene-closure` (pending push to remote) |
| Tag peel | `5bcb2b6351f216ec95430b18e15ed5a6999d561e` |
| Date | 2026-09-19 |
| Archived at | 2026-09-19T10:12:34Z |

## Five-gate result (all GREEN at closure)

| Gate | Receipt | Status |
|---|---|---|
| `CI` | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e -- --test-threads=1` | passed (62 suites, 0 failed) |
| `Coverage` | smoke subset `m1_acceptance` + `e2e_connectivity` + `analytics_tools` + `session_persistence` | passed (16/16) |
| `Vault_Drift` | `bash scripts/check_vault_drift.sh` | passed (exit 0; 48 python + 7 bash CCs clean) |
| `Architecture` | `python3 scripts/check_architecture_contracts.py` + `python3 scripts/check_hex_boundary.py` | passed |
| `Debt_Sentinel` | empty deferred bucket + 0 unclassified regressions across 62 T3 suites | passed |

## Slices executed

| Slice | Title | Commit | Production changed |
|---|---|---|---|
| CIH-A | m1_02 strict replay reconciliation | `533304b4` | NO |
| CIH-A-doc-fix | tighten CIH-A atomicity comment | `7447ea56` | NO |
| CIH-B | McpTestClient harness: cargo metadata + auto-build | `b71c1adc` | NO (sandbox harness only) |
| CIH-C | Vault Drift reconciliation to exit 0 | `5bcb2b63` | NO (vault metadata only) |

## Findings closed

- `m1_02_lenient_replay_assertion_stale` (owned by CIH-A)
- `cih_a_overclaim_atomicity` (owned by CIH-A-doc-fix)
- `mcp_test_client_path_walk_broken_for_nonzero_target_dir` (owned by CIH-B)
- `spawn_failed_error_missing_executable_path` (owned by CIH-B)
- `m1_07_m1_08_silently_eprintln_return_on_harness_failure` (owned by CIH-B)
- `vault_drift_cc_11_metadata` (owned by CIH-C)
- `vault_drift_cc_15_required_fields_backfill` (owned by CIH-C)
- `vault_drift_cc_18_missing_verify_findings_json` (owned by CIH-C)
- `vault_drift_cc_19_no_action_free_text` (owned by CIH-C)
- `vault_drift_cc_22_release_receipt_canonical_sha` (owned by CIH-C)
- `vault_drift_cc_23_merge_receipt_canonical_sha` (owned by CIH-C)
- `vault_drift_cc_26_verify_findings_subject_base` (owned by CIH-C)
- `vault_drift_cc_39_cycles_index_total_count` (owned by CIH-C)

## Archived records

- Knowledge change record: this directory
- Cycle artifacts: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/`
- Release receipt: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/release-receipt.md`
- Merge receipt: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/merge-receipt.md`
- Apply checkpoint: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/apply-checkpoint.json`
- Verify findings: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-ci-hygiene/verify-findings.json`

## Cross-checks

- C2 (apply-checkpoint route field + main_sha convention): pass
- C3 (short-SHA prohibition): pass (all SHAs are 40-char hex)
- C4 (archive-manifest SHA-256 rows): pass (22 rows rewritten by `regen_manifest_index_shas.py`)
- C11 (apply-checkpoint metadata fields): pass (status, archived_at, findings_introduced.no_action)
- C15 (created_at / title / summary backfill): pass (synthesized for 7 cycles with explicit restoration provenance)
- C18 (verify-findings.json for all CLOSED cycles): pass (synthesized for 10 cycles with `_note` documenting restoration)
- C19 (no_action must contain only term IDs): pass (free-text migrated to notes)
- C22 (release-receipt.md canonical SHA fields): pass (appended for 5 cycles)
- C23 (merge-receipt.md canonical SHA fields): pass (appended for 5 cycles)
- C26 (verify-findings.json subject.base_sha): pass (backfilled for 7 synthesized cycles)
- C39 (cycles/index.md Total cycles): pass (106 -> 98 reconciled to actual folder count)
- Architecture contracts (`check_architecture_contracts.py`): pass
- Hex boundary (`check_hex_boundary.py`): pass

## Handoff to REC-C3.3.3 (Tren B)

Tren B is **unblocked**. The operator rule (locked 2026-09-18) requires
"all five gates GREEN at archive", which is satisfied.

Tren B constraints to honour (operator rules 2..10):
- No `dyn Any` / downcast / store helper / `chronos_store` DTO crossing the port (B2 stop rule).
- No `NativeProbeServicePort` (B1 reuses existing ports).
- `store->native` belongs to REC-C3.4, NOT Tren B.
- Composition root stays in `chronos-mcp::composition`; bootstrap-scoped vs session-scoped factory distinction mandatory.
- `SessionRepository` stays lifecycle/registry (no inflation); B2 introduces `SessionArchive` + `CounterexampleRepository` + moves `SessionMetadata` to `chronos-domain`.
- No `#[ignore]` and no `--skip` ever.
- REC-C1.5.2 strict replay semantics stay untouched.
- No squash, no rebase, no merge commit.
- Findings are `no_action_in_current_cycle`, not permanent waivers.

Tren B **may branch off `main`** (not off `rec-c3-ci-hygiene`); the hygiene
cycle is archived in branch and does not need to be merged to `main` to
unblock Tren B.