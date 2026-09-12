# Archive Manifest — m9-09-vault-hygiene-active-disclosure-dedupe

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-09-vault-hygiene-active-disclosure-dedupe` |
| Change name | `m9-09-vault-hygiene-active-disclosure-dedupe` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `07e731d61424160c4f67d769db00a171837ba62c` |
| Head SHA | `07e731d61424160c4f67d769db00a171837ba62c` |
| Tag | `v0.7.7` (annotated, peel matches HEAD) |
| Base SHA | `c183ad1822423419ffb7d5f92a8d5ac46423d653` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-receipt.md
sha256: 9d25b4b223788af592afd1f773f3456e8e425527e17157a74cfcac811f0ab829
verified_at: 2026-09-12T07:15:18Z
remote_tag_peel: 07e731d61424160c4f67d769db00a171837ba62c
main_sha: 07e731d61424160c4f67d769db00a171837ba62c
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/merge-receipt.md
sha256: 30548a00744b0aa46ea5fedcd32361b039bcb89f66312965daec9c401e5f184f
git_effect: git.push → bc94a33... (docs head; tag peeled to 07e731d fix head per m9-04..m9-08 convention)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/verify-report.md
sha256: 52f2b3c1267d8497fdcbd24509fcb1ac40fdfbaca1abc2b54ca5922fd0259180
status: PASS
mode: doc-verify inline (B-direct)
scenarios: 1 (R1)
results: 1/1 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-report.md
sha256: 690fe470d857a832f326b4616e3ae8344486022453dbf1c5accefdc9150725d7
verdict: success
git_effects: all PASS
verification_summary: PASS, 1/1 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-09

**None.** This is a documentation-drift hygiene fix only. The closed-finding
ledger is unchanged.

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
| Cycle entry (m9-09) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-09-vault-hygiene-active-disclosure-dedupe/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` — duplicate `m9-01-R4` removed; metadata unchanged |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-09-vault-hygiene-active-disclosure-dedupe/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Vault-hygiene fix only:
- Removed 1 row from terms/index.md active-disclosures table

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-09-vault-hygiene-active-disclosure-dedupe/archive-manifest.md` | `7f8434f21ccb11978800f045aa34bed4ae74eee0574c35008eff3ea78b630e1e` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-09-vault-hygiene-active-disclosure-dedupe/change-entry.md` | `d0f3693a8550167aee79f594c0adae53f5e9b4e25ef7c6c191b582282c26794c` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/merge-receipt.md` | `30548a00744b0aa46ea5fedcd32361b039bcb89f66312965daec9c401e5f184f` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-receipt.md` | `9d25b4b223788af592afd1f773f3456e8e425527e17157a74cfcac811f0ab829` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/release-report.md` | `690fe470d857a832f326b4616e3ae8344486022453dbf1c5accefdc9150725d7` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-09-vault-hygiene-active-disclosure-dedupe/verify-report.md` | `52f2b3c1267d8497fdcbd24509fcb1ac40fdfbaca1abc2b54ca5922fd0259180` |
