# Implementation receipt — rec-c2.5-formal-closure

## Commits

| SHA | Description |
|---|---|
| `21ab4f24` | Governance update: `docs/ROADMAP.md` and `reconstruction-contracts.toml` reflect REC-C2 closure and REC-C3 next. |
| (tag) `rec-c2-5-formal-closure` | Annotated tag at the merge commit above. |

## Pre-existing work this cycle formalizes

| SHA | Sub-cycle | Description |
|---|---|---|
| `4c7df70e` | C2.0 | EventBus legacy inventory + ratchet |
| `686a364c` | C2.1 | TripwireFired as ExecutionLog evidence |
| `f02ab311` | C2.2 | Accepted-Raw seam + producer convergence |
| `d435557e` | C2.3 | EventBus removal |

## Files modified by `21ab4f24`

```
docs/ROADMAP.md               | 65 +++++++++++++++++++++----------------------
reconstruction-contracts.toml | 31 +++++++++++++--------
2 files changed, 50 insertions(+), 46 deletions(-)
```

## Verification

- `python3 scripts/check_legacy_evb.py` → PASS, baseline 0
- `python3 scripts/check_architecture_contracts.py` → PASS
- `python3 scripts/check_architecture_contracts.py --strict-legacy` → PASS

(No Rust code change → no `cargo test` runs were necessary.
The ratchet and the contract gate are the substantive verification.)

## Branch cleanup performed during the cycle

Local branches deleted (5):
- `chore/rec-c2.3-retire-stale-bus-doc` (merged at `ab863cf1`)
- `feat/rec-c1.5-closure` (merged at `a1a79c80`)
- `feat/rec-c1.6-lifecycle-retention-wire` (merged at `83ee38e2`)
- `feat/rec-c2.2-accepted-raw-seam` (merged at `f02ab311`)
- `feat/rec-c2.3-eventbus-removal` (merged at `d435557e`)

Remote branches deleted (3):
- `origin/feat/rec-c1.6-lifecycle-retention-wire`
- `origin/feat/rec-c2.1-tripwire-evidence`
- `origin/feat/rec-c2.2-accepted-raw-seam`

## Acceptance

REC-C2 gate closure criteria (per the convergence sequence in
`docs/ROADMAP.md`):

- `chronos-domain::bus` module deleted (REC-C2.3, `af41d8c2`).
- `ProbeBackend::read_since` removed from trait (REC-C2.3.1, `a99b3a35`).
- `bus_capacity` / `bus_fill` removed from wire (REC-C2.3.3, `fb170dcc`).
- TripwireFired flows as `ExecutionKind::TripwireFired` from the
  canonical log (REC-C2.1, `686a364c`).
- `legacy-evb-inventory.json` baseline = 0 (REC-C2.3.4, `ad28430f`).
- LEGACY-001 and LEGACY-002 contracts `verified`.
- `check_architecture_contracts.py --strict-legacy` PASSED.

All criteria met. REC-C2 gate CLOSED.