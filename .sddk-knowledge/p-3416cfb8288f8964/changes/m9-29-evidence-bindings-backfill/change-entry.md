# Change: m9-29 Evidence bindings section backfill

| Field | Value |
|---|---|
| Cycle | m9-29-evidence-bindings-backfill |
| Base SHA | `a65927d` |
| Head SHA | `8617ad0` |
| Tag | `v0.7.27` (peels to `8617ad0`) |
| Path | B-direct |
| Date | 2026-09-12 |
| Author | jcode (auto-mode B-direct) |

## Summary

m9-11 through m9-27 archive-manifest.md files were authored without
the canonical `## Evidence bindings` section. The agent in those
cycles used a different layout (`## Archived artifacts` +
`## SHA verification`).

This is a structural drift, not a content drift — the `## Artifact
index` / `## Artifact index (SHA-256)` section already lists the
SHA-256s of the artifacts. But the missing `## Evidence bindings`
section breaks the convention established by m9-01..m9-10 and m9-28,
where the Evidence bindings section is the primary evidence layer
that lets readers verify the archive's claims against the actual
files.

## Fix

1. Added `## Evidence bindings` section to each of 17 archive-manifest.md
   files (m9-11 through m9-27). Each section lists all cycle artifacts
   (cycle-artifacts/*.json + .md files) and the change-entry with their
   computed SHA-256, in the form:
   ```
   - <path> → <sha256>
   ```

2. Added cross-check #21 to vault-drift-sweep.md enforcing that all
   m9-* archive-manifests have a `## Evidence bindings` section. m9-01
   and m9-02 are exempt (use placeholder format from the pre-cycle-artifacts
   era).

## Cross-check

Cross-check #21 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- All 21 cross-checks: PASS
- 17 archive-manifest.md files each verified to have `## Evidence bindings`
  section
- Each artifact in those sections has its computed SHA-256 matching the
  actual file

## Lessons

The drift was caused by an agent who authored multiple cycles (m9-11
through m9-27) using a slightly different archive-manifest structure
than the original convention. m9-28 restored the Evidence bindings
section for m9-28 itself; m9-29 backfills it for the 17 prior cycles
that drifted.

The pattern: **structural drift compounds silently**. None of the
prior cross-checks (C1-C20) detected the missing section because none
of them checked for it. m9-29 adds C21 specifically to catch this
class of drift in future cycles.

## Risk

None. Vault metadata only; no code or runtime behavior affected.
