# Merge receipt — rec-c2.5-formal-closure

## Git state at release

- Branch: `main`
- Head: `9968ef4e` (retroactive ledger-sync artifacts)
- Pre-artifact commit: `21ab4f24` (governance change)
- Pre-cycle base: `228b476f` (state before this cycle)
- Tag: `rec-c2-5-formal-closure` → `21ab4f24^{commit}`

## Commits in this cycle

```
228b476f  chore(retire-stale-bus-doc): add merge-receipt and release-receipt   (base)
21ab4f24  chore(rec-c2.5): formal closure — REC-C2 closed, REC-C3 next        (governance)
9968ef4e  chore(rec-c2.5): retroactive ledger-sync artifacts                  (ledger artifacts)
```

## Branch deletion summary (during the cycle, before commits)

Local:
```
git branch -d chore/rec-c2.3-retire-stale-bus-doc
git branch -d feat/rec-c1.5-closure
git branch -d feat/rec-c1.6-lifecycle-retention-wire
git branch -d feat/rec-c2.2-accepted-raw-seam
git branch -d feat/rec-c2.3-eventbus-removal
```

Remote:
```
git push origin --delete feat/rec-c1.6-lifecycle-retention-wire
git push origin --delete feat/rec-c2.1-tripwire-evidence
git push origin --delete feat/rec-c2.2-accepted-raw-seam
```

## Outcomes

- `python3 scripts/check_legacy_evb.py`: PASS, baseline 0
- `python3 scripts/check_architecture_contracts.py`: PASS
- `python3 scripts/check_architecture_contracts.py --strict-legacy`: PASS
- `cargo fmt --all -- --check`: PASS (clean)
- `cargo clippy --workspace --all-targets -- -D warnings`: PASS (clean)
- Working tree: clean
- Tag `rec-c2-5-formal-closure` created and pointed at `21ab4f24`