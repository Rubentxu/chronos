# REC-C2.0 — tasks

**Cycle**: `rec-c2.0-eventbus-inventory`
**Companion**: `proposal.md`, `characterization.md`

## T0 — inventory + ratchet

1. `legacy-evb-inventory.json` at repo root: 37 entries, `baseline_total=37`.
2. `scripts/check_legacy_evb.py`: `--write` regenerates; default enforces the
   four rules; `--strict` adds the CANONICAL-destructive check.
3. Wire into `scripts/check_architecture_contracts.py`
   (`verify_legacy_evb_inventory`, `--strict-legacy`).

## T1 — characterizations

Six GREEN tests that measure reality:

- CHAR-C2-01 `chanos-native/probe_backend.rs::char_c2_01_*`
- CHAR-C2-02 `chronos-services/observe.rs::char_c2_02_*`
- CHAR-C2-03 `chronos-services/observe.rs::char_c2_03_*`
- CHAR-C2-04 `chronos-domain/tripwire.rs::char_c2_04_*`
- CHAR-C2-05 `chronos-domain/tripwire.rs::char_c2_05_*`
- CHAR-C2-06 `chronos-native/probe_backend.rs::char_c2_06_*`

## T2 — ledger hygiene (pre-existing, repaired in-cycle)

`check_architecture_contracts.py` was RED on `main@063cbdf8`: TRUTH-001/002/003
are `status="verified"` but lacked the `uat` field required by the ledger
policy. Minimum patch: add the `uat` arrays (the UATs already exist and were
named in `notes`). Documented as pre-existing in the checkpoint.

## T3 — gates + close

- `python3 scripts/check_legacy_evb.py` PASS; `--strict` FAIL with 7
  CANONICAL destructive reads (the C2 gate to clear).
- `python3 scripts/check_architecture_contracts.py` PASS.
- T0 fmt + clippy; T3 split (`--lib` excluding sandbox/e2e/native +
  `chronos-native --lib -- --test-threads=1`).
- regen archive-manifest SHAs; merge `--no-ff`; annotated tag
  `rec-c2.0-eventbus-inventory`; `archive_status = "ready"`.

## Handoff to REC-C2.1

DoD recorded in `characterization.md` §"Target for REC-C2.1". The three
explicit items C2.0 raises but does not implement:

1. invert `dual_push` (persist first);
2. `TripwireFired` as typed `ExecutionKind::TripwireFired` evidence with
   `source_seq`, persisted before fan-out;
3. `retained_until_session_end` either truly retained (consumer cursor) or
   rejected as `Unsupported`; `fire_count` derived from persisted evidence.
