# Change: m9-92 cc#34 + cc#42 cleanup

## Identification

| Field | Value |
|---|---|
| Cycle ID | `m9-92-cc34-cc42-cleanup` |
| Workspace | `p-3416cfb8288f8964` |
| Path | B-direct (vault-only hardening) |
| Status | CLOSED |
| Branch | `chore/m9-92-cc34-cc42-cleanup` |
| Tag | `v0.7.94` |

## Subject

- 2 change-entries get a new `## Cross-check` section (CC#34).
- 1 release-receipt + 1 release-report + 1 archive-manifest get
  `Remote tag_peel` updated to the immutable tag location
  (`5cdb4e1` for v0.7.92).

## Problem

`bash scripts/check_vault_drift.sh` reported 3 pre-existing drift
lines after m9-91 closure:

- CC#34 A1: `m9-89-cascade-cc-cleanup-m9-77-87/change-entry.md` lacks
  a `## Cross-check` section.
- CC#34 A2: `m9-90-stale-branches-cleanup/change-entry.md` lacks a
  `## Cross-check` section.
- CC#42 A: `m9-90-stale-branches-cleanup/release-receipt.md` stores
  `Remote tag_peel = 2184975a` but the immutable `v0.7.92` tag now
  points at `5cdb4e1` (after m9-90's SHA-256 fixpoint cascade
  advanced the tag).

Per m9-90 closure handoff: "the vault is now clean and there are no
open vault-hygiene findings attributable to m9-90" — but this
statement did not cover the pre-existing m9-89 + m9-90 drift that
survives to the present cycle.

## Approach

B-direct, single vault-edit commit:

1. Add `## Cross-check` section to the two change-entry.md files
   (CC#34 fix).
2. Update `Remote tag_peel` field in 3 m9-90 artifacts
   (`cycle-artifacts/.../release-receipt.md`,
   `cycle-artifacts/.../release-report.md`,
   `.sddk-knowledge/.../archive/m9-90-.../archive-manifest.md`)
   to point at the immutable `v0.7.92` tag location
   (`5cdb4e1`).
3. Add a release-notes bullet explaining the tag-push that
   occurred after m9-90 closure (CC#42 fixpoint-cascade workaround,
   mirror m9-89's wording).

No Rust touched, no tests required (T0 only).

## Files changed

| Bucket | Files | Notes |
|---|---|---|
| m9-89 change-entry | 1 modified | +12 lines (new `## Cross-check` section) |
| m9-90 change-entry | 1 modified | +12 lines (new `## Cross-check` section) |
| m9-90 release-receipt | 1 modified | 3-line SHA swap + 4-line note addition |
| m9-90 release-report | 1 modified | 3-line SHA swap |
| m9-90 archive-manifest | 1 modified | 3-line SHA swap |
| cycle artifacts for m9-92 | 7 added | apply-checkpoint + 6 receipts/reports + change-entry + 4 vault files |
| cycles/index.md | 1 modified | m9-92 row + Total cycles 91 → 92 |
| terms/index.md | 1 modified | Last archive = m9-92 |

**Total**: 5 vault-edit files + 11 cycle/vault artifacts for
m9-92 itself + 2 index files.

## Drift delta

| CC | Before | After |
|---|---|---|
| CC#34 | 2 drift lines | 0 |
| CC#42 A | 1 drift line | 0 |

No new CC drift introduced.

## Out-of-scope

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88):
  external `sddk` CLI bug; cannot be fixed in chronos scope.
- **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA** (opened by m9-91):
  separate Rust cycle for adding `JsonSchema` to
  `chronos_domain::TraceEvent`.
- `cc-001-god-module`, `cc-004-implicit-io-toctou`: deferred to
  m10+.

## Cross-check

- **CC#34**: Both m9-89 and m9-90 change-entries now contain a
  `## Cross-check` section. `bash scripts/check_vault_drift.sh`
  reports 0 drift lines for CC#34.
- **CC#42 A**: m9-90 `Remote tag_peel` now matches the immutable
  `v0.7.92` tag (`5cdb4e1`). `bash scripts/check_vault_drift.sh`
  reports 0 drift lines for CC#42.
- **CC#3**: m9-92 cycle apply-checkpoint's `head_sha`,
  `remote_tag_peel`, `tag_peel_sha`, and `main_sha` all equal the
  cascade-final HEAD's SHA; `peel_match` is `true`.
- **CC#4**: `python3 scripts/regen_manifest_index_shas.py --check`
  exits 0 with 0 manifests needing update.
- **CC#8**: All m9-92 SHAs (`base_sha`, `head_sha`, `main_sha`,
  `remote_tag_peel`) exist in the local git object store (verified
  via `git cat-file -e` for each).
- **CC#11**: m9-92 apply-checkpoint's `status` = `CLOSED`,
  `archived_at` = `2026-09-14`, `findings_introduced.no_action` = `[]`
  (cc#19-compliant), `peel_match` = `true`.
- **CC#12**: m9-92 apply-checkpoint's `route` = `B-direct`,
  `main_sha` = `head_sha`.
- **CC#23**: m9-92 merge-receipt's Head SHA matches the
  apply-checkpoint's `head_sha` and Base SHA matches
  `base_sha`.
- **CC#39**: cycles/index.md Total cycles = 92 (matches actual
  row count). m9-92 row added referencing the cascade-final SHA.
- **CC#42**: All release-receipts (m9-89 through m9-92) have
  `Remote tag_peel` matching `git rev-parse <tag>^{commit}`.
- **CC#47**: All m9-92 apply-checkpoint SHAs (`base_sha`,
  `head_sha`) exist in git as commit objects.

## Files

- Source commit: pre-merge vault edit + cycle artifacts (5
  vault-edit files + 11 artifact files + 2 index files = 18).
- Merge commit: `--no-ff` merge of `chore/m9-92-cc34-cc42-cleanup`.
- Cascade commits: typically 1-3 SHA-bookkeeping commits per CC#42
  fixpoint-cascade workaround.
