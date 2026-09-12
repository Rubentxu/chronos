# m9-19: Release Receipt

| Field | Value |
|---|---|
| Cycle | m9-19-route-main-sha-and-empty-folder-drift |
| Tag | `v0.7.17` |
| Pee | fix commit (chronos convention) |
| Released at | 2026-09-12T11:11:38Z |

## Tag command

```
git tag -a v0.7.17 -m "m9-19: route field normalization + main_sha convention + empty folder cleanup"
git push origin v0.7.17
```

## Note on peel convention

Following the m9-11+ convention, `v0.7.17` peels to the fix commit
(the HEAD of the cycle branch), preserving runtime semantics on the
tagged SHA. Pre-m9-11 tags peeled to docs commits; that intentional
drift is left as accepted-by-design.
