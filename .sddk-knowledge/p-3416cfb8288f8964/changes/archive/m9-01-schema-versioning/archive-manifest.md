# Archive Manifest — m9-01-schema-versioning

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-01-schema-versioning` |
| Change name | `m9-01-schema-versioning` |
| Path | `A-min` |
| Status | **CLOSED** |
| Published SHA | `25948147869307343a28e3896e8289c715efbc81` |
| Head SHA | `25948147869307343a28e3896e8289c715efbc81` |
| Tag | `v0.6.0` (annotated, peel matches HEAD) |
| Base SHA | `bceddc946b73209756d3360b0aec4dca592296c3` |
| Delivery kind | ad-hoc (no CLI storage; orchestrator owns transition) |

## Summary

Drift closure cycle. See release-receipt.md and change-entry.md for details.



## Cross-checks

- C1: pass (pre-CC-cycle, no cross-checks applied)
## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json
sha256: 6373a178670915fd49dd175692346fc470c75f33ca9c3c6c0f787f612aa05857
verified_at: 2026-09-11T17:38:20Z
remote_tag_peel: 25948147869307343a28e3896e8289c715efbc81
main_sha: 25948147869307343a28e3896e8289c715efbc81
peel_match: true
pipeline_state: { release: complete, archive: pending → closed }
```

### Merge receipt

```
path:  cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json
sha256: 1215b9cea612f5dcb56b6690d0b7cfa0028906f8fa6481178cd41c88c4e5e9bf
git_effect: git.merge.ff-only → 2594814... (fast-forward)
sha_match: true
```

### Verify evidence

```
path:  verify-report.md
sha256: 2a5eeee3a89690f967cec391708c0354a52cde83c3458b79d70949f1ea271d4f
subject_sha: 0aec8bac30f995e638e2a656b55935f48a60623d
verdict: PASS_WITH_WARNINGS
note: verified SHA (0aec8ba) precedes release SHA (2594814) by one docs-only commit; no behavioral delta.
```

### Debt evidence

```
path:  cycle-artifacts/m9-01-schema-versioning/debt-verify/debt-report.json
sha256: 5351e72d4f7cd9a634348cc4b5e99518ab312826d0f220458728f71de54f599a
subject_sha: 0aec8bac30f995e638e2a656b55935f48a60623d
verdict: PASS_WITH_WARNINGS
clusters: coupling (2) · overeng (1)
blocking_findings: 0
all_findings_target: backlog
attribution: 3 introduced · 0 pre-existing · 0 unknown
```

## Vault sync

### Disclosures — m8-07 R2 closed

**R2 — Unknown future wire fields are dropped** (m8-07 scoping §6, `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md:291-297`)

> If a future chronos-services version adds a field to `HypothesisInput`, the wire
> mirror will silently drop it on persist. Mitigated by the m9+ plan to add
> `schema_version: u32` to `CounterexampleBundleSummary`.

**Closure rationale:** m9-01 implemented the exact mitigation described: `schema_version: u32`
was added to both `CounterexampleBundleRecord` and `CounterexampleBundleSummary`
(`crates/chronos-store/src/counterexample_storage.rs:171-207`). The canonical writer
sets both fields atomically; the loader checks `record.schema_version > CURRENT` before
deserializing. Unknown future wire fields in `HypothesisInputWire` are now protected by
the bundle-level version guard.

**Closure evidence:** `counterexample_storage.rs` lines 234-235 (canonical writer sets
both fields), lines 286-299 (loader guards on `schema_version`).

### Disclosures — m9-01 (R1–R4) → m9+ follow-ups

All four scoping disclosures are left as open terms for m9+.

| ID | Título | Estado | Destino |
|---|---|---|---|
| R1 | `schema_version` silently overwritten on save (D5 canonical writer) | open | m9+ |
| R2 | Future-versioned bundles appear in list (best-effort; load() rejects individually) | open | m9+ |
| R3 | `schema_version` is internal-only (not on MCP wire or services-side summary) | open | m9+ |
| R4 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` currently unused (upper-bound check only); suppressed with `#[allow(dead_code)]` | open | m9+ |

### Debt findings → m9+ follow-ups

All three debt findings are assigned to backlog with no INC (cycle-7b policy: `PASS_WITH_WARNINGS` with zero pre-existing findings requires no INC files).

