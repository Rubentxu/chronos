# Archive Manifest — m9-02-events-side-table

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-02-events-side-table` |
| Change name | `m9-02-events-side-table` |
| Path | `A-lite` |
| Status | **CLOSED** |
| Published SHA | `1a8d104ba2da883b40bd424cdc079b344f7ed63c` |
| Tag | `v0.7.0` (annotated, peel matches HEAD) |
| Base SHA | `48a9cff54ec165987063705d5d4d3af453fcb6b3` |
| Delivery kind | ad-hoc (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/m9-02-events-side-table/receipts/release-receipt.md
sha256: 9e4027f3666d616eb5229726e6a12a06f0a5108a2c1e2db3c7f090f363c42f18
verified_at: 2026-09-11T20:34:51Z
remote_tag_peel: 1a8d104ba2da883b40bd424cdc079b344f7ed63c
main_sha: 1a8d104ba2da883b40bd424cdc079b344f7ed63c
peel_match: true
pipeline_state: { release: complete, archive: pending → closed }
```

### Merge receipt

```
path:  cycle-artifacts/m9-02-events-side-table/receipts/merge-receipt.md
sha256: 031f5d3a3960716ed2f2ed3fbcc2164121cf700c3df2fb3590215b9c2d06d6a6
git_effect: git.merge.ff-only → 1a8d104... (fast-forward)
sha_match: true
```

### Verify evidence

```
status: MISSING — verify-report.md not present in cycle-artifacts/m9-02-events-side-table/
expected_path: cycle-artifacts/m9-02-events-side-table/verify-report.md
release_report_reference: "cycle-artifacts/p-3416cfb8288f8964/m9-02-events-side-table/verify-report.md"
note: The release report (§Release Evidence) cites a verify-report at a path that does
      not exist in the actual cycle-artifacts directory. Gap in cycle artifact chain.
      No verify-report.md was committed for this cycle.
```

### Debt evidence

```
status: MISSING — debt-report.json not present in cycle-artifacts/m9-02-events-side-table/
expected_path: cycle-artifacts/m9-02-events-side-table/debt-report.json
release_report_reference: "cycle-artifacts/p-3416cfb8288f8964/m9-02-events-side-table/debt-report.json"
release_report_verdict_claim: "PASS_WITH_WARNINGS (7 backlog findings)"
note: The release report (§Release Evidence) cites a debt-report at a path that does
      not exist in the actual cycle-artifacts directory. Gap in cycle artifact chain.
      No debt-report.json was committed for this cycle.
      7 backlog findings are recorded from the task contract and scoping doc disclosures
      (m9-02 R1–R8) plus implementation-specific debt observations. No formal debt-report
      artifact exists to verify SHA binding or outer-envelope HMAC.
```

## Vault sync

### Disclosures — m8-04 R4 closed

**R4 — bundle-as-blob → side table: events still ride inside bundle blob**
(m8-04 scoping §3 B-decisions, `docs/milestones/m8-04-chronos-cli-engine-events-saved-rework-scoping.md:135`)

> "bundle-as-blob storage — deferred to m9+. Acceptable: real events are small in
> the smoke fixtures; the inefficiency only matters for 10k+ events scenes."

**Closure rationale:** m9-02 created the `counterexample_bundle_events` redb side
table, chunked by 256 events per row with composite key `(bundle_id, chunk_index)`.
All post-m9-02 saves write events to the side table; the blob carries
`events: vec![]`. The `bundle_events_or_legacy` chokepoint (D5) routes legacy
(pre-m9-02) blob-embedded events vs. post-m9-02 side-table events. The m9-02
scoping doc §9 explicitly declares R4 closed.

**Closure evidence:** `crates/chronos-store/src/counterexample_storage.rs`:
- `COUNTEREXAMPLE_BUNDLE_EVENTS` table definition (new)
- `save_bundle_record_and_events` — atomic write of record + chunks
- `bundle_events_or_legacy` — D5 chokepoint, legacy blob vs. side-table routing
- `load_counterexample_bundle_events` — side-table concat (chunk_index order)
- `save_counterexample_bundle` — sets `events: vec![]`, routes to atomic helper

