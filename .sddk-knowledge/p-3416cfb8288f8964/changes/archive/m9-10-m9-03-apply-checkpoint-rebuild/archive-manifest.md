# Archive Manifest — m9-10-m9-03-apply-checkpoint-rebuild

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-10-m9-03-apply-checkpoint-rebuild` |
| Change name | `m9-10-m9-03-apply-checkpoint-rebuild` |
| Path | `B-rebuild` |
| Status | **CLOSED** |
| Published SHA | `69f200e2144bea2cd305c38903feb4814fb38806` |
| Head SHA | `69f200e2144bea2cd305c38903feb4814fb38806` |
| Tag | `v0.7.8` (annotated, peel matches HEAD) |
| Base SHA | `0e1474a61cf5777a6d0e22358aded55be222c784` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1: pass (pre-CC-cycle, no cross-checks applied)
## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-receipt.md
sha256: 8f873901e49f0bad076ea162dab12e76b2f70c4173d1d2d2f890196e2f3357e1
verified_at: 2026-09-12T07:22:36Z
remote_tag_peel: 69f200e2144bea2cd305c38903feb4814fb38806
main_sha: 69f200e2144bea2cd305c38903feb4814fb38806
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/merge-receipt.md
sha256: 624048400045e6b08a51ed57e5a470dbfdc3ae1f05cef032536401c206bc5ceb
git_effect: git.push → 6dc4609... (docs head; tag peeled to 69f200e fix head per m9-04..m9-09 convention)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/verify-report.md
sha256: 8600ff5ee1b6ae8212664f0d9f52c9319179b74f3181c24f2e306c07b7014675
status: PASS
mode: doc-verify inline (B-rebuild)
scenarios: 1 (R1)
results: 1/1 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-report.md
sha256: ec98a9d4d82d0bbb904679a485e229448ed9c02494c925c4deeedcc9c438aaaf
verdict: success
git_effects: all PASS
verification_summary: PASS, 1/1 scenarios COMPLIANT
```

### Rebuilt artifact

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json
note:  rebuilt from unmodified pre-existing inputs of the m9-03 cycle
       (merge-receipt.md, release-receipt.md, release-report.md, verify-report.md).
       No m9-03 input artifact was modified during rebuild.
```

## Vault sync

### Findings resolved by m9-10

**None.** This is a vault-artifact rebuild only. The closed-finding
ledger of m9-03 (4 findings) is now queryable programmatically through
the rebuilt apply-checkpoint.json (which is the post-m9-04 standard),
matching the format of m9-04..m9-09.

### Findings NOT resolved (m9+ backlog unchanged)

| ID | Cluster | Severity | Title | Reason |
|---|---|---|---|---|
| m9-02 R1–R8 | disclosure | — | various scope disclosures | by-design |
| m9-01 R1–R3 | disclosure | — | forward-compat + canonical-write disclosures | by-design |
| m9-04 R1–R6 | disclosure | — | layout + collision + scan disclosures | by-design |
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC | Splits require design pass + compat shim |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m8-06-R4 | disclosure | — | Cross-variant existence predicate shrinking | by-design |
| m8-04-R-hypothesis-fallback | disclosure | — | `property_target` lost in fallback reconstruction | by-design |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-10) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-10-m9-03-apply-checkpoint-rebuild/` |
| Rebuilt artifact | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (no new closures; metadata updated with `Last archive`) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-10-m9-03-apply-checkpoint-rebuild/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Vault-artifact rebuild only:
- Created 1 JSON artifact (cycle-artifacts/.../m9-03-.../apply-checkpoint.json)
- All values lifted verbatim from unmodified m9-03 inputs

## Artifact index

| Kind | Path | SHA-256 |
| Date | `2026-09-12` |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-10-m9-03-apply-checkpoint-rebuild/archive-manifest.md` | `8640857d6edc748f41aab52f083a0fddb464358f6cabacaabbd8738d6db3e1da` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-10-m9-03-apply-checkpoint-rebuild/change-entry.md` | `50f929131710ddc94657626a34f865dabdc6b1a4d254abbc56b3da7cd271756f` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/merge-receipt.md` | `5399e13a34e03e0aa81f5f1048e016a6546a4a2335c4ca18492d10d974e39439` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-receipt.md` | `7e59b23e076da634c477d8558acff2a387f789bf49911940d908ddff3f733c89` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/release-report.md` | `ea50df07a81828435b47d4beaf817405b0cb4d53c8322f1691976102a90d7b8b` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-10-m9-03-apply-checkpoint-rebuild/verify-report.md` | `55ca517996f0267e73097c22e1f8fcf123bb1df212c2051b80a0cdad7042703e` |
| rebuilt (m9-03) apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/apply-checkpoint.json` | `e3c8a608c38fbde1858e62d52aa2708614385149c52f42652db3114a94658983` |
