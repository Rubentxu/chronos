# Verify Report — m9-29

## Subject

- base_sha: `a65927d960d7a5b42cd425dc34befe3efe7c692f`
- head_sha: `8617ad0dde11f1a9e83a7f3c110dda3d691e374d`
- cycle: m9-29

## Cross-checks

- C1: PASS
- C2: PASS (closed-by-design: m8-04-R4 and m8-07-R2 are pre-reorg cycles)
- C3: PASS (all 22 fix-peel cycles have head_sha == remote_tag_peel)
- C4: PASS (all 26 archive-manifest Artifact index SHA-256s match)
- C5: PASS (cycles 45 == 45)
- C6: PASS (terms Last archive = m9-29-evidence-bindings-backfill == cycles last)
- C7: PASS (change-entry Head/Base SHA match apply-checkpoint)
- C8: PASS (all SHA fields exist in repo)
- C9: PASS
- C10: PASS
- C11: PASS
- C12: PASS
- C13: PASS
- C14: PASS
- C15: PASS
- C16: PASS
- C17: PASS
- C18: PASS
- C19: PASS
- C20: PASS (base_sha == head_sha^ for all fix-peel cycles)
- C21: PASS (NEW — all 26 m9-* archive-manifests have `## Evidence bindings` section)

## Findings

None — clean state.

## Notes

m9-11..m9-27 archive-manifests were missing the canonical `## Evidence bindings`
section. m9-29 backfills it for all 17 cycles with one bullet per cycle artifact
(path → SHA-256).
