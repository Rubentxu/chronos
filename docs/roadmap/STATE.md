# STATE — Puntero único de continuidad operativa

> Leer **primero** al iniciar sesión, después de `AGENTS.md §0`. No equivale a un resultado de tests. La información siguiente es el snapshot de la auditoría del **2026-09-21**, no una promesa de que el HEAD futuro permanezca igual.

| Campo | Estado |
|---|---|
| Roadmap único | [docs/ROADMAP.md](../ROADMAP.md) |
| Gate operativo siguiente | **G0 — Baseline y recertificación operativa de REC-C7** |
| Estado | **G0.3 sub-slice `verified`** (mergeado en `90655a69`); **G0.1 `verified`** (mergeado en `0f773810`) — fix de 2 líneas en `crates/chronos-services/src/output.rs:1587` añade `Serialize` al derive + `#[serde(rename_all = "snake_case")]` y alinea el contrato Serde con el JsonSchema ya existente. Verificación wire-level end-to-end (`/tmp/g0.1-wire-smoke`) confirma que el binario real acepta `"mode":"query"` y `"mode":"by_id"` y rechaza `"mode":"Query"` con `expected 'query' or 'by_id'`. Sub-bug pre-existente detectado pero NO G0.1: el cliente sandbox `query_events` lee `events` raíz pero el server v2 publica `result.events` (refactor C5.2) — documentado en `exploration-report.md` §11.4, fuera de scope. T1 sandbox integration sigue `not_run` global hasta que ese sub-bug del cliente se arregle en M1+. |
| Baseline auditado | `main` @ `59c9b1eb0fac9fec63ac766e02ad84dac10a3719` (2026-09-21) |
| HEAD real al retomar | **NO FIJAR AQUÍ:** ejecutar `git fetch origin; git rev-parse HEAD; git rev-parse origin/main; git status --short` |
| HEAD real observado en sesión de recuperación más reciente | Branch `main` @ `0f773810` (post G0.1 merge); tag `v0.7.112` peels a `0be2ec2d` (intacto). HEAD real se revalida al inicio de cada sesión según §0.1. |
| Hecho histórico | REC-C0..REC-C7 declarados archivados; 19/25 contratos en `verified`, 6 `planned` al baseline; NO representa certificación global |
| Bloqueos conocidos del baseline | CI `35580556913` (boundary_conditions); Coverage `35580556941` (probe_inject); Vault `35580556922` (8 controles drift → 3 restantes: CC#11 meta + CC#18 OOS + CC#22 intencional, no cerradas per directiva); Architecture `35580556933` y Debt Sentinel `35580556898` green |
| Bloqueos adicionales identificados en este turno (2026-09-21T13:06Z) | **T1 sandbox integration: `not_run` por stall + por sub-bug de cliente** — el intento de `cargo test --workspace --lib` quedó 22 min stalled en `chronos_native` (cancelado). El intento de `cargo test -p chronos-sandbox --test query_tools` y `--test event_tools` después del rebuild expone `RpcError("missing field 'events'")` — el cliente sandbox `chronos-sandbox/src/client/tools.rs::query_events` lee `events` raíz pero el server v2 publica `result.events` (refactor `C5.2 migration`). El discriminador wire (`mode: "query"`) ahora se acepta (G0.1 verde), pero el cliente espera la forma vieja de la respuesta. **Sub-bug pre-existente en `main` (HEAD `90655a69` y `0f773810`), no introducido por G0.1 ni por G0.3** — es regresión de C5.2 que afecta al cliente, no al server. Documentado en `cycle-artifacts/p-3416cfb8288f8964/g0.1-events-read-kind-contract-align/exploration-report.md` §11.4. Pendiente M1+: update sandbox client to read `result.events` from v2 events_read response. T1 no debe figurar como verde hasta que ese sub-bug cierre. |
| Estado de G0.3 | **`verified`**. Merge commit `48e6aefa` en `main` (20 commits ahead of `2c454e0d`). CC#8 ✅, CC#11 1 línea (rec-c3.3-train-b suspended — bloqueador meta fuera de scope), CC#13 ✅, CC#17 ✅, CC#18 4 líneas (4 directorios locales untracked, fuera de scope), CC#22 4 líneas (todas en `rec-c3.3-train-b`, intencionalmente intacto per directiva), CC#39 ✅, CC#56 ✅. |
| Estado de G0.1 | **`verified`**. Merge commit `0f773810` en `main` (3 commits ahead of `2ee989ba`; branch `fix/g0.1-events-read-kind-contract-align` @ `7843407a`). 2 líneas en `crates/chronos-services/src/output.rs:1587` añaden `Serialize` al derive + `#[serde(rename_all = "snake_case")]`. 9/9 contract tests en `crates/chronos-services/tests/events_read_kind.rs` pasan. Wire-level smoke (`/tmp/g0.1-wire-smoke/`, cliente JSON-RPC manual sin rmcp) contra el binario real `chronos-mcp` confirma `mode: "query"` / `mode: "by_id"` aceptados y `mode: "Query"` rechazado con el mensaje canónico `expected 'query' or 'by_id'`. |
| Última modificación de planificación | G0.3 merge `48e6aefa`; G0.1 merge `0f773810`; AGENTS.md canónico en `aeb09099` |
| Próximas tareas | **G0.2 — migrar aserciones `probe_inject` → `observe`** (siguiente slice tras G0.1 verde). Cadena restante: (1) **G0.4** CI/Coverage/UAT; (2) **G0.5** coherencia ledger; (3) **G0.6** recertificación; (4) **G0.7** evidencias. CC#11/CC#18/CC#22 quedan `not_run` per directiva — **visibles, no desaparecidos**. T1 sandbox integration sigue `not_run` global hasta que el sub-bug del cliente sandbox (`result.events` vs `events`) se cierre en M1+. Sub-bug NO bloquea G0.2 (es independiente del dispatch `observe`). |
| Siguiente paso inmediato | Abrir `fix/g0.2-observe-uprobe-migration` desde `main @ 0f773810`. Caracterización: localizar todas las aserciones de test y ejemplos que llaman `probe_inject`; mapear al equivalente `observe(verb="create", action="inject_uprobe", ...)`; reemplazar; añadir tests negativos para los verbs (`create`/`list`/`query`/`delete`); verificar que el discriminador `verb` ya está alineado en `ObserveVerb` (verificar el patrón del archivo); UAT-G0-02 (observabilidad unificada). En paralelo, delegar al subagente de T1 la corrección del sub-bug `result.events` para destapar T1 integration verde antes de cerrar G0. |
| Actualización del puntero | Actualizar este archivo y añadir fila fechada a JOURNAL.md al cerrar cada sesión/ciclo, indicando SHA real y evidencias |

## Protocolo de recuperación sin memoria implícita

1. Leer `AGENTS.md`, este STATE, últimas entradas de [JOURNAL.md](JOURNAL.md), [ROADMAP.md](../ROADMAP.md), [CERTIFICATION.md](CERTIFICATION.md), [UAT_CATALOG.md](UAT_CATALOG.md) y `reconstruction-contracts.toml`; respetar ADRs vigentes.
2. Verificar rama/HEAD/origin/main, PR o ciclo abierto, árbol de trabajo, último checkpoint y CI del SHA; comparar con el snapshot anterior. Si cambió, **reconciliar STATE antes de continuar**.
3. Clasificar lo previo: implementado/mergeado, sólo propuesto, verificado en otro SHA, bloqueado y desconocido. No sumar trabajo de ramas no mergeadas a main.
4. Elegir sólo una slice preparada, trazarla a hito/UAT/contratos, comprobar baseline, operar con branch y pruebas proporcionales (AGENTS T0..T5 y CERT T6); registrar resultados no ejecutados como `not_run`.
5. Actualizar JOURNAL con fecha, rama, commit, qué se hizo/no se hizo, gates y enlaces a recibos, riesgos y **próxima acción exacta**. Mover el puntero aquí a la primera tarea no terminada. No inventar hashes/porcentajes.
6. Si el cambio es sólo documentación, **no** declarar CI ni UAT verdes. Si main ya tiene cambios nuevos, no sobrescribir su estado con el snapshot inicial.

## Semántica de progreso

`not_started | in_progress | blocked | implemented_unverified | verified | certified<CERT-n> | superseded`. Usar `verified` sólo si la evidencia reproducible existe para el SHA y perfil correspondiente; reflejar avance por tareas con denominador congelado e indicar que no pondera valor funcional. No afirmar `production-ready` sin CERT-4 por perfil.

## Enlaces directos a baseline CI

- CI: https://github.com/Rubentxu/chronos/actions/runs/35580556913
- Coverage: https://github.com/Rubentxu/chronos/actions/runs/35580556941
- Vault: https://github.com/Rubentxu/chronos/actions/runs/35580556922
- Architecture: https://github.com/Rubentxu/chronos/actions/runs/35580556933
