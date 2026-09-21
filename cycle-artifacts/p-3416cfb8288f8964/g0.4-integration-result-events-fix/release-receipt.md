# Release Receipt — g0.4-integration-result-events-fix

## Identification

| Field | Value |
|---|---|
| Cycle | g0.4-integration-result-events-fix |
| Path | A-min, evidence-backed closure of C5.2 sandbox-client gap |
| Base SHA | c81ca08c7a19956c6bda2ee3bbaba0399c7aec7d |
| Head SHA | b44504edfd8b93ad7c4f3d4d30aeea4181c570ac |
| Main SHA | a28421c5a34d96abd7d9b92dfa6d1f9e799e3305 |
| Remote tag | v0.7.112 |
| Remote tag_peel | 0be2ec2d53d9698956ae705938b32b80d7365ad7 |

## Release result

G0.4 ships a 7-file sandbox-client fix that closes the C5.2 migration
gap (`events_read(mode=query)` response now correctly read from
`result.events` v2 envelope instead of root-level `events`). T1 sandbox
integration unblocked: 36/36 tests passing (12 lib + 24 integration
across 7 suites).

The cycle does NOT create a new release tag (G0 is a sub-slice of
REC-C7 baseline; the canonical `v0.7.112` peel `0be2ec2d` is preserved
intact and remains the reference for downstream work).

| Peel match | true |
