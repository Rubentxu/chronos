# Terms Index — m9+ follow-ups

Terms tracked from released cycles awaiting resolution in milestone m9 or later.

## Active terms

### Disclosures (m9-02 scoping R1–R8)

| ID | Cycle | Título | Severity | Priority | Owner | Destino |
|---|---|---|---|---|---|---|
| m9-02-R1 | m9-02 | `schema_version` bumped to 2; bundles with >2 hard-rejected by loader | — | backlog | unassigned | m9+ |
| m9-02-R2 | m9-02 | Pre-m9-02 bundles get `events_count: 0` via serde default; fallback to `record.events.len()` for count | — | backlog | unassigned | m9+ |
| m9-02-R3 | m9-02 | Public `save_counterexample_bundle_events` is not atomic w.r.t. the record (only the internal wrapper is) | — | backlog | unassigned | m9+ |
| m9-02-R4 | m9-02 | `counterexample_bundle_events` MCP tool deferred to m9+ (m9-02 ships storage primitive only) | — | backlog | unassigned | m9+ |
| m9-02-R5 | m9-02 | No chunk compression (lz4/zstd) — chunk size 256 is acceptable for typical bundles | — | backlog | unassigned | m9+ |
| m9-02-R6 | m9-02 | `list_counterexample_bundles` still deserializes full blob per row (`events: vec![]` is the optimisation) | — | backlog | unassigned | m9+ |
| m9-02-R7 | m9-02 | Re-save overwrites all prior chunks (no append semantics) | — | backlog | unassigned | m9+ |
| m9-02-R8 | m9-02 | Per-chunk load not publicly exposed (concat-then-paginate is fast enough for realistic bundle sizes) | — | backlog | unassigned | m9+ |

### Disclosures (m9-01 scoping R1–R4)

| ID | Cycle | Título | Severity | Priority | Owner | Destino |
|---|---|---|---|---|---|---|
| m9-01-R1 | m9-01 | `schema_version` silently overwritten on save (D5 canonical writer) | — | backlog | unassigned | m9+ |
| m9-01-R2 | m9-01 | Future-versioned bundles appear in list (best-effort; load() rejects individually) | — | backlog | unassigned | m9+ |
| m9-01-R3 | m9-01 | `schema_version` is internal-only (not on MCP wire or services-side summary) | — | backlog | unassigned | m9+ |

### Debt findings from m9-02

(empty — last item closed by m9-04, see Terminated terms below)

### Debt findings from m9-04

| ID | Cycle | Cluster | Severity | Priority | Título | Owner | Destino |
|---|---|---|---|---|---|---|---|
| cc-001-god-module | m9-04 | coupling | MEDIUM | P2 | `counterexample_storage.rs` at 2,556 lines; 5 distinct concerns | unassigned | m9+ backlog |
| cc-004-implicit-io-toctou | m9-04 | coupling | LOW | P3 | `save_bundle_record_and_events` opens read-then-write; pre-existing TOCTOU pattern (m9-02 R7) | unassigned | m9+ backlog |

### Debt findings from m9-01

(empty — both FIND-M9-01-DV-COUP-01 and FIND-M9-01-DV-COUP-02 terminated by m9-07/m9-08; see Terminated terms below)

### Follow-ups inherited from prior cycles

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking: variant still fixed in ExistencePredicateShrinker | unassigned | m9+ |
| m8-04-R-hypothesis-fallback | m8-04 | property_target lost in fallback reconstruction | unassigned | m9+ (non-issue post m8-07 but synthetic defaults remain for pre-m8-07 bundles) |

### Findings deferred from m9-72

(none — FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION closed by m9-81)

### Findings deferred from m9-74

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| FIND-M9-74-NATIVE-PTRACE-TESTS-NEED-SERIAL | m9-74 | The `chronos-native` lib suite cannot run in parallel here: two ptrace tests fail and a third blocks in `waitpid` until the harness is killed (17 min, 0% CPU); serial it is 101 passed in 13 s | unassigned | m9+ |

### Findings deferred from m9-75

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| FIND-M9-75-MCP-TOOLS-DO-NOT-DISCLOSE-DEGRADED-STORE | m9-75 | With `CHRONOS_ALLOW_IN_MEMORY_FALLBACK=1` the degraded in-memory mode is logged but never surfaced in a tool response, so an opted-in client cannot tell from a `session_save` / `session_list` payload that nothing is persisted | unassigned | m9+ |

### Findings deferred from m9-71

| ID | Cycle | Título | Owner | Destino |
|---|---|---|---|---|
| FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION | m9-71 | Every archive-manifest that lists `cycles/index.md` / `terms/index.md` SHAs goes stale on the next cycle, so each cycle must rewrite prior manifests' artifact indexes (CC#4); the set grows by one per cycle | unassigned | m9+ |

### Findings deferred from m9-77..m9-79

None — m9-77 wired `session_start{action=attach}`, m9-78 made `session_stop` safe to detach from a traced target, m9-79 distinguished the attach capability value (`probe_type: "ptrace_attach"`) from the misleading `ebpf_user` literal. All three closed in their respective cycles with no deferred findings.

### Findings deferred from m9-80

None — m9-80 closed the property-policy ownership refactor (layered split, spec rev 2): domain owns the four `eval_*` / `observe_property_target` primitives; services wraps via 6 new `From` impls. No wire/protocol change. 552 tests pass across all tiers. No new tests introduced.

### Findings closed in m9-81

| ID | Cycle | Título | Notes |
|---|---|---|---|
| FIND-M9-72-COUNTEREXAMPLE-INLINE-TABLE-CLASSIFICATION | m9-72 | `counterexample_storage.rs` keeps four hand-rolled copies of the read-path `TableDoesNotExist` / else-propagate policy that `chronos-store::table_error` now names | Closed by m9-81 — 6 sites refactored to use `chronos_store::table_error::classify_read_table_error().or_not_found(...)` |

## Terminated terms

| ID | Cycle | Título | Closed by |
|---|---|---|---|
| m8-04-R4 | m8-04 | bundle-as-blob → side table: events still ride inside bundle blob | m9-02-events-side-table (`v0.7.0`) |
| m8-07-R2 | m8-07 | Unknown future wire fields are dropped | m9-01-schema-versioning (`v0.6.0`) |
| FIND-M9-02-DV-API-01 | m9-02 | `save_counterexample_bundle_events` dead API: no caller uses it standalone | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-DOC-01 | m9-02 | `events_count` doc drift: fallback branch not documented at call site | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-OE-01 | m9-02 | Side-table chunk-iteration skeleton duplicated in 3 places | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-COUP-01 | m9-02 | Fallback outside D5 chokepoint: wrong module boundary | m9-03-side-table-debt-cleanup (`v0.7.1`) |
| FIND-M9-02-DV-PERF-01 | m9-02 | Side-table key layout forces full-scan for single-bundle reads | m9-04-side-table-key-layout (`v0.7.2`) |
| cc-002-env-coupling-test | m9-04 | `replay_integration::temp_db_path` reads `std::env::temp_dir()` (test-only) | m9-04-side-table-key-layout (no-action; test-scope coupling accepted) |
| overeng-001-v3-chunk-decode-dup | m9-04 | v3 chunk decode ladder duplicated in `load_counterexample_bundle_events` and `count_counterexample_bundle_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-002-v3-range-scan-verify-dup | m9-04 | v3 range-scan + identity-verify ladder duplicated in `collect_bundle_chunks_range` and `save_bundle_record_and_events` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| overeng-003-events-count-none-branch | m9-04 | `collect_bundle_chunks(events_count: Option<u64>)` Option wrapper; no caller exercises None post-remediation | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| cc-003-wrong-direction-visibility | m9-04 | `storage.rs::db()` widened `pub(crate)` → `pub`; table constants widened to `pub const` | m9-05-side-table-overeng-cleanup (`v0.7.3`) |
| m9-01-R4 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` unused; `#[allow(dead_code)]` | m9-06-known-schema-versions-invariant (`v0.7.4`) |
| FIND-M9-01-DV-OE-01 | m9-01 | `KNOWN_BUNDLE_SCHEMA_VERSIONS` dead speculative code | m9-06-known-schema-versions-invariant (`v0.7.4`) |
| FIND-M9-01-DV-COUP-01 | m9-01 | Duplicated version envelopes: `schema_version` on record + summary, no loader equality check | m9-07-coup-01-invariant-assertion (`v0.7.5`) |
| FIND-M9-01-DV-COUP-02 | m9-01 | List/load policy asymmetry: hard-reject on load vs best-effort include on list | m9-08-list-load-schema-error-variant (`v0.7.6`) |

| FIND-M9-55-FABRICATED-BASE-SHA | m9-38 | apply-checkpoint.json base_sha referenced a non-existent commit (fabricated) | m9-55-apply-checkpoint-fabricated-sha (`v0.7.53`) |
| FIND-M9-69-MCP-STORE-ISOLATION | m9-69 | `chronos-mcp` server tests use the real `$HOME` store; `SessionStore::list_sessions` hard-fails on stale-schema records | m9-70-mcp-store-isolation (`v0.7.72`) |
| FIND-M9-70-SERVICES-TABLE-STRING-MATCH | m9-70 | `chronos-services::sessions::list_sessions` still substring-matches the error text for a missing table; unreachable after the chronos-store fix | m9-71-services-list-store-contract (`v0.7.73`) |
| FIND-M9-71-LOAD-SESSION-TABLE-ERROR-COLLAPSE | m9-71 | Four `chronos-store` read paths turned every `open_table` failure into `Ok(None)`/`Ok(false)`/`SessionNotFound`; `load_session`'s loop over `cas::get` made it silent event loss | m9-72-read-path-table-error-classification (`v0.7.74`) |
| FIND-M9-72-SANDBOX-SHARED-STORE-SAVE-TIMEOUT | m9-72 | Every sandbox client shared one `$HOME/.local/share/chronos/sessions.redb`; intermittent 30 s `tools/call` timeout in `session_save` (`session_persistence.rs:128`/`:133`) | m9-73-sandbox-client-store-isolation (`v0.7.75`) |
| FIND-M9-73-CAS-PUT-ONE-WRITE-TRANSACTION-PER-EVENT | m9-73 | `ContentStore::put` commits one redb write transaction (fsync, immediate durability) per event and `save_session` loops over it, so a 35k-event crash session takes ~3 minutes to save | m9-74-cas-put-many-batching (`v0.7.76`) |
| FIND-M9-73-SESSION-EDGE-CASES-HEAVY-SAVE-NEVER-COMPLETES | m9-73 | `session_edge_cases` fails `test_compare_sessions_crash_vs_normal` and `test_performance_regression_audit_different_workloads` on every run in this environment, on `main` too; symptom of the row above | m9-74-cas-put-many-batching (`v0.7.76`) |
| FIND-M9-73-CC4-REGEN-RITUAL-NOT-IN-REPO | m9-73 | The CC#4 archive-manifest SHA regeneration lived in agent scratch rather than in `scripts/`, so nothing exercised the gate's own logic (the broken awk of m9-66 went unnoticed) and the naive whole-tree version churned the self-referential row of the pre-m9-11 manifests | m9-76-cc4-regen-tool-in-repo (`v0.7.78`) |
| FIND-M9-74-V1-SHIMS-RETURN-V2-ENVELOPE | m9-74 | `compare_sessions` and `performance_regression_audit` stopped returning the flat v1 result in m7-03 (`947e73b`) and returned the tagged `session_compare` envelope instead, against their own descriptions and the output enum's doc; hidden behind the CAS timeout until m9-74 fixed it. Found and fixed in-cycle | m9-74-cas-put-many-batching (`v0.7.76`) |
| FIND-M9-73-SILENT-IN-MEMORY-FALLBACK-MASKS-STORE-OPEN-FAILURE | m9-73 | `chronos-mcp::open_default_store` fell back to an in-memory store when the configured store could not be opened, so a locked store yielded successful saves and empty listings instead of an error (found by falsification of the cycle's own test) | m9-75-fail-closed-store-open (`v0.7.77`) |
## Metadata

| Campo | Valor |
|---|---|
| Project | chronos |
| Vault | `.sddk-knowledge/p-3416cfb8288f8964/` |
| Last updated | 2026-09-14T09:48Z |
| Last archive | m9-81-counterexample-table-classifier |
