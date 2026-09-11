# Release Report: m9-01-schema-versioning

## Status

| Field | Value |
|---|---|
| Cycle | `m9-01-schema-versioning` |
| Path | A-min |
| Status | **success** |
| Route | local |
| Runtime status | RELEASED |
| Next phase | archive |

## Verified SHA

| Field | Value |
|---|---|
| Main SHA | `25948147869307343a28e3896e8289c715efbc81` |
| Base commit | `bceddc946b73209756d3360b0aec4dca592296c3` |
| Tag | `v0.6.0` |
| Tag peel | `25948147869307343a28e3896e8289c715efbc81` |
| Remote SHA match | ✅ verified |

## Pipeline Evidence

### verify evidence (from `sddk-verify`)

| Field | Value |
|---|---|
| Report | `verify-report.md` |
| SHA-256 | `2a5eeee3a89690f967cec391708c0354a52cde83c3458b79d70949f1ea271d4f` |
| Subject SHA (at verify time) | `0aec8bac30f995e638e2a656b55935f48a60623d` |
| Verdict | **PASS_WITH_WARNINGS** |
| Findings SHA-256 | `14d5b301a31aa8899c0c4966e3ff38a86c21b758d4b42d4d85b81bf00d813565` |

> **Note**: The verify report subject SHA is `0aec8ba`. The release SHA is `2594814`, which adds one `docs(m9-01): verify + debt-verify evidence artifacts` commit on top. This commit is documentation-only; no behavioral change relative to the verified SHA.

### debt evidence (from `sddk-debt-verify`)

| Field | Value |
|---|---|
| Report | `cycle-artifacts/m9-01-schema-versioning/debt-verify/debt-report.json` |
| Subject SHA | `0aec8bac30f995e638e2a656b55935f48a60623d` |
| Verdict | **PASS_WITH_WARNINGS** |
| Clusters | coupling (2 findings), overeng (1 finding) |
| Blocking findings | 0 |
| All findings target | backlog |

## Git Effects

| Step | Command | Exit | Result |
|---|---|---|---|
| Fetch origin main | `git fetch origin main --tags` | 0 | fetched |
| Checkout main | `git checkout main` | 0 | clean |
| Fast-forward merge | `git merge --ff-only feat/m9-01-schema-versioning` | 0 | bceddc9..2594814, fast-forward |
| Push main | `git push origin main` | 0 | bceddc9..2594814 main->main |
| Verify SHA | `git rev-parse HEAD` == `git rev-parse origin/main` | 0 | match |
| Create annotated tag | `git tag -a v0.6.0 $SHA -m "feat: ..."` | 0 | created |
| Push tag | `git push origin refs/tags/v0.6.0` | 0 | tag pushed |
| Verify tag peel | `git ls-remote origin refs/tags/v0.6.0^{}` | 0 | peel matches HEAD |

## Receipts

| Receipt | Path | SHA-256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json` | `1215b9cea612f5dcb56b6690d0b7cfa0028906f8fa6481178cd41c88c4e5e9bf` |
| release-receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json` | `6373a178670915fd49dd175692346fc470c75f33ca9c3c6c0f787f612aa05857` |

## Blockers

- None.

## Envelope

```yaml
status: success
route: local
change: m9-01-schema-versioning
cycle_id: m9-01-schema-versioning
main_sha: 25948147869307343a28e3896e8289c715efbc81
tag: v0.6.0
merge_receipt: cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json
release_receipt: cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json
runtime_status: RELEASED
next_phase: archive
lease_after_transition: absent
optional_distribution: not_requested
blockers: []
inventory:
  path: null
  sha256: null
  unavailable_reason: git-not-initialized
```

## Files Inventory

The `inventory.json` SDDK CLI artifact was not generated because the cycle has no runtime record in `sddk cycle status` (CLI returns `STORAGE_NOT_FOUND: cycle not found: m9-01-schema-versioning`). This is an ad-hoc cycle; the orchestrator owns the runtime transition path.

Cycle touches:

| Bucket | Added | Modified | Deleted |
|---|---|---|---|
| crates/ | 0 | 3 | 0 |
| docs/milestones/ | 1 | 0 | 0 |

Top paths: `crates/chronos-store/src/counterexample_storage.rs` (+307 lines, production + 5 tests), `crates/chronos-services/src/counterexample.rs` (+68 lines, production + 1 test), `crates/chronos-cli/src/replay.rs` (+8 lines, test fixtures), `docs/milestones/m9-01-schema-versioning-scoping.md` (+518 lines).

## Next

`sddk-archive` consumes `release-receipt` and emits `archive-manifest`. Archive performs durable spec/knowledge sync and cycle closure.
