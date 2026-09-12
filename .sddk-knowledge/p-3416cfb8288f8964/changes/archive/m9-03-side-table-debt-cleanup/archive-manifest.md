# Archive Manifest — m9-03-side-table-debt-cleanup

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle | `m9-03-side-table-debt-cleanup` |
| Change name | `m9-03-side-table-debt-cleanup` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `2c98ce9a1df65d44ae865376fee46eb0d95ac425` |
| Head SHA | `2c98ce9a1df65d44ae865376fee46eb0d95ac425` |
| Tag | `v0.7.1` (annotated, peel matches HEAD) |
| Base SHA | `6f375fd96dbc0c03fe36b473d306f1e54d078b08` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/release-receipt.md
sha256: e5c3b40cc588cc639c325763fc88e01e4a398756324b1b8b58b7ce59e09a45ce
verified_at: 2026-09-11T21:13:10Z
remote_tag_peel: 2c98ce9a1df65d44ae865376fee46eb0d95ac425
main_sha: 2c98ce9a1df65d44ae865376fee46eb0d95ac425
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/merge-receipt.md
sha256: 6d2cc59dfc7dada71dbc9a77d449b3b564794fe743b5b7825cc626ffbc31109e
git_effect: git.push → 2c98ce9... (direct push to origin/main)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/verify-report.md
sha256: 3405fa5f96e31b00e0ea7b23143667182625e88b1d7851bdbdee7404e8f82414
status: PASS
mode: light-verify inline (B-direct)
scenarios: 4 (one per finding)
results: 4/4 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/release-report.md
sha256: 0d2e6ef7b5ac310bd640f17f3e263426e019ed38351571fae4e61038bd06d2cb
verdict: success
git_effects: all PASS
verification_summary: PASS, 4/4 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-03

Four m9-02 debt findings are closed by this cycle:

| ID | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|
| FIND-M9-02-DV-API-01 | api | LOW | P3 | `save_counterexample_bundle_events` dead API: no caller uses it standalone | `crates/chronos-store/src/counterexample_storage.rs` — deleted public method; grep across `crates/` + `chronos-sandbox/` returned zero references |
| FIND-M9-02-DV-DOC-01 | doc | LOW | P3 | `events_count` doc drift: fallback branch not documented at call site | `crates/chronos-store/src/counterexample_storage.rs` docstring updated: "Returns 0 when the side table does not exist or the bundle has no chunks." |
| FIND-M9-02-DV-OE-01 | overeng | MEDIUM | P2 | Side-table chunk-iteration skeleton duplicated in 3 places | `crates/chronos-store/src/counterexample_storage.rs::collect_bundle_chunks` — 3 inline `open_table + iter + decode` loops collapsed into one private helper; `load_counterexample_bundle_events` and `count_counterexample_bundle_events` now call the helper |
| FIND-M9-02-DV-COUP-01 | coupling | MEDIUM | P2 | Fallback outside D5 chokepoint: wrong module boundary | `bundle_events_count_or_legacy` chokepoint (D5) added to store; both `chronos-services` call sites (`counterexample.rs:376` and `:537`) now route through it; fallback logic co-located with canonical writer boundary |

**Closed by:** `m9-03-side-table-debt-cleanup` · SHA `2c98ce9...` · tag `v0.7.1`

### Findings NOT resolved (m9+ backlog)

| ID | Cluster | Severity | Priority | Título | Reason |
|---|---|---|---|---|---|
| FIND-M9-02-DV-PERF-01 | perf | MEDIUM | P2 | Side-table key layout forces full-scan for single-bundle reads | Out of scope for B-direct; requires separate design |
| m9-02 R1–R8 | various | — | backlog | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 scoping |
| m9-01 R1–R4 | various | — | backlog | See `changes/m9-01-schema-versioning/change-entry.md` | Deferred from m9-01 scoping |
| m8-06 R4 | various | — | backlog | Cross-variant existence predicate shrinking | Deferred from m8-06 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-03) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-03-side-table-debt-cleanup/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (4 findings closed) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-03-side-table-debt-cleanup/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Bounded debt-cleanup:
- Dead public API removed
- Docstring drift corrected
- Private helper extracted (no public surface change)
- Internal chokepoint added (same redb semantics)

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-03-side-table-debt-cleanup/archive-manifest.md` | pending (computed after write) |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-03-side-table-debt-cleanup/change-entry.md` | `99b97f7a731c323fd0997f23f6078efeb2b912bdcdd20417877d9169fafb8b87` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/merge-receipt.md` | `6d2cc59dfc7dada71dbc9a77d449b3b564794fe743b5b7825cc626ffbc31109e` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/release-receipt.md` | `e5c3b40cc588cc639c325763fc88e01e4a398756324b1b8b58b7ce59e09a45ce` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/release-report.md` | `0d2e6ef7b5ac310bd640f17f3e263426e019ed38351571fae4e61038bd06d2cb` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-03-side-table-debt-cleanup/verify-report.md` | `3405fa5f96e31b00e0ea7b23143667182625e88b1d7851bdbdee7404e8f82414` |
