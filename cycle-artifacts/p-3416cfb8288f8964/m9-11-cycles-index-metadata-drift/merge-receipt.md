# Merge Receipt — m9-11-cycles-index-metadata-drift

| Campo | Valor |
|---|---|
| Cycle ID | `m9-11-cycles-index-metadata-drift` |
| Path | B-direct |
| Branch | `fix/m9-11-cycles-index-metadata-drift` |
| Base SHA | `6120e98` (main @ start of cycle) |
| Head SHA | `cd0115fd8f942058cde109c72a975cab7ea7473c` |
| Tag | `v0.7.9` |
| Status | MERGED |

## Fix commit

| SHA | Subject |
|---|---|
| `cd0115fd8f942058cde109c72a975cab7ea7473c` | fix(m9-11): cycles/index.md Total cycles metadata drift (22→26) + add cross-check #5 to vault-drift-sweep procedure |

## What was fixed

The `Total cycles` metadata field in `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md`
was set to `22` but the file contained `26` data rows.

Drift history (from git log -p):
- After `6a55b04` (m9-06 archive): rows=22, declared=22 ✓
- After `5055396` (m9-07 archive): rows=23, declared=22 ✗ **drift introduced**
- After `c183ad1` (m9-08 archive): rows=24, declared=22 ✗
- After `0e1474a` (m9-09 archive): rows=25, declared=22 ✗
- After `e50b27e` (m9-10 archive): rows=26, declared=22 ✗

Four consecutive cycles (m9-07, m9-08, m9-09, m9-10) added rows to the
cycles index without bumping the `Total cycles` counter. The drift was
not caught by the previous session's "auto-mode exhausted" verdicts
because the standing `vault-drift-sweep.md` procedure (which they
introduced) had no cross-check for this metadata field.

## Procedure extension

This cycle adds **cross-check #5** to `vault-drift-sweep.md`:

```bash
actual=$(awk -F'|' '/^\| (m6 |m[7-9])/{c++} END{print c+0}' \
  .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
declared=$(awk -F'|' '/Total cycles/{gsub(/[ \t]+/, "", $3); print $3}' \
  .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
[ "$actual" = "$declared" ] && echo "OK: $actual == $declared" \
  || echo "DRIFT: actual=$actual declared=$declared"
```

The procedure is now self-referentially catching: it caught the drift
that the previous procedure could not.
