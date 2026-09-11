# Release Receipt: m9-02-events-side-table

## Cycle

| Field | Value |
|---|---|
| cycle_id | m9-02-events-side-table |
| path | A-lite |
| tag | v0.7.0 |
| tag_sha | 1a8d104ba2da883b40bd424cdc079b344f7ed63c |
| main_sha | 1a8d104ba2da883b40bd424cdc079b344f7ed63c |

## Tag Evidence

```bash
# Local tag type
$ git cat-file -t refs/tags/v0.7.0
tag

# Local tag peel
$ git rev-parse refs/tags/v0.7.0^{}
1a8d104ba2da883b40bd424cdc079b344f7ed63c

# Remote tag peel
$ git ls-remote origin "refs/tags/v0.7.0^{}"
1a8d104ba2da883b40bd424cdc079b344f7ed63c
```

## Tag Push Receipt

```
To github.com:Rubentxu/chronos.git
 * [new tag]         v0.7.0 -> v0.7.0
```

## Annotated Tag Message

```
feat(m9-02): bundle events side table — lazy-loading + legacy-compat (schema v2)
```

## Contract Compliance

- Rule 3: Annotated tag marks release ✓
- Rule 4: release-receipt recorded at 2026-09-11T20:34:51Z ✓
- Rule 5: No optional post-tag distribution required ✓
- Rule 6: Recovery — no interruption, clean first-pass ✓

## Chain Link

release-receipt → archive (sddk-archive)
