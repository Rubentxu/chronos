# Release receipt — rec-c3-ci-hygiene

> Hygiene cycle: pre-existing baseline failures on origin/main blocked Tren B
> (REC-C3.3.3) start. Originally closed 2026-09-19T09:35:00Z at the historical
> anchor `2e761d4f76dc4c7b2a78972753770ac2aedf0357` (CIH-A/B/C). REOPENED
> 2026-09-19T16:14:00Z after CIH-D (commits e82921c9 + 8fdc3ef4) added the
> `PositionBeforeRetention` -> `ServiceError::CursorStale` translator fix.
> REOPENED again 2026-09-19T17:29:00Z after CIH-E (commit 379ba34d) delivered
> the explicit-session-scope plumbing for tripwire tools and pinned a scope-
> awareness contract at the MCP boundary.
> Cycle merge head remains `2e761d4f76dc4c7b2a78972753770ac2aedf0357` per
> operator rule #8 and CC#12 m9-19 convention. Tren B (REC-C3.3.3) unblock
> requires both: (a) CIH-D + CIH-E integrated into main, AND (b) all five GH
> Actions workflows GREEN on the integrated HEAD. (a) is satisfied locally;
> (b) is PARTIAL at HEAD 379ba34d (CIH-E flipped the local tripwire_tools
> test surface; remote Coverage expected GREEN, remote CI still RED on
> multi_session CIH-F; remaining 4/5 expected). Re-closure is blocked until
> REC-C1.5 owns the multi_session failure (CIH-F).

## Released artifacts

