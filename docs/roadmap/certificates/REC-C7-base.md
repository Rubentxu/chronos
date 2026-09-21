# Certificate — REC-C7-base

**Capacidad:** REC-C7 recertificación (post-G0.* correcciones).
**Perfil:** `base` (no privileged).
**SHA validado:** `afa14fd20f191d5884a8b030d4f05a915024f794` (HEAD de `main` en sesión G0.6).
**Tag remoto:** `v0.7.112` peel `0be2ec2d53d9698956ae705938b32b80d7365ad7` (intacto).
**Fecha:** 2026-09-21T16:18Z.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente (mismo SHA, mismo host, run reproducible).

## Alcance

Recertifica REC-C7 **sin alterar el cierre histórico**. El ciclo REC-C7 fue archivado (REC-C0..REC-C7) en `docs/historico/`; esta ficha NO sobreescribe ni reabre ese archivo, sino que publica la **evidencia operativa** de que las correcciones G0.1/G0.2/G0.4 (los sub-bugs de REC-C7 reabiertos en el slice G0) están verdes sobre el SHA actual.

## Limitaciones explícitas

- **No privileged.** El perfil `base` no cubre UAT-G0-04 (uprobe real con ptrace+eBPF). Ver `uat-g0-04-uprobe-privileged-not-run.md`.
- **CI remoto no validado en este SHA.** El gate de UAT-G0-05 documenta qué está verde en local y qué queda pendiente de CI remoto (`bash scripts/check_vault_drift.sh` se ejecuta localmente; `cargo test --workspace --tests` se ejecuta localmente sin GH Actions).
- **Smoke test limitation.** `scripts/smoke_test_ccs.sh` asume vault clean y falla por diseño con la directiva m9-89 que suspende CC#11/CC#18/CC#22 en `rec-c3.3-train-b`. Documentado en STATE; no es regresión.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisitos identificados: UAT-G0-01/02/03 (firmware contract: serde + JSON-RPC client + schema coinciden). ADR-equivalente en STATE.md `Estado de G0.1/2/4` y exploration-report §11/§5. |
| CERT-1 | `passed` | Implementación aislada: T0 fmt + clippy 0 warnings sobre `afa14fd2`; T1 lib 11/12 crates GREEN (services 391/391, mcp 84/84, sandbox 12/12, query/store/capture/log/index/domain/ebpf GREEN); chronos-native serial required `--test-threads=1` per §6.5. |
| CERT-2 | `passed` | Integración canónica: T3 sandbox integration 36/36 GREEN (12 lib + 24 integration: query_tools 1, event_tools 3, query_filters 6+3 ignored legacy, observe_uprobe 2, probe_drain_canonical 4, e2e_connectivity 1, probe_lifecycle_edge_cases 7). 3 `offset_*` tests `#[ignore]`d §0.4 (G0.4-DEBT-01). Sub-bug C5.2 cerrado (G0.4). |
| CERT-3 | `not_run` | Plataforma representativa con uprobe real: requiere host privilegiado (ptrace+eBPF) — fuera de scope de este entorno. Ver `uat-g0-04-uprobe-privileged-not-run.md`. |
| CERT-4 | `not_run` | Perfil operable / production-ready: no intentado. Requiere CERT-3 + threat model + SBOM + runbook. Programado en OPS.1..OPS.8. |

## Mapa UAT IDs

| UAT ID | Perfil | Fixture | Comando | Resultado | Recibo |
|---|---|---|---|---|---|
| UAT-G0-01 | base | wire-smoke `/tmp/g0.1-wire-smoke/` | `cargo run --release` con `CHRONOS_MCP_PATH=…/chronos-mcp` | re-ejecutar en G0.6 (ver `uat-g0-01-events-read-kind.md`) | `/tmp/g0.1-wire-smoke-output.log` |
| UAT-G0-02 | base | wire-smoke `/tmp/g0.2-wire-smoke/` | idem | idem | `/tmp/g0.2-wire-smoke-output.log` |
| UAT-G0-03 | base | wire-smoke `/tmp/g0.4-wire-smoke/` (C5.2 `result.events`) | idem | idem | `/tmp/g0.4-wire-smoke-output.log` |
| UAT-G0-04 | privileged | host con ptrace+eBPF | n/a | `not_run` per directiva | `uat-g0-04-uprobe-privileged-not-run.md` |
| UAT-G0-05 | base | local vault + CI | `bash scripts/check_vault_drift.sh` + `cargo test --workspace --tests --exclude chronos-sandbox --exclude chronos-e2e` | local green; CI remoto pendiente | `uat-g0-05-ci-architecture-vault.md` |

## Pruebas negativas

- `tests/observe_uprobe.rs::test_observe_with_invalid_verb_returns_typed_error` — `observe(verb="frobnicate")` → JSON-RPC -32602 "unknown variant, expected one of 'create'…". G0.2 negativo.
- `tests/observe_uprobe.rs::test_observe_uprobe_against_nonexistent_session_returns_typed_error` — `observe(create, fake-uuid)` → `result.isError=true` con UUID en `content[0].text`. G0.2 negativo.
- `tests/events_read_kind.rs` × 9 — discriminador `EventsReadKind` rechaza `mode="Query"` (capital), `mode="unknown"`, etc. G0.1 negativo.

## Deuda residual aceptada

- DEBT-G0.4-01/02: cerrados en G0.4 (sandbox client migration C5.2 cerrada; struct mirror del wire v2).
- DEBT-G0.5-01: 3 `offset_*` tests `#[ignore]`d, cuerpos preservados verbatim §0.4. Recategorizar en M1+ para migrar a `next_cursor`.
- DEBT-M7-02-01: 4 `probe_inject` tests pre-C5.2 fallan con wrapper `observe` actual; la migración m7-02 está hecha pero los tests pre-existen. Out-of-scope G0; M1+.
- DEBT-G0-04: UAT-G0-04 privileged, env-locked. Out-of-scope G0; M1+.
- DEBT-VAULT-CC-PERMANENT: CC#11/CC#18/CC#22 en `rec-c3.3-train-b` suspended per m9-89 directiva. No son regresión; están listados en STATE explícitamente.
- DEBT-VAULT-CC-REC-C3: 4 `verify-findings.json` missing en `rec-c3.*` out-of-scope per directiva.

## Incompatibilidades / exclusiones

- **Sin CI remoto ejecutado en este SHA.** El proyecto tiene `.github/workflows/` pero no se ha lanzado en `afa14fd2` desde esta sesión. La batería local cubre 100% de los gates definidos para G0 (T0..T3); CI remoto queda como M1+ (H1.1).
- **Sin cobertura Tarpaulin.** El proyecto tiene `cargo-tarpaulin` configurado pero no se ha ejecutado en este SHA. La cobertura histórica queda como M1+.
- **Sin benchmark.** UAT-H1-04 (perf con fixture/host/kernel conocidos) — M1+ (H1.5).

## Fecha / condición de recertificación

Recertificar este certificado cuando:

1. Cualquier cambio material en `crates/chronos-mcp/src/lib.rs`, `crates/chronos-sandbox/src/client/tools.rs`, o `crates/chronos-services/src/output.rs:1587` (puntos donde viven los 3 contratos G0.1/G0.2/G0.4).
2. Cualquier bump de `rmcp`, `serde`, `serde_json`, `schemars` o `tokio` que afecte wire-shape.
3. Cualquier nuevo tool MCP introducido en `tools::list` (el contrato canónico de 41 tools podría cambiar).
4. Recertificación programada al menos cada release candidate o cada 90 días, lo que ocurra antes.

## Historial

- 2026-09-21: emitido (este certificado).
