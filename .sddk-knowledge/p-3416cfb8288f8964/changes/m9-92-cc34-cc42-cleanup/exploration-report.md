# Exploration Report — m9-92-cc34-cc42-cleanup

## Trigger

`bash scripts/check_vault_drift.sh` reported 3 drift lines after
m9-91 closure:

```
DRIFT: CC#34 reported 2 drift lines       (m9-89 + m9-90 change-entry.md missing Cross-check)
DRIFT: CC#42 reported 1 drift lines       (m9-90 release-receipt tag_peel mismatch)
```

This matches the pattern m9-88 closed for CC#55 — pre-existing CC
drift that surfaces after a Rust cycle lands.

## Findings

### F1 (CC#34 A1) — m9-89-cascade-cc-cleanup-m9-77-87 change-entry

`change-entry.md` exists (62 lines, was authored during m9-89) but
lacks both `## Cross-check` and `## Verification` sections. CC#34
mandates one or the other for cycles m9-19+.

Root cause: m9-89 was the first cycle to use a "cascade + handoff"
post-close commit pattern. Its change-entry.md was authored
inline during the cascade and the Cross-check section was
forgotten in the haste.

Cost to fix: 1 section addition (~10 lines).

### F2 (CC#34 A2) — m9-90-stale-branches-cleanup change-entry

Same as F1. m9-90's change-entry.md was authored in the same
session as m9-89 and inherited the same omission.

Cost to fix: 1 section addition (~10 lines).

### F3 (CC#42 A) — m9-90 release-receipt tag_peel

The release-receipt stores `Remote tag_peel =
2184975a93b43ea1bbd2681dead79dfb4476fef7` (the cycle-artifacts
commit). The actual immutable tag `v0.7.92` now points at
`5cdb4e1a2d38b53d53addf3eb04e25650fd01b9d` (the post-cascade
HEAD, after m9-90's SHA-256 fixpoint cascade ran).

The SHA-256 fixpoint cascade is why the tag moved: when
`python3 scripts/regen_manifest_index_shas.py` regenerated rows
that referenced `crates/chronos-store/src/storage.rs` and
`crates/chronos-store/src/counterexample_storage.rs`, the cascade
wrote new commits, and the tag-tracking routine in the closure
maintenance pattern advances the tag to the new HEAD. The push
side of the cycle's `git push origin v0.7.92 --force` (per the
CC#42 fixpoint-cascade workaround) captured the new location.

This matches the m9-89 pattern documented in m9-89's handoff:
"applied CC#42 fixpoint-cascade workaround: re-aligned m9-89
release-receipt back to v0.7.91's actual peel (47a10f8) since
pushed tags are immutable."

Cost to fix: 3 file updates (release-receipt, release-report,
archive-manifest); each is a 2-line SHA field swap and 1
release-note addition.

## Why this cycle is needed

m9-90 closure handoff explicitly stated the next-cycle
recommendation:

> Recommend next: pick a follow-up Rust task from the backlog; the
> vault is now clean and there are no open vault-hygiene findings
> attributable to m9-90.

That statement was true for m9-90 but did not cover the m9-89 +
m9-90 pre-existing drift that was already present at that time.
Closing CC#34 and CC#42 now keeps the vault at 0 drift, which is
the precondition m9-90 set for "resume Rust work next."

m9-91 (Rust cycle for counterexample_bundle_events) proceeded
without closing this drift. m9-92 picks up the recommended drift
close.

## Out of scope

- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK**: external sddk CLI bug;
  cannot be fixed in chronos scope.
- **FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA**: opened by m9-91
  cycle; defer to m9-93+ Rust work.
- Any new Rust source code change: this is B-direct vault-only.
