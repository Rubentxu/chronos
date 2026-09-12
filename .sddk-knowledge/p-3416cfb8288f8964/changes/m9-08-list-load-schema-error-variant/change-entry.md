# Change: m9-08 list load schema error variant

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m9-08-list-load-schema-error-variant` |
| Path | `B-direct` |
| Status | CLOSED |
| Base SHA | `50553969309b945156c239fd5a5a7f794431df21` |
| Head SHA | `d89862bbe67256cf6274be1d71ee7f8857cb8808` |
| Tag | `v0.7.6` (annotated, peel matches published SHA) |

## Commits

| SHA | Subject |
|---|---|
| `d89862b` | fix(m9-08): dedicated StoreError::SchemaTooNew variant (closes FIND-M9-01-DV-COUP-02) |
| `cb47fe8` | docs(m9-08): light-verify evidence (R1 PASS, FIND-M9-01-DV-COUP-02 closed) |

## Scope

Trivial B-direct cleanup closing the long-outstanding **FIND-M9-01-DV-COUP-02**
(coupling · LOW P3) finding from the m9-01 cycle. The debt-report prescribed
exactly one remediation:

> **Remediation (backlog):** at the first real v2 bump, add a dedicated error
> variant (e.g. `SchemaTooNew { found, supported }`).

m9-01 itself shipped three schema versions (`KNOWN_BUNDLE_SCHEMA_VERSIONS = [1, 2, 3]`,
`CURRENT_BUNDLE_SCHEMA_VERSION = 3`), so the trigger condition has been
satisfied for two cycles. m9-08 takes the prescribed action:

- `StoreError` in `error.rs` gained a new `SchemaTooNew { found: u32, supported: u32 }`
  variant with a `Display` impl that emits the same `"newer than supported"`
  substring the pre-fix `Serialization` message contained (preserves
  log-scraper compatibility) plus the structured fields for callers to
  branch on without message parsing.
- `load_counterexample_bundle` previously returned
  `StoreError::Serialization(format!(...))` for the forward-compat reject.
  It now returns `StoreError::SchemaTooNew { found, supported }`, so callers
  pattern-matching on `Serialization` to catch corrupt blobs no longer
  accidentally catch the forward-compat rejection.
- Two pre-existing tests (`m9_02_future_version_load_is_rejected` and
  `m9_04_saved_v3_schema_and_future_rejection`) gained a `match &err { ... }`
  block that pattern-matches on the new variant, so a future regression
  that re-overloads `Serialization` would be caught at the type level
  instead of the message-string level.
- One new unit test (`error::tests::test_schema_too_new_variant_display_and_fields`)
  pins the `Display` output and the field access pattern.

The list-side best-effort semantics (m9-02 R2 disclosure) are preserved
unchanged. The asymmetry between `list_counterexample_bundles` (best-effort
include) and `load_counterexample_bundle` (hard-reject) is a deliberate
product call and not the target of this finding.

### Changed paths

- `crates/chronos-store/src/error.rs` — added `SchemaTooNew { found, supported }`
  variant with doc comment + 1 unit test (Display + field pattern match).
- `crates/chronos-store/src/counterexample_storage.rs` — loader swap from
  `Serialization(format!(...))` to `SchemaTooNew { found, supported }` +
  `match &err { ... }` block added to 2 pre-existing tests.

## Findings resolved

| ID | Cluster | Título | Closed by |
|---|---|---|---|
| FIND-M9-01-DV-COUP-02 | coupling | List/load policy asymmetry shipped as an error-kind overload (forward-compat reject should not be `Serialization`) | m9-08-list-load-schema-error-variant (`v0.7.6`) |

Verdict: **PASS** (1/1 finding closed)

## Findings NOT resolved (m9+ backlog inherited)

| ID | Cluster | Severity | Title | Reason |
|---|---|---|---|---|
| cc-001-god-module | coupling | MEDIUM | `counterexample_storage.rs` at ~2.6K LoC (+27 LoC this cycle, +141 net m9-04..m9-08) | Splits require design pass + compat shim |
| cc-004-implicit-io-toctou | coupling | LOW | `save_bundle_record_and_events` opens read-then-write | Concurrency design required |
| m9-02 R1-R8 | various | — | See `changes/m9-02-events-side-table/change-entry.md` | Deferred from m9-02 |
| m8-06 R4 | various | — | Cross-variant existence predicate shrinking | Deferred from m8-06 |
| m8-04-R-hypothesis-fallback | various | — | `property_target` lost in fallback reconstruction | Deferred from m8-04 |