**Closed by:** `m9-02-events-side-table` · SHA `1a8d104...` · tag `v0.7.0`

### Disclosures — m9-02 R1–R8 → m9+ follow-ups

All eight scoping disclosures are left as open terms for m9+.

| ID | Título | Estado | Destino |
|---|---|---|---|
| m9-02 R1 | `schema_version` bumped to 2; bundles with >2 hard-rejected | disclosed | m9+ |
| m9-02 R2 | Pre-m9-02 bundles get `events_count: 0`; fallback to blob for count | disclosed | m9+ |
| m9-02 R3 | Public `save_counterexample_bundle_events` not atomic w.r.t. record | disclosed | m9+ |
| m9-02 R4 | `counterexample_bundle_events` MCP tool is m9+ scope | disclosed | m9+ |
| m9-02 R5 | No chunk compression (lz4/zstd) | disclosed | m9+ |
| m9-02 R6 | `list_counterexample_bundles` still deserializes full blob per row | disclosed | m9+ |
| m9-02 R7 | Re-save overwrites all prior chunks (no append semantics) | disclosed | m9+ |
| m9-02 R8 | Per-chunk load not publicly exposed | disclosed | m9+ |

### Debt findings → m9+ follow-ups

Five implementation-specific debt findings from m9-02 are assigned to backlog.

| ID | Cluster | Severity | Priority | Título | Destino |
|---|---|---|---|---|---|
| FIND-M9-02-DV-API-01 | api | LOW | P3 | `save_counterexample_bundle_events` dead API: no caller uses it standalone | m9+ backlog |
| FIND-M9-02-DV-DOC-01 | doc | LOW | P3 | `events_count` doc drift: fallback branch not documented at call site | m9+ backlog |
| FIND-M9-02-DV-OE-01 | overeng | MEDIUM | P2 | Side-table chunk-iteration skeleton duplicated in 3 places | m9+ backlog |
| FIND-M9-02-DV-COUP-01 | coupling | MEDIUM | P2 | Fallback outside D5 chokepoint: wrong module boundary | m9+ backlog |
| FIND-M9-02-DV-PERF-01 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads | m9+ backlog |

**Inherited from scoping disclosures (no separate INC):**

| ID | Cluster | Severity | Priority | Título | Destino |
|---|---|---|---|---|---|
| FIND-M9-01-DV-COUP-01 | coupling | MEDIUM | P2 | Duplicated version envelopes: `schema_version` on record + summary | m9+ backlog |
| FIND-M9-01-DV-COUP-02 | coupling | LOW | P3 | List/load policy asymmetry | m9+ backlog |
| FIND-M9-01-DV-OE-01 | overeng | LOW | P3 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead code | m9+ backlog |

No INC files created (cycle-7b policy: `PASS_WITH_WARNINGS`; all findings target backlog).

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-02) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-02-events-side-table/` |
| m8-04 R4 closure | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (m8-04 R4 moved to Terminated) |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (R1–R8 added; R4 closed) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` |

## Vault validation

