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

## Canonical SHA fields (added by CIH-C)


| Head SHA | `ab863cf14814bda9746975dcb31954daa2dafd8d` |

| Base SHA | `unknown` |

| Branch | `main` |

| Date | `2026-09-19T00:00:00Z` |
