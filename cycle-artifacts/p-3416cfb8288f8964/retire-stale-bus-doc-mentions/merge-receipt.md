# Merge receipt — retire-stale-bus-doc-mentions

## Git state at release

- Branch: `main`
- Head: `b9461a3f` (handoff commit, 2026-09-18)
- Pre-release merge commit: `ab863cf1` (chore merge)
- Retroactive ledger-sync commit: `473ae666`
- Tag: `retire-stale-bus-doc-mentions` → `ab863cf1`

## Merge chain

```
d435557e  Merge branch 'feat/rec-c2.3-eventbus-removal'      (REC-C2.3 merge)
ab863cf1  Merge branch 'chore/rec-c2.3-retire-stale-bus-doc'  (chore merge)
b9461a3f  docs(cycle): add session handoff note for 2026-09-18
473ae666  chore(retire-stale-bus-doc): retroactive ledger-sync artifacts
```

## Commits covered by this cycle

| SHA | Description |
|---|---|
| `23757379` | 5 doc-only edits + 2 residual test stubs + fmt fix + `Default` impl |
| `99c0dcee` | cycle-artifacts proposal + tasks |
| `473ae666` | retroactive ledger-sync reports (exploration/spec/impl/verify) |

## Outcomes

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- 653/653 lib tests pass across `chronos-log`, `chronos-native`, `chronos-services`, `chronos-mcp`
- Ratchet `check_legacy_evb.py`: baseline 0
- Working tree clean
- Tag `retire-stale-bus-doc-mentions` points at `ab863cf1`
## Canonical SHA fields (added by CIH-C.1)

| Field | Value |
|---|---|
| Head SHA | `ab863cf14814bda9746975dcb31954daa2dafd8d` |
| Base SHA | `ab863cf14814bda9746975dcb31954daa2dafd8d` |
| Branch | `retire-stale-bus-doc-mentions` |
| Date | (original merge date not verifiable from current artifacts; see _restoration_note) |

## Restoration note

CIH-C.1 (2026-09-19) appended these canonical SHA fields because the
original merge-receipt.md artifact produced by the prior cycle did
not carry them. The Head SHA and Base SHA were reconstructed from
the apply-checkpoint.json `head_sha` and `base_sha` fields (which in
turn were reconciled from git history by CIH-C.1 — see
apply-checkpoint.json restorations for related cycles). The Branch
field reflects the cycle's branch (from apply-checkpoint.json `branch` field).
The Date field is left as a restoration placeholder because the
original receipt did not record a verifiable merge timestamp.
