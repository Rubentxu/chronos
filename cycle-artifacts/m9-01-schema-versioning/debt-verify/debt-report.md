# Debt Report — m9-01-schema-versioning (post-verify debt gate)

**Verdict: `PASS_WITH_WARNINGS`** · re_iterate_from: `none` · fail_closed: true

| Subject | Value |
|---|---|
| Branch | `feat/m9-01-schema-versioning` |
| Base → Head | `bceddc946b73209756d3360b0aec4dca592296c3` → `0aec8bac30f995e638e2a656b55935f48a60623d` |
| Diff digest | `1d6e9cc1b652c1dd1c77c37c74f6f5556d76005d2802d1a7ac4a62fc5977141c` |
| Verify evidence | `verify-report.md` (sha256 `2a5eeee3…71d4f`), verdict `PASS_WITH_WARNINGS`, subject bound to head ✔ |
| Path / depth | A-min / **smoke** (required clusters: coupling, overeng) |
| Coverage | 2/2 required clusters completed, 0 failed |
| Generated | 2026-09-11T17:35:30Z |

Source of truth: `debt-report.json` (sha256 `5351e72d4f7cd9a634348cc4b5e99518ab312826d0f220458728f71de54f599a`). This Markdown is derived and never overrides the JSON.

## Counts

| Dimension | Value |
|---|---|
| Severity | 0 critical · 0 high · **1 medium** · **2 low** (total 3) |
| Confidence | 3 high · 0 medium · 0 low |
| Attribution | **3 introduced** · 0 pre-existing · 0 unknown |
| Cluster | coupling 2 · overeng 1 |

## Findings

### FIND-M9-01-DV-COUP-01 — MEDIUM · introduced (backlog, P2)

**Duplicated version envelopes: `schema_version` exists both on the record envelope and the nested summary with no equality enforcement.**
`crates/chronos-store/src/counterexample_storage.rs:171-179` (summary field) and `:195-207` (record field, doc-declared authoritative). `save()` canonicalizes both (lines 234-235, D5), so no drift is persistable through the public API today — but the loader checks only `record.schema_version > CURRENT` (line 293); a hand-constructed record (e.g. a future migration tool) with a mismatched summary version would silently pass.

**Remediation (backlog):** add a 3-line loader assertion `record.schema_version == summary.schema_version`, or drop the nested field at the next wire-format break.

### FIND-M9-01-DV-COUP-02 — LOW · introduced (backlog, P3)

**List/load policy asymmetry shipped as an error-kind overload.**
`load_counterexample_bundle` hard-rejects future versions as `StoreError::Serialization` with an upgrade message (lines 286-299) while `list_counterexample_bundles` best-effort includes them (pinned by test, verify disclosure R2). The asymmetry is a deliberate, disclosed product call; the cost is that callers cannot distinguish "corrupt blob" from "written by newer chronos" without parsing the message.

**Remediation (backlog):** at the first real v2 bump, add a dedicated error variant (e.g. `SchemaTooNew { found, supported }`).

### FIND-M9-01-DV-OE-01 — LOW · introduced (backlog, P3)

**`KNOWN_BUNDLE_SCHEMA_VERSIONS` is dead speculative code.**
`counterexample_storage.rs:54-61`: declared with `#[allow(dead_code)]`, exactly one grep hit in the workspace (its own declaration), disclosed honestly as R4 in verify. 3 LOC reducible, change risk LOW.

**Remediation (backlog):** delete it and re-add at the first version-set tightening (YAGNI); the lint-cascade justification is weak for a single item.

## Clean dimensions (no findings)

- **Hidden dependencies:** none added — no ambient state, no implicit I/O outside the store boundary, no env coupling, no time/randomness in changed business logic.
- **Global state:** none — no module-level mutables, no singletons, no registries.
- **Dependency direction:** correct — chronos-store does not depend on chronos-services (Cargo.toml confirmed); only doc-comment mirrors, no wrong-direction imports. Fan-in of `counterexample_storage` is 5 modules, fan-out of `counterexample.rs` is 3 internal crates: both far below thresholds.
- **No circular imports** introduced.
- **Debt markers:** zero `ponytail:` items in scope.
- **Bloat trajectory:** STABLE — 969 insertions over the cycle, of which 518 are the scoping doc and 68 the apply-checkpoint; ~138 production LOC vs ~303 test LOC (deliberate investment, no new abstractions).

## Decision reasons (table order)

1. **no-critical-introduced** — no CRITICAL findings; no circular dependency, no unencapsulated shared mutable state, no LSP violation.
2. **no-high-introduced** — zero HIGH findings with HIGH/MEDIUM confidence.
3. **warning-band-below-thresholds** — 1 MEDIUM + 2 LOW introduced, all unsuppressed, below the 3-MEDIUM FAIL threshold → `PASS_WITH_WARNINGS`, backlog attached.
4. **pre-existing-policy** — no pre-existing findings re-discovered; per cycle-7b, **no INC files required**.

## Waivers

None.

## Cluster runs

| Cluster | Status | Attempts | Analyzer |
|---|---|---|---|
| coupling | completed | 1 | inline Rust-adapted catalog (grep + read), debt-gate/v1 + MiniMax-M3 |
| overeng | completed | 1 | smoke inline catalog + scoped debt-marker scan, debt-gate/v1 + MiniMax-M3 |

Note: cluster subagent spawn was unavailable in this session's light-swarm mode; the coordinator executed both cluster catalogs inline over the same evidence, at A-min smoke scope. All cluster runs carry `subject_sha = 0aec8ba…`.

## Runtime handoff

`specification_only` — no debt-specific CLI transition is declared. This cycle is ad-hoc (`sddk cycle status` → STORAGE_NOT_FOUND for `m9-01-schema-versioning`); the `release.complete` debt gates (`debt-severity-assigned`, `debt-priority-assigned`) were **not** evaluated via the CLI and are owned by the orchestrator. The debt report above satisfies their intent: verdict `PASS_WITH_WARNINGS`, severities and priorities assigned per finding.

## Next

`sddk-release` (verdict mapping: PASS/PASS_WITH_WARNINGS → success → sddk-release).
