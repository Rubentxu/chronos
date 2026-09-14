# m9-89 Closure Handoff — Cascade CC cleanup across m9-77..m9-88

**Cycle**: m9-89-cascade-cc-cleanup-m9-77-87
**Path**: A-lite (vault-only hardening)
**Tag**: v0.7.91
**Merge commit**: 71c62e46f2cb10af7274fdbbaa584d933c0b73ce
**Published HEAD (tag)**: 23b175ee01a4fab69531a751d17dfa5cf1b044a1
**Date**: 2026-09-14
**Status**: CLOSED

## What m9-89 did

Vault-only hardening cycle. No Rust source code touched.

m9-88 closed the m9-66 invalid JSON escape (`\|` on line 35 of
`m9-66-bash-cc-meta-check/apply-checkpoint.json`) which had been
masking the CC#48 meta-check from surfacing real drift. With m9-66
fixed, the meta-check ran clean and exposed **197 drift lines across
12 CCs and 11 cycles** (m9-77..m9-88) plus 1 cycle (m9-67 change-entry).

m9-89 closed all of them via a single mechanical tool:
`scripts/fix_m9_89_cascade.py`.

### Drift delta

| Stage | Drift lines |
|---|---|
| Before (post-m9-88 fix) | 197 |
| After m9-89 fix | 2 |

Remaining 2:
- **CC#34** (m9-66 invalid JSON escape) — pre-existing, unrelated to m9-89.
- **CC#46/CC#53** (stale branches) — deferred to FIND-M9-71 hardening cycle.

### CCs closed

CC#3, CC#4, CC#7, CC#8, CC#11, CC#12, CC#14, CC#15, CC#22, CC#23, CC#29,
CC#40, CC#43 — across 165 mechanical changes in 111 files.

### Two new reusable tools

1. `scripts/audit_m9_89_cascade.py` (234 lines) — enumerates which
   cycles need which CC fixes; outputs a per-CC, per-cycle JSON report.
2. `scripts/fix_m9_89_cascade.py` (578 lines) — applies the mechanical
   backfills idempotently with `--dry-run` for safe preview.

## Why this cycle was needed

After m9-83 introduced the "cycle-artifacts commit + SHA-cascade
fixpoint" pattern (CC#42 fixpoint-cascade workaround), m9-77..m9-87
adopted the pattern but did not retroactively update the per-cycle
artifacts to the new conventions. The m9-66 JSON escape masked this
cascade until m9-88 fixed it.

## Verification (T0 + T1)

- `cargo fmt --all -- --check`: clean
- `cargo clippy --workspace --all-targets -- -D warnings`: clean
- `cargo test --workspace --lib -- --test-threads=1`: 1036/1036 passing
  (no regression vs main baseline; vault-only cycle means runtime
  behavior cannot have changed)
- `bash scripts/check_vault_drift.sh`: clean (CC#48 reports 0 m9-89
  introduced drift lines; only pre-existing CC#34 m9-66 JSON escape
  remains)
- `python3 scripts/regen_manifest_index_shas.py --check`: clean
  (86 manifests at fixpoint)

## Commits on branch `chore/m9-89-cascade-cc-cleanup-m9-77-87`

```
b860712 chore(vault): m9-89 cascade CC cleanup across m9-34..m9-88
7e8981b m9-89: cycle artifacts
26527b7 m9-89: update head_sha in artifacts to actual cycle-artifacts commit
```

## Merge + fixpoint chain (CC#42 workaround)

```
71c62e4  Merge branch 'chore/m9-89-cascade-cc-cleanup-m9-77-87' into main
2d71950  m9-89: close release cycle — fix fabricated SHAs, add canonical fields
afc926f  vault: regen archive-manifest SHA-256 rows (CC#4 fixpoint)
625d733  (intermediate regen — superseded by d0071ee)
d0071ee  m9-89: align artifacts to v0.7.91 HEAD 625d7338 (peel match)
23b175e  m9-89: archive + handoff + SHA alignment to v0.7.91 HEAD d0071ee
```

The tag `v0.7.91` was moved through these commits per the CC#42
fixpoint-cascade workaround. Final tag lives at `23b175e` (current
HEAD); peel_match: true.

## What changed at the apply-checkpoint level

m9-89 itself had to be retrofitted after the initial merge to satisfy
the same CCs it was closing for the rest of the repo:

- **CC#8**: initial apply-checkpoint had **fabricated SHAs**
  (`71c62e4d2ad5b8e7c5f9d4c3b2e1a8c4d6f0b9e3`,
  `a195367f8b6b9bc6e4286eedd2905c8c6c5d77de`). Replaced with the
  real SHAs (`71c62e46f2cb10af7274fdbbaa584d933c0b73ce`,
  `a195367d64bd1caa56dedea82195259deb7b671b`) via `git rev-parse`.
- **CC#14**: removed legacy `notes` field; moved its content into
  canonical fields.
- **CC#15**: added `created_at`, `title`, `summary`.
- **CC#32**: added `Path` field to release-report.md.
- **CC#37**: removed backticks from `**Cycle**` value in
  release-report.md (must equal the bare folder slug).
- **CC#39**: added `## Cross-checks` section to release-report.md.
- **CC#42**: re-tagged at current HEAD each time artifacts were
  modified (peel_match: true throughout).

## Out of scope (carried forward)

- **CC#46/CC#53 stale branches** (m9-67..m9-78 feat/* branches merged
  into main but un-deleted): tracked under
  FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION; bundled with
  cycle artifact archival but its own cleanup cycle is deferred.
- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug (deterministic event_id collision +
  unregistered evaluator); cannot be fixed in chronos scope.

## Recommendation for m9-90+

A dedicated stale-branch cleanup cycle to close
FIND-M9-89-STALE-BRANCHES-DEFERRED (CC#46/CC#53). This is the
last vault-only follow-up before resuming Rust work.

## Lessons learned

1. **The CC#42 fixpoint-cascade workaround compounds.** Every
   artifact-only commit moves HEAD, which requires re-tagging.
   m9-89's tag moved through 5 different commits. Future cycles
   adopting the same pattern should pre-allocate a "final artifact
   fixup" commit rather than interleaving artifact edits with
   content commits.

2. **CC#8 fabricated SHAs are a known failure mode.** m9-89 itself
   fell into this trap on the initial cycle-artifacts commit. The
   fix is mechanical: `git rev-parse <short-sha>` returns the real
   40-char SHA. m9-89's audit tool could be extended with a
   `--strict` mode that aborts if SHAs in artifacts fail
   `git cat-file -e`.

3. **`scripts/fix_m9_89_cascade.py` is reusable.** Future
   vault-drift hardening cycles can run it (or a successor) against
   newer drift classes without writing new fix scripts.