```
result: nodes: 1 new (m9-02 change entry), 1 closure (m8-04 R4), 1 terms update
note: vault is at .sddk-knowledge/ (p-3416cfb8288f8964/); sddk/ is the repo-local
      vault (zero nodes). Validation passes with zero errors.
```

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status CLOSED recorded in this manifest.
```

## Specs synced

| Dominio | added | modified | removed |
|---|---|---|---|
| data-model / bincode envelope | 1 (events moved to side table, events_count on summary, schema v2) | 0 | 0 |

No formal spec.md existed for this change; the scoping doc
(`docs/milestones/m9-02-events-side-table-scoping.md`) and design doc
(`docs/milestones/m9-02-events-side-table-design.md`) serve as the durable specs.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-manifest.md` | `651593249af2b12a94b477f4c4918f5e7637685580619cd85ee0872e1fceb39b` |
| archive-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-02-events-side-table/archive-report.md` | `e59337788241a4ea96cef37c58e81563535f61caf74c8437f3078bd34aca1f89` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-02-events-side-table/change-entry.md` | `73f66009ac7ee76f7eac519bbaf672618058a03964068ad627b8d880f40c2277` |
| merge-receipt | `cycle-artifacts/m9-02-events-side-table/receipts/merge-receipt.md` | `031f5d3a3960716ed2f2ed3fbcc2164121cf700c3df2fb3590215b9c2d06d6a6` |
| release-receipt | `cycle-artifacts/m9-02-events-side-table/receipts/release-receipt.md` | `9e4027f3666d616eb5229726e6a12a06f0a5108a2c1e2db3c7f090f363c42f18` |
| release-report | `cycle-artifacts/m9-02-events-side-table/receipts/release-report.md` | `6d72bd949bab959236bb3ede04d38f6c9b7d2b300dae55240ab8e522e7a74422` |
| scoping doc | `docs/milestones/m9-02-events-side-table-scoping.md` | `001c7969d3f416432268cab8a0880460f87b23f6ee20800d9dafb2321afd0653` |
| design doc | `docs/milestones/m9-02-events-side-table-design.md` | `6c13ff80e0d130463df66071653ae0dfd7ab9d302a36b7a2d35394b52aeb57c9` |
| apply-checkpoint | `apply-checkpoint.json` | `4a4b367de0ba46730549e60343ea6b629612eda11cf23cb5c5addaa7f1a67dd8` |
| terms index | `.sddk-knowledge/p-3416cfb8288f8964/terms/index.md` | `5eb3b6bc6c93b4e2e84ddcb7fa17057ee6a0bc110c2a339112a71114435f98cd` |
| cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` | `2c9bc11bd421f75ea83a590d289957776340a687c477fbca7943dfcd9569eff5` |

## Runtime status

```
status: CLOSED
phase: archive
closed_at: 2026-09-11T22:38:00Z  (approximate; ad-hoc cycle without CLI storage)
cycle_id: m9-02-events-side-table
```

## Envelope

```yaml
status: success
cycle_id: m9-02-events-side-table
change_name: m9-02-events-side-table
path: A-lite
published_subject:
  main_sha: 1a8d104ba2da883b40bd424cdc079b344f7ed63c
  tag: v0.7.0
release_receipt: cycle-artifacts/m9-02-events-side-table/receipts/release-receipt.md
merge_receipt: cycle-artifacts/m9-02-events-side-table/receipts/merge-receipt.md
runtime_status: CLOSED
next_recommended: ready-for-next-cycle
context_quality: C1
skill_resolution: fallback-path
follow_up_incidences: []
disclosures_closed: [m8-04 R4]
disclosures_open: [m9-02 R1, m9-02 R2, m9-02 R3, m9-02 R4, m9-02 R5, m9-02 R6, m9-02 R7, m9-02 R8, m8-06 R4]
debt_findings: [FIND-M9-02-DV-API-01, FIND-M9-02-DV-DOC-01, FIND-M9-02-DV-OE-01, FIND-M9-02-DV-COUP-01, FIND-M9-02-DV-PERF-01, FIND-M9-01-DV-COUP-01, FIND-M9-01-DV-COUP-02, FIND-M9-01-DV-OE-01]
all_debt_target: backlog
artifact_gaps:
  - kind: verify-report
    expected: cycle-artifacts/m9-02-events-side-table/verify-report.md
    note: Missing from cycle-artifacts. Release report cites incorrect path (p-3416cfb8288f8964/ prefix).
  - kind: debt-report
    expected: cycle-artifacts/m9-02-events-side-table/debt-report.json
    note: Missing from cycle-artifacts. Release report cites incorrect path. Debt findings
          recorded from task contract and scoping disclosures; no formal SHA/HMAC binding.
```
