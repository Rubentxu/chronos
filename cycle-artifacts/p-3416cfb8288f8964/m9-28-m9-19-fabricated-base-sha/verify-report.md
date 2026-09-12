# Verify Report — m9-28

## Cross-checks

- C1: PASS
- C2: PASS (closed-by-design: m8-04-R4 and m8-07-R2 are pre-reorg cycles)
- C3: PASS (m9-19 head_sha and remote_tag_peel still match after base_sha fix)
- C4: PASS (no archive-manifest SHAs to drift)
- C5: PASS (cycles 44 == 44)
- C6: PASS (terms Last archive = m9-28-m9-19-fabricated-base-sha == cycles last)
- C7: PASS (change-entry Head/Base SHA match apply-checkpoint after fix)
- C8: PASS (all SHA fields exist in repo, including m9-19's new base_sha)
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
- C20: PASS (NEW — base_sha == head_sha^ for all fix-peel cycles)

## Findings

None — clean state.

## Notes

m9-19's base_sha was fabricated (`6dce3739b7e2f0fcbdb2c10c0a35b27cfb2b8a37`).
Cross-check #8 caught it via `git cat-file -e` (the SHA did not exist in the
repo). The real value `735c57b7178c93ea25f9cb603a3cb97b9ca7f81e` is the parent
of m9-19's fix commit (`ec58934...`), i.e., the docs(m9-18) commit that
finalized m9-18's vault archive work.

C20 prevents recurrence: it enforces `base_sha == head_sha^` for fix-peel
cycles (m9-07+ where tags peel to a `fix(m9-XX): ...` commit). Docs-peel
cycles (m9-03..m9-06) are exempted because they branched off docs commits
not fix commits.
