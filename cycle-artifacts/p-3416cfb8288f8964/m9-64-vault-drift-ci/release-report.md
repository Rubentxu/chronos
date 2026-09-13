# Release Report — m9-64-vault-drift-ci

## Path

B-direct

## Subject

Closed a CI gap: the 52 vault CCs only ran manually during SDDK cycles. A regression in any vault file would not be caught until the next session's drift sweep (typically days later). Added `.github/workflows/vault-drift.yml` that triggers on push to main and PRs touching vault files, running `scripts/check_vault_drift.sh`. The script extracts the CC#48 meta-check from `vault-drift-sweep.md` and executes it; exits 1 on drift, 0 on clean.

## Files changed

| Group | Count | Change |
|---|---|---|
| `.github/workflows/vault-drift.yml` | 1 | New CI workflow (37 lines) |
| `scripts/check_vault_drift.sh` | 1 | Bash entry point (61 lines) |

Total: 2 files changed, 98 insertions(+), 0 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#1..CC#52 | Vault schema and cross-reference checks | pass (no drift) |
| Self-test | Synthetic m9-99 drift injection caught by script (exit 1) | pass |

No new CC added — the cycle uses the existing CC#48 as the entry point.

## History

Discovered during a session-end sweep that confirmed 52 vault CCs all pass but had no automation. The drift sweep is run manually at the start of each session (per `vault-drift-sweep.md` §Trigger) and at the end of each SDDK cycle's verify phase, but in between cycles (typically several days), any push to a vault file could introduce drift that goes undetected until the next cycle.

m9-64 closes that gap with a minimal-surface fix: 1 workflow + 1 script, no source code changes, no API surface change, no test runtime impact (the script runs in <10s).

The workflow uses `actions/setup-python@v5` with Python 3.11 (matches local SDDK cycle environment). It triggers only on push to main and PRs touching the specific vault paths — code-only changes do not trigger it.
