# Release receipt — rec-c3-ci-hygiene

> Hygiene cycle: pre-existing baseline failures on origin/main blocked Tren B
> (REC-C3.3.3) start. Originally closed 2026-09-19T09:35:00Z at the historical
> anchor `2e761d4f76dc4c7b2a78972753770ac2aedf0357` (CIH-A/B/C). REOPENED
> 2026-09-19T16:14:00Z after CIH-D (commits e82921c9 + 8fdc3ef4) added the
> `PositionBeforeRetention` -> `ServiceError::CursorStale` translator fix.
> Cycle merge head remains `2e761d4f76dc4c7b2a78972753770ac2aedf0357` per
> operator rule #8 and CC#12 m9-19 convention. Tren B (REC-C3.3.3) unblock
> requires both: (a) CIH-D integrated into main, AND (b) all five GH Actions
> workflows GREEN on the integrated HEAD. (a) is satisfied locally; (b) is
> PARTIAL at HEAD 8fdc3ef4 (3/5 GREEN, 2/5 with PRE-EXISTING failures
> documented below). Re-closure is blocked until REC-C1.5 owns the
> multi_session failure and REC-C3.3.x owns the tripwire_tools failure.

## Released artifacts

- Branch: `rec-c3-ci-hygiene`
- Cycle merge base: `fa5eb582`
- Cycle merge head: `2e761d4f76dc4c7b2a78972753770ac2aedf0357` (immutable per operator rule #8)
- Post-CIH-D remote HEAD: `8fdc3ef40dc307d861e16e9bdf2e178431dacddc`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c3-ci-hygiene`
- Cycle status: REOPENED (post-CIH-D) — see remote verification below

## Five-gate result (locally GREEN at historical anchor; PARTIAL at post-CIH-D HEAD)

### Locally at HEAD 2e761d4f (CIH-C closure)

| Gate | Receipt | Status |
|---|---|---|
| `CI` | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e -- --test-threads=1` | passed (62 suites, 0 failed) |
| `Coverage` | smoke subset `m1_acceptance` + `e2e_connectivity` + `analytics_tools` + `session_persistence` | passed (16/16) |
| `Vault_Drift` | `bash scripts/check_vault_drift.sh` | passed (exit 0; 48 python + 7 bash CCs clean) |
| `Architecture` | `python3 scripts/check_architecture_contracts.py` + `python3 scripts/check_hex_boundary.py` | passed |
| `Debt_Sentinel` | empty deferred bucket + 0 unclassified regressions across 62 T3 suites | passed |

### Remotely at HEAD 8fdc3ef4 (CIH-D reopen, 2026-09-19T16:14:00Z)

| Gate | Status | Run ID | Detail |
|---|---|---|---|
| `Vault_Drift` | ✓ success | 35453324991 | 13s, all clean |
| `Architecture_Contracts` | ✓ success | 35453324922 | all clean |
| `Sandbox_Debt_Sentinel` | ✓ success | 35453324927 | all clean |
| `CI` | ✗ failure | 35453324929 | PRE-EXISTING: `chronos-sandbox::multi_session::test_drop_session_not_in_load_list` panics on registry message "reopen belongs to REC-C1.5". Owner: REC-C1.5. Reproduces identically on origin/main. |
| `Coverage` | ✗ failure | 35453324957 | PRE-EXISTING: 4 tripwire_tools tests (`test_tripwire_create_and_list`, `test_tripwire_delete`, `test_tripwire_multiple_conditions`, `test_tripwire_query`) panic with "no active session: supply scope=session{session_id} or start a session". Owner: REC-C3.3.x. Reproduces identically on origin/main. |

Both pre-existing failures are NOT introduced by CIH-D. CIH-D's translator
fix surfaces the typed `CursorStale` variant that `restart_uat::r2_stale_cursor`
expects; this regression was masked pre-CIH-B by the harness `SpawnFailed`.

## Slices executed

- **CIH-A** (`533304b4`, doc-fix `7447ea56`): reconciled `m1_02_execution_log_persistence_impl` Case 6 to REC-C1.5.2 strict replay contract. Typed `LogError::ReplayIntegrity { kind: ReplayIntegrityError::CorruptSegment }`, no silent salvage, atomicity owned by `apply_plan()`.
- **CIH-B** (`b71c1adc`): `McpTestClient::start` harness now resolves the `chronos-mcp` binary via `cargo metadata` + lazy `cargo build --bin chronos-mcp` with `OnceLock` cache. `McpProcess::spawn_with_env` now reports executable path + exit status on `SpawnFailed`. Smoke subset 9/9 passed; `m1_07`/`m1_08` no longer eprintln-and-return.
- **CIH-C** (`5bcb2b63`): Vault Drift CC#11/CC#15/CC#18/CC#19/CC#22/CC#23/CC#26/CC#39 reconciled to exit 0; CC#4 stale SHAs regenerated via `regen_manifest_index_shas.py`. Cycles with missing artifacts restored with explicit `_note` documentation; `SESSION_HANDOFF_2026-09-18.md` relocated to `session-handoff/` per `m10-vault-handoff-relocate`. Cycle formally closed 2026-09-19T09:35:00Z.
- **CIH-D** (`e82921c9` + `8fdc3ef4`): production translator fix in `crates/chronos-services/src/session_log.rs` that splits `ExecutionLogError::PositionBeforeRetention` out of the `DrainFailed` wildcard arm into its own match arm constructing the typed `ServiceError::CursorStale { requested_next_seq: u64, retained_from_seq: u64 }`. Five new unit tests pin the contract. Closes the Coverage regression that CIH-B revealed by fixing the harness `SpawnFailed`. Locally all gates GREEN at HEAD 8fdc3ef4; remotely 3/5 GREEN. Cycle reopened to record this evidence.

## Invariants

- Production code in `crates/{chronos-log,chronos-native,chronos-mcp,chronos-store,chronos-webhook,...}`: untouched by CIH-A/B/C.
- **CIH-D did modify production code** in `crates/chronos-services/src/session_log.rs`: the canonical-evidence translator now constructs typed `ServiceError::CursorStale { requested_next_seq: u64, retained_from_seq: u64 }` for `ExecutionLogError::PositionBeforeRetention` instead of collapsing it into opaque `ServiceError::DrainFailed(String)`. This is the minimal production change required to close the Coverage regression that CIH-B revealed by fixing the harness `SpawnFailed`. No other variant behavior changes. 377 existing chronos-services lib tests still pass (382/382 with the 5 new translator tests).
- No `#[ignore]`, no `--skip`, no waiver patterns added to `reconstruction-contracts.toml`.
- All pre-existing baseline failures now have a concrete owner: `test_drop_session_not_in_load_list` → REC-C1.5; `tripwire_tools` (4 tests) → REC-C3.3.x.
- `reconstruction-contracts.toml` `[baseline_scope.deferred]` remains empty.
- `active_gate` remains `REC-C3`; closure of this hygiene cycle does NOT flip the active gate (Tren B is unblocked to branch off `main`, not to flip the gate).
- Tren B unblock requires BOTH: (a) ff-merge of `rec-c3-ci-hygiene` into `origin/main`, AND (b) all 5 GH Actions workflows GREEN on the integrated HEAD. (a) is satisfied; (b) is PARTIAL (3/5) at the most recent remote verification; re-closure requires the remaining 2 pre-existing failures to be owned and fixed outside this cycle.

## Canonical SHA fields

| Field | Value |
|---|---|
| Cycle | `rec-c3-ci-hygiene` |
| Head SHA | `2e761d4f76dc4c7b2a78972753770ac2aedf0357` |
| Remote tag | (no remote tag pushed; this is a hygiene cycle — see scope_decisions: "No tag.") |
| Remote tag_peel | n/a (no remote tag) |
| Peel match | false |
| Date (original CIH-C closure) | `2026-09-19T09:35:00Z` |
| Date (CIH-D reopen + remote verification) | `2026-09-19T16:14:00Z` |
| Post-CIH-D remote HEAD | `8fdc3ef40dc307d861e16e9bdf2e178431dacddc` |
| Remote verify verdict (post-CIH-D) | PARTIAL: 3/5 GREEN (Vault_Drift, Architecture, Debt_Sentinel); 2/5 with PRE-EXISTING failures (CI: REC-C1.5 owner; Coverage: REC-C3.3.x owner) |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |