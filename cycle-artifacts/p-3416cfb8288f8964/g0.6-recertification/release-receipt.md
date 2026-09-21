# Release Receipt — g0.6-recertification

**Cycle:** `g0.6-recertification`
**Project:** p-3416cfb8288f8964
**Branch:** `fix/g0.6-recertification`
**Status:** CLOSED

## Required fields

| Field | Value |
|---|---|
| **Head SHA** | `afa14fd20f191d5884a8b030d4f05a915024f794` (post-G0.6 branch tip) |
| **Remote tag** | `v0.7.112` |
| **Remote tag_peel** | `0be2ec2d53d9698956ae705938b32b80d7365ad7` |
| **Peel match** | `true` (tag peel resolves to the same tree that G0.5's `a00845eb` and G0.6 docs commit reference) |

## Verification matrix

### T0 — Format / Lint

| Suite | Result |
|---|---|
| `cargo fmt --all -- --check` | exit 0 ✅ |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 ✅ (post fix of 2 pre-existing clippy::doc_list_item_without_indent) |

### T1 — Lib unit tests

| Crate | Result | Notes |
|---|---|---|
| `chronos-services` | 391/391 ✅ | |
| `chronos-mcp` | 84/84 ✅ | |
| `chronos-sandbox` (lib) | 12/12 ✅ | |
| `chronos-native` (serial) | 109/109 ✅ | `--test-threads=1` per AGENTS.md §6.5; 12.87s |
| `chronos-query`, `chronos-store`, `chronos-capture`, `chronos-log`, `chronos-index`, `chronos-domain`, `chronos-ebpf` | GREEN | `test result: ok` with 0 failed |

### T3 — Sandbox integration subset (8 suites, serial)

| Suite | Result |
|---|---|
| `query_tools` | 1/1 ✅ |
| `event_tools` | 3/3 ✅ |
| `query_filters` | 6/6 ✅ + 3 `#[ignore]`d §0.4 |
| `observe_uprobe` | 2/2 ✅ |
| `probe_drain_canonical` | 4/4 ✅ |
| `e2e_connectivity` | 1/1 ✅ |
| `probe_lifecycle_edge_cases` | 7/7 ✅ |
| `analytics_tools` | 4/4 ✅ (serial; concurrent mass-run falla por timeout — pre-existing flakiness §6.5) |

### T4 — Wire smoke (3 smokes)

| Smoke | Result |
|---|---|
| `/tmp/g0.1-wire-smoke/` | 3/3 GREEN — `mode=query|by_id` aceptados; `mode=Pascal` rechazado con `-32602 "unknown variant 'Query', expected 'query' or 'by_id'"` |
| `/tmp/g0.2-wire-smoke/` | 3/3 GREEN — `observe(frobnicate)` → -32602; `observe(create, fake-uuid)` → typed error; `observe(scope=42)` → -32602 internally tagged |
| `/tmp/g0.4-wire-smoke/` | wire shape C5.2 confirmado — 9 keys top-level (`completeness`, `gap_summary`, `mode`, `next_cursor`, `provenance`, `result`, `retention`, `session_id`, `tail`); `result.events` nested |

### Vault Drift

| CC | Status | Reason |
|---|---|---|
| CC#4 | GREEN | 102 manifests clean post-fixpoint of m9-72..m9-76 |
| CC#11 | 1 drift | `rec-c3.3-train-b` suspended per m9-89 directiva |
| CC#18 | 4 drift | 4 `rec-c3.*` out-of-scope per directiva |
| CC#22 | 4 drift | `rec-c3.3-train-b` suspended per m9-89 directiva |

`scripts/smoke_test_ccs.sh` falla por diseño con directivas suspended — limitation del test runner, NO regresión.

### Certificates issued

- `docs/roadmap/certificates/REC-C7-base.md`
- `docs/roadmap/certificates/uat-g0-01-events-read-kind-base.md`
- `docs/roadmap/certificates/uat-g0-02-observe-uprobe-base.md`
- `docs/roadmap/certificates/uat-g0-03-cursor-gap-replay-base.md`
- `docs/roadmap/certificates/uat-g0-04-uprobe-privileged-not_run.md`
- `docs/roadmap/certificates/uat-g0-05-ci-architecture-vault-base.md`

### Cycle artifacts

- `cycle-artifacts/p-3416cfb8288f8964/g0.6-recertification/exploration-report.md`
- `cycle-artifacts/p-3416cfb8288f8964/g0.6-recertification/apply-checkpoint.json`
- `cycle-artifacts/p-3416cfb8288f8964/g0.6-recertification/verify-findings.json`
- `cycle-artifacts/p-3416cfb8288f8964/g0.6-recertification/release-receipt.md` (este archivo)

## Out-of-scope (per directiva / M1+)

- DEBT-G0.5-01: 3 `offset_*` tests `#[ignore]`d §0.4; migration a `next_cursor` en M1+.
- DEBT-M7-02-01: 4 `probe_inject` legacy prefix en m7-02 debt; M1+.
- DEBT-G0-04: UAT-G0-04 privileged, env-locked; M1+.
- DEBT-VAULT-CC-PERMANENT + DEBT-VAULT-CC-REC-C3: suspended per m9-89 directiva.
- Concurrent mass-run flakiness: documented en AGENTS.md §6.5.

## Limitations

- **No privileged:** UAT-G0-04 not_run per directiva.
- **No CI remoto:** `.github/workflows/` not run on `afa14fd2`.
- **No Tarpaulin:** cobertura no ejecutada.
- **No benchmark:** perf fixtures/percentiles no ejecutados (UAT-H1-04 = M1+).

## Date / recertification trigger

Recertificar cuando:

1. Cambie `crates/chronos-services/src/output.rs:1587` (`EventsReadKind`) o el wire shape de `events_read`.
2. Cambie `ObserveVerb`/`ObserveScopeWire` o el wrapper `observe`.
3. Cambie `TraceEvent`/`GetEventResponse`/`V2Query`/`V2Result` structs.
4. Cualquier bump de `rmcp`, `serde`, `serde_json`, `schemars` que afecte wire-shape.
5. Cualquier nuevo tool MCP introducido en `tools::list`.
6. Recertificación programada al menos cada release candidate.

## Historial

- 2026-09-21T16:30Z: emitido (este certificado).