- Branch: `rec-c3-ci-hygiene`
- Cycle merge base: `fa5eb582`
- Cycle merge head: `2e761d4f76dc4c7b2a78972753770ac2aedf0357` (immutable per operator rule #8)
- Post-CIH-D remote HEAD: `8fdc3ef40dc307d861e16e9bdf2e178431dacddc`
- Post-CIH-E HEAD: `379ba34d5eef76d2510ca2d7f0b7c2d7d3b1df2b`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c3-ci-hygiene`
- Cycle status: REOPENED (post-CIH-E) — see remote verification below

## Five-gate result (locally GREEN at historical anchor; PARTIAL at post-CIH-E HEAD)

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

### Locally at HEAD 379ba34d (CIH-E local verification, 2026-09-19T17:29:00Z)

| Gate | Receipt | Status |
|---|---|---|
| `T0 (lint)` | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets --exclude chronos-e2e -- -D warnings` | exit 0; 0 errors / 0 warnings |
| `T1 (lib)` | `cargo test --workspace --lib --exclude chronos-native --exclude chronos-e2e --exclude chronos-sandbox --no-fail-fast` | 1233 passed, 0 failed |
| `T1 (chronos-native serial)` | `cargo test -p chronos-native --lib -- --test-threads=1` | 108 passed, 0 failed |
| `T4 (tripwire_tools)` | `cargo test -p chronos-sandbox --test tripwire_tools -- --test-threads=1` | 7/7 passed (incl. new `test_tripwire_scope_awareness_at_mcp_boundary`) |
| `T4 (tripwire_depth)` | `cargo test -p chronos-sandbox --test tripwire_depth -- --test-threads=1` | 6/6 passed |
| `T4 (probe_drain_canonical)` | `cargo test -p chronos-sandbox --test probe_drain_canonical -- --test-threads=1` | 4/4 passed |
| `T4 (rec_c2_2_uat_c2)` | `cargo test -p chronos-sandbox --test rec_c2_2_uat_c2 -- --test-threads=1` | 3/3 passed |

`CI` and `Coverage` are expected to flip on remote GH Actions once
`379ba34d` reaches `origin/main` after ff-merge of `rec-c3-ci-hygiene`.
The only remaining remote failure after that is `CI :: multi_session ::
test_drop_session_not_in_load_list`, owned by REC-C1.5 (CIH-F).

### Remotely at HEAD 379ba34d (CIH-E expected post-ff-merge, NOT verified yet)

| Gate | Expected | Note |
|---|---|---|
| `Vault_Drift` | GREEN (unchanged) | unaffected by CIH-E |
| `Architecture_Contracts` | GREEN (unchanged) | unchanged |
| `Sandbox_Debt_Sentinel` | GREEN (unchanged) | unchanged |
| `CI` | UNCHANGED: still RED on `multi_session::test_drop_session_not_in_load_list` | CIH-F owner; not a CIH-E concern |
| `Coverage` | expected GREEN | CIH-E flipped the tripwire_tools test surface locally; remote should match |

Both pre-existing failures are NOT introduced by CIH-D. CIH-D's translator
fix surfaces the typed `CursorStale` variant that `restart_uat::r2_stale_cursor`
expects; this regression was masked pre-CIH-B by the harness `SpawnFailed`.

## Slices executed

- **CIH-A** (`533304b4`, doc-fix `7447ea56`): reconciled `m1_02_execution_log_persistence_impl` Case 6 to REC-C1.5.2 strict replay contract. Typed `LogError::ReplayIntegrity { kind: ReplayIntegrityError::CorruptSegment }`, no silent salvage, atomicity owned by `apply_plan()`.
- **CIH-B** (`b71c1adc`): `McpTestClient::start` harness now resolves the `chronos-mcp` binary via `cargo metadata` + lazy `cargo build --bin chronos-mcp` with `OnceLock` cache. `McpProcess::spawn_with_env` now reports executable path + exit status on `SpawnFailed`. Smoke subset 9/9 passed; `m1_07`/`m1_08` no longer eprintln-and-return.
- **CIH-C** (`5bcb2b63`): Vault Drift CC#11/CC#15/CC#18/CC#19/CC#22/CC#23/CC#26/CC#39 reconciled to exit 0; CC#4 stale SHAs regenerated via `regen_manifest_index_shas.py`. Cycles with missing artifacts restored with explicit `_note` documentation; `SESSION_HANDOFF_2026-09-18.md` relocated to `session-handoff/` per `m10-vault-handoff-relocate`. Cycle formally closed 2026-09-19T09:35:00Z.
- **CIH-D** (`e82921c9` + `8fdc3ef4`): production translator fix in `crates/chronos-services/src/session_log.rs` that splits `ExecutionLogError::PositionBeforeRetention` out of the `DrainFailed` wildcard arm into its own match arm constructing the typed `ServiceError::CursorStale { requested_next_seq: u64, retained_from_seq: u64 }`. Five new unit tests pin the contract. Closes the Coverage regression that CIH-B revealed by fixing the harness `SpawnFailed`. Locally all gates GREEN at HEAD 8fdc3ef4; remotely 3/5 GREEN. Cycle reopened to record this evidence.
- **CIH-E** (`379ba34d`): tripwire MCP tools now require an explicit `session_id` scope at every handler entry. `crates/chronos-mcp/src/server.rs` adds `session_id: Option<String>` to `TripwireCreateParams` and `TripwireDeleteParams`; replaces `NoParams` with `TripwireListParams { session_id }` and `TripwireQueryParams { session_id }`. Every handler maps `params.session_id` -> `ObserveScope::Session{session_id}` and threads it as `ObserveInput.scope`; the canonical-evidence observe pipeline resolves the canonical session with documented precedence (scope=session{id} wins; active_session fallback). `chronos-sandbox/src/client/{tools,types}.rs` forward the session_id argument at every tripwire call. All affected sandbox tests (`tripwire_tools`, `tripwire_depth`, `rec_c2_2_uat_c2`, `probe_drain_canonical`, `m0_acceptance`) own a real probe session before any tripwire call; `m0_04` starts the probe first so the canonical scope is available at create time. New test `test_tripwire_scope_awareness_at_mcp_boundary` verifies the explicit-scope contract end-to-end at the MCP boundary. Locally T0 0/0, T1 1233/1233 (+ 108 chronos-native serial), T4 tripwire_tools 7/7, tripwire_depth 6/6, probe_drain_canonical 4/4, rec_c2_2_uat_c2 3/3. Locally flips the tripwire_tools test surface from FAIL to PASS; remote Coverage expected GREEN after ff-merge.

## Invariants

- Production code in `crates/{chronos-log,chronos-native,chronos-mcp,chronos-store,chronos-webhook,...}`: untouched by CIH-A/B/C.
- **CIH-D did modify production code** in `crates/chronos-services/src/session_log.rs`: the canonical-evidence translator now constructs typed `ServiceError::CursorStale { requested_next_seq: u64, retained_from_seq: u64 }` for `ExecutionLogError::PositionBeforeRetention` instead of collapsing it into opaque `ServiceError::DrainFailed(String)`. This is the minimal production change required to close the Coverage regression that CIH-B revealed by fixing the harness `SpawnFailed`. No other variant behavior changes. 377 existing chronos-services lib tests still pass (382/382 with the 5 new translator tests).
- **CIH-E did modify production code** in `crates/chronos-mcp/src/server.rs`: the four `tripwire_*` MCP tool handlers now require an explicit `session_id` scope (via the new `session_id` field on each params struct) and thread it through the canonical-evidence observe pipeline. Wire shape is forward-compatible; same v1 responses. No `#[ignore]`, no `--skip`, no waiver patterns added to `reconstruction-contracts.toml`.
- **CIH-E architectural follow-up (NOT in slice)**: `TripwireManager` keys by `TripwireId` (global), not by session. Cross-session isolation of tripwire *definitions* requires `Tripwire` to gain `session_id`, `TripwiresService::create` to stamp it, and `TripwireManager.list()` to filter by it. Per-call `fire_count` is correctly scoped today (ExecutionLog keyed by session_id) so leakage is bounded to the manager view only. Carried as `FIND-CIH-E-follow-up-TripwireManager-per-session-keying`, owner REC-C3.x.
- All pre-existing baseline failures now have a concrete owner: `test_drop_session_not_in_load_list` → REC-C1.5 (CIH-F); `tripwire_tools` (4 tests) → REC-C3.3.x (CLOSED by CIH-E at HEAD 379ba34d).
- `reconstruction-contracts.toml` `[baseline_scope.deferred]` remains empty.
- `active_gate` remains `REC-C3`; closure of this hygiene cycle does NOT flip the active gate (Tren B is unblocked to branch off `main`, not to flip the gate).
- Tren B unblock requires BOTH: (a) ff-merge of `rec-c3-ci-hygiene` into `origin/main`, AND (b) all 5 GH Actions workflows GREEN on the integrated HEAD. After (a) with CIH-D + CIH-E integrated, (b) is expected to be 4/5 GREEN; re-closure requires CIH-F (REC-C1.5 owner) for the remaining CI failure.

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
| Date (CIH-E local verification) | `2026-09-19T17:29:00Z` |
| Post-CIH-D remote HEAD | `8fdc3ef40dc307d861e16e9bdf2e178431dacddc` |
| Post-CIH-E local HEAD | `379ba34d5eef76d2510ca2d7f0b7c2d7d3b1df2b` |
| Remote verify verdict (post-CIH-D) | PARTIAL: 3/5 GREEN (Vault_Drift, Architecture, Debt_Sentinel); 2/5 with PRE-EXISTING failures (CI: REC-C1.5 owner; Coverage: REC-C3.3.x owner) |
| Remote verify verdict (post-CIH-E) | EXPECTED: 4/5 GREEN (Vault_Drift, Architecture, Debt_Sentinel, Coverage); 1/5 with PRE-EXISTING failure (CI: REC-C1.5 owner; CIH-F). Local evidence at 379ba34d corroborates the Coverage flip; remote re-run pending ff-merge. |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |