# Archive Manifest — m9-05-side-table-overeng-cleanup

## Identidad del ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-05-side-table-overeng-cleanup` |
| Change name | `m9-05-side-table-overeng-cleanup` |
| Path | `B-direct` |
| Status | **CLOSED** |
| Published SHA | `07d01d5869ff6e3ffed29315b761e6e476f3b70d` |
| Head SHA | `07d01d5869ff6e3ffed29315b761e6e476f3b70d` |
| Tag | `v0.7.3` (annotated, peel matches HEAD) |
| Base SHA | `c9e541750e39458b2b28c5d19ad0ca11d0e73d5b` |
| Delivery kind | local (no CLI storage; orchestrator owns transition) |

## Evidence bindings

### Release receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-receipt.md
sha256: d3bd177e6f3af104042e741a7e96451308ee22534a6014a926cb375f96f5c935
verified_at: 2026-09-12T08:40:00Z
remote_tag_peel: 07d01d5869ff6e3ffed29315b761e6e476f3b70d
main_sha: 07d01d5869ff6e3ffed29315b761e6e476f3b70d
peel_match: true
pipeline_state: { release: complete, archive: closed }
```

### Merge receipt

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/merge-receipt.md
sha256: 7307975defbf9613259563fb3766f15758336a324dea8cad742f57d68eee0db9
git_effect: git.push → 07d01d5... (direct push to origin/main)
sha_match: true
```

### Verify evidence

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md
sha256: a5ce50aa2a4f84e0b2b47f9035525d5013bbeabb6f988a01341c6e08f0598103
status: PASS
mode: light-verify inline (B-direct)
scenarios: 4 (R1, R2, R3, R4 — one per finding)
results: 4/4 COMPLIANT
```

### Release report

```
path:  cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-report.md
sha256: b656a98a7765157818b90c337b1f0713ad3d68adbffaafab790840d72aae0527
verdict: success
git_effects: all PASS
verification_summary: PASS, 4/4 scenarios COMPLIANT
```

## Vault sync

### Findings resolved by m9-05

Four m9-04 debt findings are closed by this cycle:

| ID | Cluster | Severity | Priority | Título | Evidence |
|---|---|---|---|---|---|
| overeng-001-v3-chunk-decode-dup | overeng | MEDIUM | P2 | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` | `crates/chronos-store/src/counterexample_storage.rs` — `decode_chunk_payload` helper extracted; both call sites now route through it |
| overeng-002-v3-range-scan-verify-dup | overeng | LOW | P3 | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` | `crates/chronos-store/src/counterexample_storage.rs` — `collect_v3_keys_for_bundle` helper extracted; `save_bundle_record_and_events` D6 cleanup block now routes through it |
| overeng-003-events-count-none-branch | overeng | LOW | P3 | `collect_bundle_chunks(events_count: Option<u64>)` and `get_bundle_events_count` Option wrapper; no caller exercises None post-remediation | `crates/chronos-store/src/counterexample_storage.rs` — `Option<u64>` wrapper removed; `events_count: u64` plumbed directly with D7 short-circuit inlined |
| cc-003-wrong-direction-visibility | coupling | MEDIUM | P2 | `storage.rs::db()` widened `pub(crate)` → `pub`; `COUNTEREXAMPLE_BUNDLES` and `COUNTEREXAMPLE_BUNDLE_EVENTS` widened to `pub const` | `crates/chronos-store/src/storage.rs` — `db()` narrowed back to `pub(crate)`; table constants reverted to module-private; three `#[doc(hidden)] pub` chokepoints added on `SessionStore` (`insert_v2_chunk_for_test`, `count_v3_chunks_for_test`, `insert_bundle_record_for_test`); `crates/chronos-cli/tests/replay_integration.rs` rewritten to use chokepoints |

**Closed by:** `m9-05-side-table-overeng-cleanup` · SHA `07d01d5...` · tag `v0.7.3`

### Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Título | Reason |
|---|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.5K LoC; 5 distinct concerns (keys/records/persistence/schema-version/v2-legacy) | Splits require a design pass + backward-compat shim; not B-direct |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) | Concurrency design needed |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m9-01 R1-R4 | various | — | See `changes/m9-01-schema-versioning/change-entry.md` | Deferred from m9-01 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | property_target lost in fallback reconstruction | Deferred from m8-04 |

## Knowledge nodes updated

| Node kind | Path |
|---|---|
| Cycle entry (m9-05) | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-05-side-table-overeng-cleanup/` |
| Terms (m9+) | `.sddk-knowledge/p-3416cfb8288f8964/terms/` (4 findings closed, 2 backlog) |
| Cycles index | `.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` |
| Archive manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-05-side-table-overeng-cleanup/archive-manifest.md` |

## Ledger state (pre-archive)

```
note: CLI ledger not available (sddk not in PATH). Ad-hoc cycle, no CLI storage.
      Runtime status RELEASED recorded in release receipt.
```

## Specs synced

No new specs introduced. Bounded debt-cleanup:
- Private helpers extracted (no public surface change for R1, R2)
- `Option<u64>` wrapper removed (signature simplification; behavior unchanged for the exercised branch)
- Cross-crate test access narrowed to typed chokepoints (replaces raw `db()` + `TableDefinition` access)

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| archive-manifest (this file) | `.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-05-side-table-overeng-cleanup/archive-manifest.md` | `47bbe915e680822169f8e7877029ec5373e3170a4768829414205a2df4c07940` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-05-side-table-overeng-cleanup/change-entry.md` | `7125b84c756ef88283bc673c7acff3ab19d24f729370be7c5a39420ed8144ee8` |
| scoping | `docs/milestones/m9-05-side-table-overeng-cleanup-scoping.md` | `5760d827563e2d3ad8e7521285ec56fd17ea2adee367f30820aae0e01964b141` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/merge-receipt.md` | `7307975defbf9613259563fb3766f15758336a324dea8cad742f57d68eee0db9` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-receipt.md` | `d3bd177e6f3af104042e741a7e96451308ee22534a6014a926cb375f96f5c935` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/release-report.md` | `b656a98a7765157818b90c337b1f0713ad3d68adbffaafab790840d72aae0527` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup/verify-report.md` | `a5ce50aa2a4f84e0b2b47f9035525d5013bbeabb6f988a01341c6e08f0598103` |