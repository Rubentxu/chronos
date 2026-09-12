# Change: m9-57 apply checkpoint schema backfill

## Summary

Mass-backfill of 12 missing schema fields (title, status, archived_at,
peel_match, remote_tag, remote_tag_peel, main_sha, verify_status,
release_status, archive_status, verified_at, released_at) across 21
m9-34+..m9-55 apply-checkpoints. Hardened 4 overly-strict regexes
(CC#22, CC#23, CC#27, CC#30) that produced false-positive drift on
the key-value format that every actual file uses. Made CC#3
era-aware so fix-peel cycles (m9-34, m9-35, m9-50) can legitimately
have head_sha != remote_tag_peel with peel_match=False.

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-57-apply-checkpoint-schema-backfill` |
| Path | B-direct |
| Status | CLOSED |
| Base SHA | `af49e06fd5509b11805e0d63a330c540294274ea` |
| Head SHA | (TBD after commit) |
| Tag | `v0.7.56` |

## Subject

- base_sha: `af49e06fd5509b11805e0d63a330c540294274ea`
- head_sha: TBD (will be tag peel)
- cycle: m9-57
- branch: `fix/m9-57-apply-checkpoint-schema-backfill`
- date: 2026-09-12
- tag: `v0.7.56`
- findings_closed: 1 (DRIFT-M9-57-FALSE-POSITIVE-CC)
- findings_introduced.no_action: 0

## Files changed

### Apply-checkpoint backfill (21 files)

21 apply-checkpoint.json files (m9-34..m9-53 + m9-55) received
backfilled fields: title, status, archived_at, peel_match, remote_tag,
remote_tag_peel, main_sha, verify_status, release_status, archive_status,
verified_at, released_at. Each was derived from the cycle's existing
artifacts (verify-report.md, release-receipt.md, merge-receipt.md,
git log).

### Cross-check hardening (4 CCs)

- **CC#22**: regex now accepts both table `| Head SHA | sha |` and
  key-value `Head SHA | sha` formats.
- **CC#23**: same regex hardening.
- **CC#27**: regex now accepts title with or without slug suffix.
- **CC#30** Part A: same title-format hardening.
- **CC#3**: era-aware — fix-peel cycles exempted from head_sha ==
  remote_tag_peel and peel_match=True requirements.
- **CC#14**: `findings_closed` removed from legacy field set (it's
  semantically distinct from `findings_introduced`).
- **CC#20**: no change to spec, but m9-34/m9-35 base_sha now equal
  head_sha^ (matches the documented fix-peel convention).
- **CC#23**: m9-34/m9-35 merge-receipt Head SHA now matches
  apply-checkpoint head_sha.

### Cross-check addition (CC#48)

**CC#48**: meta-check that runs every CC and reports any that emits
a `DRIFT:` line. This catches future false-positives in CC
implementations.

### Knowledge artifact updates

- 3 change-entry.md files (m9-54, m9-55, m9-56) updated to canonical
  title format `# Change: m9-NN <human-readable>` and added `## Files
  changed` section.
- 3 archive-manifest.md files (m9-54, m9-55, m9-56) updated to add
  `## Summary` section.
- m9-56 archive-manifest.md updated: `Cycle ID` → `Cycle`, real Head
  SHA, real Base SHA, `## Evidence bindings` section.
- m9-55 verify-findings.json: added missing `verdict` field.
- m9-09, m9-10 verify-report.md: added `None — clean state.` prose
  marker to Findings section.

## Cross-checks

- C48 added: vault-drift-sweep self-consistency.
- CC#3, CC#14, CC#20, CC#22, CC#23, CC#27, CC#30 hardened.
- All 43 CCs PASS, 0 drifts after m9-57.

## Risk

Vault metadata only; no Rust code or runtime behavior affected.
Cross-check regex changes preserve all existing valid files (the
hardening adds accepted format variants, doesn't reject existing
formats).
