# C3.3.2 — Tren A close receipt

| Field            | Value                                                          |
|------------------|----------------------------------------------------------------|
| Cycle            | REC-C3.3.2 — composition / integration inversion (Tren A)      |
| Project          | p-3416cfb8288f8964 (chronos)                                   |
| Branch           | rec-c3.3-train-a                                               |
| Released baseline (origin/main pre-merge) | `147c10195`                       |
| Workspace version (pre-merge)             | `0.1.1`                            |
| Head SHA (vault)                          | `25420e5dab54a02e46da20ecc0bacf050d2ac741` |
| Merge SHA (origin/main post-merge)        | `d30d025071d9f464a6ba9ff6cc50d68ce300f167` |
| Commits ahead of base                    | 46                                  |
| Tag                                      | none (structural inversion; next behavior-bumping cycle earns the tag) |
| Date                                     | 2026-09-18                                                  |
| Operator decision                        | option 1 — fast-forward merge with documented bypass        |

## Pre-merge gates (PR #31 head `d30d0250`)

| Gate                       | Status |
|----------------------------|--------|
| Architecture Contracts     | GREEN  |
| Sandbox Debt Sentinel      | GREEN  |
| CI                         | RED — exclusively by `m1_02_execution_log_persistence_impl` pre-existing baseline failure (REC-C1.5.2 strict replay vs stale lenient test) |
| Coverage                   | RED — exclusively by 6 `tripwire_depth` tests failing under tarpaulin harness (subprocess collision) |
| Vault Drift Sweep          | RED — exclusively by 16 lines of pre-existing drift on `origin/main` (CC#18: 9, CC#19: 6, CC#39: 1) |

## Post-merge invariants (verified on `origin/main == d30d0250`)

| Invariant                                            | Status |
|------------------------------------------------------|--------|
| `services → ebpf = 0` (no waiver)                    | OK     |
| `services → browser = 0` (no waiver)                 | OK     |
| Ledger = exactly 3 waivers                           | OK (`services→store`, `services→native`, `store→native`) |
| `cargo fmt --all -- --check`                         | OK (exit 0) |
| `cargo clippy --workspace --all-targets --features "chronos-services/test-utils" -- -D warnings` | OK (exit 0) |
| `python3 scripts/check_architecture_contracts.py`    | PASSED |
| `python3 scripts/check_legacy_evb.py`                | PASSED (ratchet = 0) |
| `cargo test -p chronos-services --test c33_25_capability_bundle_ratchet` | 7/7 pass |

## Findings carried forward (no_action_in_current_cycle, NOT permanent debt)

| ID                                  | Owned by                    |
|-------------------------------------|-----------------------------|
| `FIND-Tren-A-01`                    | rec-c3-ci-hygiene slice C   |
| `FIND-CI-PRE-EXISTING-TEST-FAILURES` | rec-c3-ci-hygiene slices A (m1_02) + B (tripwire_depth) |

`FIND-INFRA-DERIVE-SKIP-ARGS-EMPTY` was closed inside this cycle (commit `5d3c9302`).

## New cycle opened

**`rec-c3-ci-hygiene`** — must reach all-GREEN before Tren B starts.

| Slice | Scope | Acceptance |
|-------|-------|------------|
| A | `m1_02_execution_log_persistence_impl` reconciliation. Corrupt segment ⇒ typed `ReplayIntegrity::CorruptSegment`. No partial handle. No invented Gap. No production semantic change unless evidence contradicts REC-C1.5.2. | `cargo test -p chronos-sandbox --test m1_acceptance m1_02_execution_log_persistence_impl` GREEN |
| B | `tripwire_depth` Coverage. NO `#[ignore]`, NO `--skip`. Instrument `McpTestClient::start()` with executable path + child exit status + stderr. Fix the harness / coverage integration, not the six tests individually. | Coverage job GREEN |
| C | Vault Drift Sweep reconciliation CC#18/19/39. | `python3 scripts/check_vault_drift.sh` exit 0 |

## Gate state after rec-c3-ci-hygiene must be GREEN

```
CI               GREEN
Coverage         GREEN
Vault Drift      GREEN
Architecture     GREEN
Debt Sentinel    GREEN
```

## Next cycle (only after hygiene closes)

**`rec-c3-3-3 Tren B`**:

- **B1** `services → native` — no `NativeProbeServicePort`; reuse existing ports.
- **B2** `services → store` — `SessionArchive` (save/load/list/delete) + `CounterexampleRepository` (separate); `SessionMetadata` moves to `chronos-domain`. Stop rule: if removing `chronos-store` from `services/Cargo.toml` requires `dyn Any` / downcast / store helper / `chronos_store` DTO crossing the port, the inversion is wrong; abort and replan.
- **`store → native` is NOT Tren B.** It belongs to REC-C3.4 (per C3.3.0 + C3.3.2 exploration reports, lines 34/210/224/257). Last waiver after Tren B = `store→native`; the final waiver closes only at the end of REC-C3.4.

Composition root stays in `chronos-mcp::composition`. Bootstrap-scoped vs session-scoped factory distinction is mandatory for B1 and B2.
