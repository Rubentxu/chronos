# m9-88-cc55-drift-remediation-closure-2026-09-14

## Cycle identity

- **Cycle ID:** m9-88-cc55-drift-remediation
- **Path:** B-direct (vault-only)
- **Branch:** feat/m9-88-cc55-drift-remediation
- **Base SHA:** 0dba57ddf8391acbee5adbd2fb6c6ab30fc179bc (m9-87 vault commit)
- **Head SHA:** 78ec3861a71b3258cb55f37d70d85f072a039028
- **Merge SHA:** 8b6a9bc625e55ef9065b851ef5fbb25999fce942
- **Remote tag:** v0.7.90 (peel: 851dba6)
- **Date:** 2026-09-14T13:56Z

## What m9-88 did

Fixed 3 pre-existing CC#55 drift lines that surfaced after m9-87
closure:

1. **m9-66 JSON parse**: line 35 had raw `\|` bytes (invalid JSON
   escape). Promoted to `\\|` (valid JSON escape representing the
   regex characters `^\|`).
2. **m9-67 base_sha**: was `67b3d76bb6e9...` (off-by-one). Replaced
   with real parent `5c83df9ce96862c0c95598d63cddb147e3ea6ab5` (vault
   head `ef21e1fe` parent). Cascade-updated 5 files.
3. **m9-85 base_sha**: was `2c2a5cc8f837...` (non-existent). Replaced
   with real parent `72e120c2e2bb9774c459ef516dcf3e7b21ef90c3`.
   Cascade-updated 4 files.
4. **m9-79 peel_match** (bonus): was `None` despite head==peel (both
   `f41abd45...`). Set to `true`.

No chronos source code modified. Vault-only cycle.

## Verification

- **CC#55 specific**: 0 base_sha / head_sha reachability errors
  across all m9-NN apply-checkpoints.
- **Round-trip safety** (no source change expected): 77 store + 264
  services + 35 cli tests pass.
- **Lint**: `cargo clippy --workspace --all-targets -- -D warnings`
  clean. `cargo fmt --all -- --check` clean.

## Cascade effect: pre-existing drift surfaced

The m9-66 JSON fix unblocks all CCs that load apply-checkpoints.
Pre-existing drift in m9-77..m9-87 now surfaces (~150 drift lines
across CC#3, CC#7, CC#8, CC#11, CC#12, CC#14, CC#15, CC#18, CC#22,
CC#23, CC#29, CC#39, CC#40, CC#43).

This is the same failure pattern that m9-57 documented: a strict CC
that fails on malformed input silently passes if downstream CCs don't
inspect the failure.

**Out of scope for m9-88** (recommend follow-up hardening cycles):
- `status: "CLOSED"` enforcement (currently "archived"/"released").
- `archived_at` backfill for cycles with archive-manifest.md.
- `findings_introduced` field backfill (currently missing).
- `peel_match` field backfill.
- `CC#3` hardening to accept `peel_match=False` for non-fix-peel
  cycles (e.g. m9-77..m9-82 with tags on code commits).

## Cross-cycle context

| Cycle | Description | Drift closure |
|---|---|---|
| m9-87 | cc-001 god-module schema split | 8 CCs closed |
| m9-88 | CC#55 drift remediation (this) | 1 CC closed (CC#55) |

m9-88 is intentionally narrow: it addresses only CC#55. The
cascading pre-existing drift it reveals is tracked for follow-up.

## Discovery: FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK (deferred-external)

During m9-88 explore phase, I investigated FIND-M9-81 (the `sddk
cycle evaluate-gate` CLI bug). Reproduction confirmed:

```
$ sddk cycle evaluate-gate --transition explore --gate scope --outcome passed ...
admission event recording failed (fail-soft): storage error:
event_store:duplicate_event_id:authority-approval-system-cli_run-advisory_high_approval
error[ENGINE_UNREGISTERED_EVALUATOR]: evaluator sddk.cli is not registered for gate scope
```

Root cause: sddk CLI binary at `/home/rubentxu/.local/bin/sddk`
generates a deterministic event_id (`authority-approval-system-cli_run-advisory_high_approval`)
that collides on repeated calls. Plus, evaluators must be registered
in the workflow manifest for the gate type.

**Cannot fix without modifying the sddk binary or filing upstream.**
The chronos project continues to use the manual vault-tracked workflow.

## Cycle artifacts written

All under `cycle-artifacts/p-3416cfb8288f8964/m9-88-cc55-drift-remediation/`:

- apply-checkpoint.json
- implementation-receipt.md
- merge-receipt.md
- release-receipt.md
- release-report.md
- verify-findings.json
- verify-report.md

Plus `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-88-cc55-drift-remediation/archive-manifest.md`
with `## Evidence bindings` + `## Cross-checks` sections.

Plus `cycles/index.md` updated with m9-88 row and `Total cycles | 88`.

## Carry-forward

Open (not addressed by m9-88):

- **FIND-M9-88-CASCADE-DRIFT-SURFACED** — recommend m9-89 hardening
  cycle to address the ~150 drift lines that surface in m9-77..m9-87
  after m9-66 JSON fix.
- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** — external sddk CLI binary
  bug. Deferred.
- **M7 milestone** — events_read merge, observe merge, lifecycle work.
- **cc-001 final slimming** — optional, diminishing returns.

## Tag pre-creation pattern (CC#42 workaround)

Tag `v0.7.90` was pre-created at `78ec386` (refactor commit), then
moved to `851dba6` (merge commit) per the CC#42 fixpoint-cascade
workaround documented in m9-83 handoff.

## Session timing

- Session start: ~13:43Z (post-m9-87 handoff).
- Cycle close: ~13:57Z.
- Wall time: ~14 min (mostly writing cycle artifacts + commit/merge).