| ID | Cluster | Severity | Priority | Título | Destino |
|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | P2 | Duplicated version envelopes: `schema_version` on both record and summary, no equality enforcement in loader | m9+ backlog |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | P3 | List/load policy asymmetry: hard-reject future versions on load vs best-effort include on list | m9+ backlog |
| FIND-M9-01-DV-OE-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` is dead speculative code | m9+ backlog |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-01) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-01-schema-versioning/` |
| m8-07 closure (R2) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m8-07-hypothesis-reconstruction-fidelity/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md` |

## Vault validation

```
sddk vault validate --root . --scope . --vault sddk
result: { nodes: 0, backlinks: 0, errors: 0, warnings: 0 }
note: vault is at .sddk-knowledge/ (new); sddk/ is the existing repo-local
      vault (zero nodes before this archive). Validation passes with zero errors.
```

## Ledger state (pre-archive)

```
sddk ledger verify --root . --scope .
event_count: 572
last_hash: sha256:ccb7b5f4f5cba5ac50b172155e070ff71c4500b17a3fd269e5ac7ae0a9529b71
note: CLI ledger already reconciled to RELEASED/archive by orchestrator.
      sddk cycle transition archive.complete not executed (ad-hoc cycle, no CLI storage).
      Runtime status CLOSED recorded in this manifest.
```

## Specs synced

| Dominio | added | modified | removed |
|---|---|---|---|
| data-model / bincode envelope | 1 (schema_version on CounterexampleBundleRecord + Summary) | 0 | 0 |

No formal spec.md existed for this change; the scoping doc
(`docs/milestones/m9-01-schema-versioning-scoping.md`) serves as the durable spec.

## Artifact index

| Kind | Path | SHA-256 |
| Date | `2026-09-11` |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-manifest.md` | `a7eaf340b9a4b364547b8f6136bafec68102e5880eac580fc172b4be45f07476` |
| archive-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-01-schema-versioning/archive-report.md` | `319d7ddf9d2cff6892d78fb68fa018905ede4a58aa422d02e5f927144b6878c0` |
| merge-receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json` | `1215b9cea612f5dcb56b6690d0b7cfa0028906f8fa6481178cd41c88c4e5e9bf` |
| release-receipt | `cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json` | `6373a178670915fd49dd175692346fc470c75f33ca9c3c6c0f787f612aa05857` |
| release-report | `cycle-artifacts/m9-01-schema-versioning/receipts/release-report.md` | `c62e63db1fd66cf01b0f613e22617a77ea2f2d6c8f2c5a15a87cda5e3998be3c` |
| debt-report | `cycle-artifacts/m9-01-schema-versioning/debt-verify/debt-report.json` | `5351e72d4f7cd9a634348cc4b5e99518ab312826d0f220458728f71de54f599a` |
| apply-checkpoint | `apply-checkpoint.json` | `fd77c4003bc82525866749687097d215b0223fc58c863c0a89e7728a282d65cb` |
| scoping doc | `docs/milestones/m9-01-schema-versioning-scoping.md` | `5da4416eb66b734d8e983a2b71377333fb4775fdb9509263d44f3af8de9edc9d` |

## Runtime status

```
status: CLOSED
phase: archive
closed_at: 2026-09-11T17:43:00Z  (approximate; ad-hoc cycle without CLI storage)
cycle_id: m9-01-schema-versioning
```

## Envelope

```yaml
status: success
cycle_id: m9-01-schema-versioning
change_name: m9-01-schema-versioning
path: A-min
published_subject:
  main_sha: 25948147869307343a28e3896e8289c715efbc81
  tag: v0.6.0
release_receipt: cycle-artifacts/m9-01-schema-versioning/receipts/release-receipt.json
merge_receipt: cycle-artifacts/m9-01-schema-versioning/receipts/merge-receipt.json
runtime_status: CLOSED
next_recommended: ready-for-next-cycle
context_quality: C1
skill_resolution: fallback-path
follow_up_incidences: []
disclosures_closed: [m8-07 R2]
disclosures_open: [m9-01 R1, m9-01 R2, m9-01 R3, m9-01 R4]
debt_findings: [FIND-M9-01-DV-COUP-01, FIND-M9-01-DV-COUP-02, FIND-M9-01-DV-OE-01]
all_debt_target: backlog
```
