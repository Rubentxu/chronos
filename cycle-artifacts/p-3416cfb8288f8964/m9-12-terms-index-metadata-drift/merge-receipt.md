# Merge Receipt — m9-12-terms-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-12-terms-index-metadata-drift` |
| Path | B-direct |
| Branch | `fix/m9-12-terms-index-metadata-drift` |
| Base SHA | `f4818d1` (main @ start of cycle) |
| Head SHA | `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` |
| Tag | `v0.7.10` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` | fix(m9-12): terms/index.md Last archive metadata drift (m9-10 → m9-11) + add cross-check #6 to vault-drift-sweep |

## What was fixed

The `Last archive` metadata field in `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md`
was pointing at `m9-10-m9-03-apply-checkpoint-rebuild` but a more recent
cycle (`m9-11`) had been recorded in `cycles/index.md`.

Drift history (from git log -p):
- After `e50b27e` (m9-10 archive): terms Last archive = m9-10 ✓
- After `cd0115f` (m9-11 archive): terms Last archive = m9-10 ✗ **drift introduced**
- After `0012f1242cef949efc4cbd4c8d419a135ee3cf8a` (m9-12 fix): terms Last archive = m9-11 ✓

m9-11 added itself to `cycles/index.md` but did not bump `terms/index.md`'s
`Last archive` pointer. This is the same kind of metadata drift that
m9-11 fixed for `cycles/index.md` (`Total cycles` field), but in a
different file. m9-11's check #5 covered cycles-index only; m9-12 adds
check #6 to cover terms-index too.

## Procedure extension

This cycle adds **cross-check #6** to `vault-drift-sweep.md`:

```bash
last_archive_in_terms=$(awk -F'|' '/Last archive/{gsub(/[ \t]+/, "", $3); print $3}' \
  .sddk-knowledge/p-3416cfb8288f8964/terms/index.md)
last_closed_cycle=$(awk -F'|' '/^\| m[0-9]+/{gsub(/[ \t]+/, "", $3); last=$3} END {print last}' \
  .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
[ "$last_archive_in_terms" = "$last_closed_cycle" ] \
  && echo "OK: $last_archive_in_terms == $last_closed_cycle" \
  || echo "DRIFT: terms=$last_archive_in_terms cycles=$last_closed_cycle"
```

The procedure is now self-referentially catching: check #6 catches
the same kind of drift that m9-11 introduced (and m9-11 itself missed
because it didn't have check #6 yet).
