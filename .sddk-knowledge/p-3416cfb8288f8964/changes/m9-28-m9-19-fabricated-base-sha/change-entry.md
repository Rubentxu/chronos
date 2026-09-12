# Change: m9-28 m9-19 base_sha fabrication fix


## Subject

- base_sha: `8eb648779071f9a634e2439754034f1288929c62`
- head_sha: `709e3c470d50064b9a2c819b5761a2972eadafcb`
- cycle: m9-28
- tag: `v0.7.26`
- route: B-direct
- date: 2026-09-12


## Summary

m9-19's `apply-checkpoint.json` was authored with a fabricated
`base_sha = 6dce3739b7e2f0fcbdb2c10c0a35b27cfb2b8a37`. The 40-char
SHA looked plausible (starts with `6dce373`, the m9-18 tag short
prefix) but does not exist in the repository.

Cross-check #8 (apply-checkpoint SHA fields exist in repo) caught
this when running `git cat-file -e 6dce3739b7e2...` which exits 1.

The real `base_sha = 735c57b7178c93ea25f9cb603a3cb97b9ca7f81e` is
the parent of m9-19's fix commit (`ec58934...`), i.e., the
`docs(m9-18): add release receipts + vault archive + backfill m9-18
published SHA` commit. This is the commit m9-19's branch actually
diverged from.

## Fix

1. Replaced m9-19's fabricated `base_sha` with the real value
   `735c57b7178c93ea25f9cb603a3cb97b9ca7f81e`.
2. Updated m9-19's change-entry `Base SHA` short form from `6dce373`
   to `735c57b`.
3. Added cross-check #20 to `vault-drift-sweep.md` enforcing
   `base_sha == head_sha^` for fix-peel cycles (m9-07+) while
   exempting docs-peel cycles (m9-03..m9-06) which intentionally
   branched off docs commits.

## Cross-check

Cross-check #20 added to
`.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`.

## Verification

- C8 (raw `git cat-file -e` check): PASS after fix
- C20 (era-aware `base_sha == head_sha^` check): PASS for all
  21 fix-peel cycles (m9-07..m9-28)
- C7 (change-entry Head/Base SHA match apply-checkpoint): PASS after
  m9-19's change-entry short form is updated
- All other 17 cross-checks: PASS

## Lessons

C8 catches fabrication but only when the fabricated SHA doesn't even
exist via `git cat-file -e`. The m9-19 fabricated SHA was a near-miss:
it didn't exist in the repo (caught), but if it had been a real
collision with some other commit (or just a plausible-looking 40-char
string), C8 would still catch it because the value wouldn't equal
the real `base_sha` — but only if someone noticed and compared.

C20 is the preventive check: it asserts the **rule** (`base_sha ==
head_sha^` for fix-peel cycles) so even near-miss fabrication is
caught mechanically. The era-awareness (fix-peel vs docs-peel) is
critical because docs-peel cycles (m9-03..m9-06) intentionally have
`base_sha != head_sha^`.

The deeper lesson: **m9-19's apply-checkpoint was authored with a
short SHA guessed from the m9-18 tag, then padded to 40 chars**. This
is the same class of error as m9-11's `cd0115f8c93b...` fabrication
(caught by m9-14) — both come from "look up the prior tag, take the
short SHA, write it down" without verifying the actual 40-char form.
The fix is always: use `git rev-parse <short-sha>^` to get the real
parent. C20 enforces this structurally for fix-peel cycles.

## Files changed

- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/merge-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/release-receipt.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/release-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/m9-28-m9-19-fabricated-base-sha/verify-report.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-28-m9-19-fabricated-base-sha/archive-manifest.md`
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-28-m9-19-fabricated-base-sha/change-entry.md`
- `.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md`


## Risk

None. Vault metadata only; no code or runtime behavior affected.
