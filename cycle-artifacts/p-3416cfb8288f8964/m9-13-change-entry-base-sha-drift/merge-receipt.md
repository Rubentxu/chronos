# Merge Receipt — m9-13-change-entry-base-sha-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-13-change-entry-base-sha-drift` |
| Path | B-direct |
| Branch | `fix/m9-13-change-entry-base-sha-drift` |
| Base SHA | `bfb9ede` (main @ start of cycle) |
| Head SHA | `26848cf` |
| Tag | `v0.7.11` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `26848cf` | fix(m9-13): m9-11 change-entry Base SHA drift (cd0115f → 6120e98) + add cross-check #7 to vault-drift-sweep |

## What was fixed

The `Base SHA` field in
`.sddk-knowledge/p-3416cfb8288f8964/changes/m9-11-cycles-index-metadata-drift/change-entry.md`
was `cd0115f` (the fix commit) but the apply-checkpoint correctly had
`base_sha = 6120e98` (the parent of the fix).

The change-entry was authored with a confusing note ("the m9-11 fix
commit itself, since this is a single-commit cycle") but the base
should be the **parent** of the fix (the main HEAD before the cycle
started), not the fix itself.

This is a documentation drift — the change-entry's Base SHA was
incorrectly self-referential. m9-13 corrects it to the proper parent
SHA `6120e98`.

## Procedure extension

This cycle adds **cross-check #7** to `vault-drift-sweep.md`. The check
parses change-entry `Head SHA` and `Base SHA` fields and verifies they
are prefixes of the corresponding fields in `apply-checkpoint.json`.

This catches copy-paste errors where the change-entry was authored with
the wrong SHA prefix (e.g. head vs base confusion, or using the fix
SHA instead of the parent).
