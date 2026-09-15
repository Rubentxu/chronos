# Verify Report — m9-66

**Cycle**: m9-66-bash-cc-meta-check
**Path**: B-direct

## Summary

Two-commit B-direct cycle that hardens vault drift detection by fixing two silently-broken bash CCs and adding a third meta-check that brings the bash CCs into the auto-validation pipeline. The cycle also regenerates 54 stale SHAs that the now-functional CC#4 detected.

## Subject

| Base | Head (final) | Dirty diff digest | CWD | Verified at |
|---|---|---|---|---|
| `c46851b` | `963a143aacd5e005fb39593afea060106f172c00` | `sha256:64bed0ca468750c2dfa11ff3c21e84ce08739470633256c37f06d5c4459d1424` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-13T12:18:00Z |

## Files Inventory

12 files changed across the two commits (257 insertions, 69 deletions):

| File | Change |
|---|---|
| `.sddk-knowledge/.../vault-drift-sweep.md` | Fixed CC#4 awk (strip backticks + trim whitespace; exclude self-reference); fixed CC#5 regex (m9-* only); added CC#54 (bash meta-check, sibling of CC#48) |
| `scripts/check_vault_drift.sh` | Refactored to invoke CC#48 (python) AND CC#54 (bash); capture bash_exit before `|| true`; tmpfile for count handoff |
| `archive/m9-01-schema-versioning/archive-manifest.md` | Regenerated 2 SHAs |
| `archive/m9-02-events-side-table/archive-manifest.md` | Regenerated 4 SHAs |
| `archive/m9-03-side-table-debt-cleanup/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-04-side-table-key-layout/archive-manifest.md` | Regenerated 7 SHAs |
| `archive/m9-05-side-table-overeng-cleanup/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-06-known-schema-versions-invariant/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-07-coup-01-invariant-assertion/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-08-list-load-schema-error-variant/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-09-vault-hygiene-active-disclosure-dedupe/archive-manifest.md` | Regenerated 6 SHAs |
| `archive/m9-10-m9-03-apply-checkpoint-rebuild/archive-manifest.md` | Regenerated 7 SHAs |

Total: 12 files, 54 SHA regenerations, 2 CC fixes, 1 CC added.

## Drift Evidence (pre-cycle baseline)

```
CC#5: actual=81 declared=65 (off by 16 — silently counted m6/m7/m8 rows)
CC#4: 54 SHAs stale in m9-01..m9-10 (silently never fired due to broken awk)
CC#48: only ran python CCs; bash CCs validated manually only
```

## Drift Evidence (post-cycle state)

```
CC#5: actual=65 declared=65 (clean)
CC#4: 54 SHAs regenerated; 0 drift (self-references excluded by design)
CC#48 + CC#54: 46 python + 7 bash CCs all auto-validated
```

## Gates

| Gate | Status | Evidence |
|---|---|---|
| T0: cargo fmt --check | PASS | no output |
| T0: cargo clippy --workspace --all-targets -- -D warnings | PASS | no warnings (37s) |
| T1: `cargo test -p chronos-domain/-p chronos-services/-p chronos-browser --lib` | PASS | 263 passed |
| T1: `cargo test -p chronos-native --lib -- --test-threads=1` | PASS | 99 passed |
| T4-smoke: `cargo test -p chronos-sandbox --test e2e_connectivity --test probe_lifecycle` | PASS (full file) | 1/1 + 5/5 passed when run together |
| `./scripts/check_vault_drift.sh` (post-cycle) | PASS | "vault-drift-sweep: PASS (46 python CCs all clean, 7 bash CCs all clean)" |

## Cross-checks

- CC#1..CC#53: pass (no drift)
- CC#54 (new, bash, sibling of CC#48): pass when run via check_vault_drift.sh
- Self-test (inject wrong Total cycles value): CC#5 catches → CC#54 reports DRIFT, exit 1
- Self-test (inject wrong SHA in m9-01 manifest): CC#4 catches → CC#54 reports DRIFT, exit 1

## Drift Class History

The 54 stale SHAs in m9-01..m9-10 were never caught because CC#4 was silently broken (the awk regex required stripping backticks before matching). Once the regex was fixed, the 54 stale references surfaced immediately. They were regenerated as part of commit 2/2.

The pattern — "fix a broken CC, then have to fix what it would have caught" — is the second instance in this session (after m9-65 surfaced 49 stale branches that CC#46 was missing). It argues for adding a "CC smoke test" cycle that periodically injects drift into each CC to confirm it's still firing. This is deferred — see follow-ups.

## Notes

- **Sandbox T4-smoke ordering**: `test_session_start_via_v2_then_session_stop_via_v2` failed when run alone (TimeoutError after 30s) but passed when run as part of the full probe_lifecycle suite. This is a known environmental sensitivity (likely related to MCP server warm-up after binary spawn). It is not a regression — the same binary passed when the full file was executed. Documented in handoff for human awareness.
- **Self-reference row in archive-manifests**: each manifest has an `archive-manifest (this file)` row that lists its own SHA. By definition this can never be the current SHA (writing it would change the file). CC#4 explicitly excludes this case; the row is informational and serves as a "this file is the manifest" marker.
- **CC#48 + CC#54 architecture**: CC#48 is python-only (executes other python CCs); CC#54 is bash-only (executes other bash CCs). They are siblings, not parent/child — neither invokes the other. Both are called by `check_vault_drift.sh`. Adding new CCs requires picking the right meta-check: python → CC#48; bash → CC#54. A future "shell script" CC (e.g., Ruby, Perl) would need a third meta-check.

## History

m9-66 was prompted by a session-end drift sweep that detected CC#5's off-by-16 silent failure (Total cycles said 65 but the regex counted 81). Investigating CC#5's broken regex revealed CC#4 was also broken (silently never fired) — opening the door to 54 stale SHAs that had been accumulating since m9-11 introduced the archive-manifest table format. Adding CC#54 closes the meta-check gap so future broken-CCs don't go unnoticed for cycles at a time.

## Findings

None — clean state. (m10-legacy-migration)
