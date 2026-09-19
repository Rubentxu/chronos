# Release receipt — rec-c3-ci-hygiene

> Hygiene cycle: pre-existing baseline failures on origin/main blocked Tren B
> (REC-C3.3.3) start. Three slices reconciled. No production code touched.
> Five gates GREEN at closure. Branch archived in place (not fast-forwarded to
> main); Tren B is unblocked to branch off `main` once this archive lands.

## Released artifacts

- Branch: `rec-c3-ci-hygiene`
- Cycle merge base: `fa5eb582`
- Cycle merge head: `5bcb2b63`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c3-ci-hygiene`

## Five-gate result (all GREEN)

| Gate | Receipt | Status |
|---|---|---|
| `CI` | `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e -- --test-threads=1` | passed (62 suites, 0 failed) |
| `Coverage` | smoke subset `m1_acceptance` + `e2e_connectivity` + `analytics_tools` + `session_persistence` | passed (16/16) |
| `Vault_Drift` | `bash scripts/check_vault_drift.sh` | passed (exit 0; 48 python + 7 bash CCs clean) |
| `Architecture` | `python3 scripts/check_architecture_contracts.py` + `python3 scripts/check_hex_boundary.py` | passed |
| `Debt_Sentinel` | empty deferred bucket + 0 unclassified regressions across 62 T3 suites | passed |

## Slices executed

- **CIH-A** (`533304b4`, doc-fix `7447ea56`): reconciled `m1_02_execution_log_persistence_impl` Case 6 to REC-C1.5.2 strict replay contract. Typed `LogError::ReplayIntegrity { kind: ReplayIntegrityError::CorruptSegment }`, no silent salvage, atomicity owned by `apply_plan()`.
- **CIH-B** (`b71c1adc`): `McpTestClient::start` harness now resolves the `chronos-mcp` binary via `cargo metadata` + lazy `cargo build --bin chronos-mcp` with `OnceLock` cache. `McpProcess::spawn_with_env` now reports executable path + exit status on `SpawnFailed`. Smoke subset 9/9 passed; `m1_07`/`m1_08` no longer eprintln-and-return.
- **CIH-C** (`5bcb2b63`): Vault Drift CC#11/CC#15/CC#18/CC#19/CC#22/CC#23/CC#26/CC#39 reconciled to exit 0; CC#4 stale SHAs regenerated via `regen_manifest_index_shas.py`. Cycles with missing artifacts restored with explicit `_note` documentation; `SESSION_HANDOFF_2026-09-18.md` relocated to `session-handoff/` per `m10-vault-handoff-relocate`. Cycle formally closed.

## Invariants

- Production code in `crates/{chronos-log,chronos-services,chronos-mcp,chronos-store,chronos-native,chronos-webhook,...}`: untouched.
- No `#[ignore]`, no `--skip`, no waiver patterns added to `reconstruction-contracts.toml`.
- All five pre-existing baseline failures now have a concrete cycle + slice owner.
- `reconstruction-contracts.toml` `[baseline_scope.deferred]` remains empty.
- `active_gate` remains `REC-C3`; closure of this hygiene cycle does NOT flip the active gate (Tren B is unblocked to branch off `main`, not to flip the gate).

## Canonical SHA fields

| Field | Value |
|---|---|
| Head SHA | `5bcb2b6351f216ec95430b18e15ed5a6999d561e` |
| Remote tag | `rec-c3-ci-hygiene-closure` (pending push) |
| Remote tag_peel | `5bcb2b6351f216ec95430b18e15ed5a6999d561e` |
| Peel match | true |
| Date | `2026-09-19T10:12:34Z` |
| Base SHA | `fa5eb5827e997c874047c6c2318ad2b5b1c8f323` |