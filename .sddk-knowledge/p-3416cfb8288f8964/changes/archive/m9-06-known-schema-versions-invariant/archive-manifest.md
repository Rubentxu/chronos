# Archive Manifest — m9-06-known-schema-versions-invariant

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-06-known-schema-versions-invariant` |
| Change name | `m9-06-known-schema-versions-invariant` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `3383905d93ac66b6e90b6de8cea760c1ad4e2c99` |
| Head SHA | `3383905d93ac66b6e90b6de8cea760c1ad4e2c99` |
| Tag | `v0.7.4` (annotated, peel matches HEAD) |
| Base SHA | `9d2c676bfc6331f730e7db9eac3ec9c057db1de4` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1: pass (pre-CC-cycle, no cross-checks applied)
## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-receipt.md
sha256: 6e625ca1ebdca4a237478e98cbb57f56382b0e867fb212a6fdddc1c1b0f6e229
verified_at: 2026-09-12T08:43:51Z
remote_tag_peel: 3383905d93ac66b6e90b6de8cea760c1ad4e2c99
main_sha: 3383905d93ac66b6e90b6de8cea760c1ad4e2c99
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/merge-receipt.md
sha256: fcffbd44b1e163789c5781b80329f0b32554225d4991726fa62549e05138525b
git_effect: git.push → 3383905... (direct push to origin/main)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md
sha256: 5bbc23c901ad1ea8a132f8a37176a19a55d3e696dbed07b088838c93f96bc0ee
status: PASS
mode: light-verify inline (B-direct)
scenarios: 2 (R1, R2)
results: 2/2 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-report.md
sha256: e1927c4ae60d0c3d23f4e1f89ca0fe1f3eaa75b4981b8c853295ec602b3134fb
verdict: success
git_effects: all PASS
verification_summary: PASS, 2/2 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-06

Two findings closed by this cycle:

| ID | Cycle | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|---|
| m9-01-R4 | m9-01 | disclosure | — | backlog | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | `crates/chronos-store/src/counterexample_storage.rs` — `#[allow(dead_code)]` removed; compile-time invariant forces the constant to be evaluated in production builds |
| FIND-M9-01-DV-OE-01 | m9-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | Same: the `const _: () = { ... }` block references the constant at module scope |

**Closed by:** `m9-06-known-schema-versions-invariant` · SHA `3383905...` · tag `v0.7.4`

### Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Título | Reason |
|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | Duplicated `schema_version` on record + summary | Requires design decision (which is canonical) |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | List/load policy asymmetry | Requires list-side change |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.5K LoC | Splits require design pass + compat shim |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | property_target lost in fallback reconstruction | Deferred from m8-04 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-06) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-06-known-schema-versions-invariant/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (2 findings closed, 1 still active from m9-01) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-06-known-schema-versions-invariant/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Trivial B-direct cleanup:
- Removed `#[allow(dead_code)]` attribute
- Added compile-time invariant referencing the constant
- Added a lib test pinning the invariant

## Artifact index

| Kind | Path | SHA-256 |
| Date | `2026-09-12` |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-06-known-schema-versions-invariant/archive-manifest.md` | `a4cf2f5bde00b4492facdb1b37499e7239a8cea6c9b1b548e36b6b52af392708` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-06-known-schema-versions-invariant/change-entry.md` | `d6be53c35c24fbe1c86f957add8a5da5da0831a3709ae07934502452f4f37c51` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/merge-receipt.md` | `fcffbd44b1e163789c5781b80329f0b32554225d4991726fa62549e05138525b` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-receipt.md` | `6e625ca1ebdca4a237478e98cbb57f56382b0e867fb212a6fdddc1c1b0f6e229` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/release-report.md` | `e1927c4ae60d0c3d23f4e1f89ca0fe1f3eaa75b4981b8c853295ec602b3134fb` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-06-known-schema-versions-invariant/verify-report.md` | `5bbc23c901ad1ea8a132f8a37176a19a55d3e696dbed07b088838c93f96bc0ee` |