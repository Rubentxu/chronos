# Release receipt — retire-stale-bus-doc-mentions

> **Status**: retroactive. The cycle merged to `main` on 2026-09-17; the
> ledger sync closes it on 2026-09-18.

## Released artifacts

- Branch: `main`
- Head at merge time: `ab863cf1`
- Head at ledger sync time: `b9461a3f` (handoff) → `473ae666` (ledger-sync)
- Tag: `retire-stale-bus-doc-mentions` at `ab863cf1`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/retire-stale-bus-doc-mentions`

## Tags verified

- `retire-stale-bus-doc-mentions` → `ab863cf14814bda9746975dcb31954daa2dafd8d`
- Predecessor: `rec-c2.3-eventbus-removal` → `d435557e`

## Evidence summary

| Gate | Receipt | Status |
|---|---|---|
| `exploration-sufficient` | `gate-exploration-sufficient-93c5e52edbc99ed8-1` | passed |
| `requirements-testable` | `gate-requirements-testable-22a6ce51a9c6437e-1` | passed |
| `implementation-complete` | `gate-implementation-complete-e15b2582d3754292-1` | passed |
| `tests-pass` | `gate-tests-pass-795d0d6bb5c615ea-1` | passed (653/653) |
| `policy-compliant` | `gate-policy-compliant-795d0d6bb5c615ea-1` | passed |
| `debt-severity-assigned` | `gate-debt-severity-assigned-795d0d6bb5c615ea-1` | passed |
| `debt-priority-assigned` | `gate-debt-priority-assigned-795d0d6bb5c615ea-1` | passed |
| `no-pending-effects` | `gate-no-pending-effects-0a830311f6c7dd3e-1` | passed |

## Cycle path

A-min (retroactive; proposal §Tier declared B-direct but ledger path
forced A-min to satisfy `phase.specify.complete.a-min` and
`phase.verify.complete.a-min` transitions).

## Notes

- No public API change.
- No MCP wire change.
- No ratchet regression.
- No follow-on cycles from this one (closes ledger only).
## Canonical SHA fields (added by CIH-C.1)

| Field | Value |
|---|---|
| Cycle | `retire-stale-bus-doc-mentions` |
| Head SHA | `ab863cf14814bda9746975dcb31954daa2dafd8d` |
| Remote tag | (no remote tag pushed; archived in branch) |
| Remote tag_peel | `ab863cf14814bda9746975dcb31954daa2dafd8d` |
| Peel match | true |
| Date | (original release date not verifiable from current artifacts; see _restoration_note) |

## Restoration note

CIH-C.1 (2026-09-19) appended these canonical SHA fields because the
original release-receipt.md artifact produced by the prior cycle did
not carry them. The Head SHA and remote_tag_peel were reconstructed
from the apply-checkpoint.json `head_sha` field (which in turn was
reconciled from git history by CIH-C.1 — see apply-checkpoint.json restorations for related cycles
similar reconciliation entries). The "Remote tag" entry reflects that
no remote tag was pushed for this cycle (it was archived in branch).
