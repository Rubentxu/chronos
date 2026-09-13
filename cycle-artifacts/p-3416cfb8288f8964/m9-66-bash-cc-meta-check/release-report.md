# Release Report — m9-66-bash-cc-meta-check

## Path

B-direct

## Subject

Hardened vault drift detection by fixing two silently-broken bash CCs (CC#4 had a broken awk regex that prevented it from ever firing; CC#5 was off-by-16 since m6/m7/m8 milestone rows were added) and adding a third meta-check (CC#54, bash, sibling of CC#48) that brings the 7 bash CCs into the auto-validation pipeline. Side-effect: 54 stale SHAs that the now-functional CC#4 detected were regenerated across m9-01..m9-10 archive-manifests. check_vault_drift.sh refactored to invoke both CC#48 and CC#54.

## Files changed

| Group | Count | Change |
|---|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | 1 | CC#4 fix (regex); CC#5 fix (regex); CC#54 added (~120 lines) |
| `scripts/check_vault_drift.sh` | 1 | Refactored: invoke CC#48 + CC#54; capture bash_exit before \|\| true (~50 lines) |
| `archive/m9-01..m9-10/archive-manifest.md` | 10 | Regenerated 54 SHAs across 10 manifests |

Total: 12 files changed, 257 insertions(+), 69 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#1..CC#53 | All existing vault CCs | pass (no drift) |
| CC#54 | New bash meta-check (sibling of CC#48) | pass when run via check_vault_drift.sh |
| T0: cargo fmt + clippy | Lint gate | pass |
| T1: chronos-domain/-services/-browser unit tests | 263 passed | pass |
| T1: chronos-native unit tests | 99 passed | pass |
| T4-smoke: e2e_connectivity + probe_lifecycle (full file) | 1 + 5 = 6 passed | pass |

## Self-tests performed

1. Injected wrong `Total cycles` value in cycles/index.md (changed 65 to 99). Script: `DRIFT detected (CC#54, exit 1): DRIFT: CC#5: actual=81 declared=65`. Restored.
2. Injected wrong SHA in m9-01 archive-manifest.md. Script: `DRIFT detected (CC#54, exit 1): DRIFT: CC#4: .sddk-knowledge/.../archive-manifest.md :: ...`. Restored.
3. Verified clean state: `vault-drift-sweep: PASS (46 python CCs all clean, 7 bash CCs all clean)`.

## History

m9-66 was triggered by a session-end drift sweep that detected CC#5 was silently off-by-16 (Total cycles said 65 but the regex counted 81). Investigating CC#5's broken regex led to discovering CC#4 was also broken — opening the door to 54 stale SHAs accumulating across m9-01..m9-10 since the archive-manifest table format was introduced.

The pattern — "fix a broken CC, then have to fix what it would have caught" — is the second instance in this session (after m9-65 surfaced 49 stale branches that CC#46 was missing). It argues for a future "CC smoke test" cycle that periodically injects drift into each CC to confirm it's still firing. This is deferred.

The 54 stale SHAs were all in m9-01..m9-10 — the earliest cycles after the archive-manifest format was introduced in m9-11. m9-11 onward had correct SHAs. The drift accumulated because CC#4 was broken from inception, so no comparison ever happened.

m9-66 establishes the precedent that fixing a broken CC is part of the cycle that adds a new CC: every cycle that touches vault CCs should re-test every existing CC for similar silent failures.
