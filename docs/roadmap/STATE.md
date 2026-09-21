# STATE — Puntero único de continuidad operativa

> Leer **primero** al iniciar sesión, después de `AGENTS.md §0`. No equivale a un resultado de tests. La información siguiente es el snapshot de la auditoría del **2026-09-21**, no una promesa de que el HEAD futuro permanezca igual.

| Campo | Estado |
|---|---|
| Roadmap único | [docs/ROADMAP.md](../ROADMAP.md) |
| Gate operativo siguiente | **G0 — Baseline y recertificación operativa de REC-C7** |
| Estado | **`verified` para el sub-slice G0.3 vault drift sweep** — branch `fix/g0.3-cc8-rec-c5-fabricated-sha` mergeado a `main` en `48e6aefa` (merge --no-ff, 20 commits ahead, 28 files changed, +555/-369) y pusheado a `origin` (local == remote == `48e6aefa`). Tag `v0.7.112` intacto (peel `0be2ec2d`). Operador preautorizó AUTO ("adelante"). Próximo gate de producto: G0.1 (enum `EventsReadKind` falta `Serialize`). |
| Baseline auditado | `main` @ `59c9b1eb0fac9fec63ac766e02ad84dac10a3719` (2026-09-21) |
| HEAD real al retomar | **NO FIJAR AQUÍ:** ejecutar `git fetch origin; git rev-parse HEAD; git rev-parse origin/main; git status --short` |
| HEAD real observado en sesión de recuperación más reciente | Branch `main` @ `48e6aefa` (merge commit del G0.3 sweep, 20 commits ahead of `2c454e0d` baseline); tag `v0.7.112` peels a `0be2ec2d` (intacto). HEAD real se revalida al inicio de cada sesión según §0.1. |
| Hecho histórico | REC-C0..REC-C7 declarados archivados; 19/25 contratos en `verified`, 6 `planned` al baseline; NO representa certificación global |
| Bloqueos conocidos del baseline | CI `35580556913` (boundary_conditions); Coverage `35580556941` (probe_inject); Vault `35580556922` (8 controles drift → 3 restantes en `aeb09099`: CC#11 meta + CC#18 OOS + CC#22 intencional); Architecture `35580556933` y Debt Sentinel `35580556898` green |
| Bloqueos adicionales identificados en recuperación | Drift de mapping M8/M9/M10 en archive-manifest.md y apply-checkpoint.json de REC-C7 (CONC-001→M9, UI-001→M10 según ledger canónico; mi reporte previo decía M8/M9). **CORREGIDO aditivamente en `cf9b3a0a`** (§0.4): sección "Correction filed 2026-09-21" en archive-manifest.md + campo `notes.post_convergence_roadmap_correction_2026_09_21` en apply-checkpoint.json. Pendiente opcional: reemplazar el array `notes.post_convergence_roadmap` con el mapeo correcto en un commit `docs(archive): correct M8/M9/M10 mapping` enlazando JOURNAL row 3. No bloquea G0; el array viejo ya está marcado como `do_not_act_on_old_array`. |
| Estado de G0.3 | **`verified`**. Merge commit `48e6aefa` en `main` (20 commits ahead of `2c454e0d`). CC#8 ✅, CC#11 1 línea (rec-c3.3-train-b suspended — bloqueador meta fuera de scope), CC#13 ✅, CC#17 ✅, CC#18 4 líneas (4 directorios locales untracked, fuera de scope), CC#22 4 líneas (todas en `rec-c3.3-train-b`, intencionalmente intacto per directiva), CC#39 ✅, CC#56 ✅. |
| Última modificación de planificación | Reorganización documental / creación del nuevo roadmap y sistema de certificación, **sin pruebas ni cambios de código**; G0.3 abierto en branch dedicado, **CC#39 y CC#56 cerrados mecánicamente en `08a78c49`+`743fcc7c`+`156736de`, CC#39 canonical-name fix en `3c666091`+`9707d8ac`** |
| Próximas tareas | (1) G0.1 enum `EventsReadKind`: añadir `Serialize` al derive + tests negativos + JSON-RPC e2e (UAT-G0-01); (2) G0.2 migrar aserciones de `probe_inject` a `observe`; (3) G0.4 CI/Coverage/UAT (reparar boundary_conditions + probe_inject); (4) G0.5 coherencia ledger; (5) G0.6 recertificación; (6) G0.7 evidencias. CC#11/CC#18/CC#22 quedan `not_run` por directiva. |
| Siguiente paso inmediato | Delegar investigación de G0.1 a subagente de arquitectura (modelo MiniMax-M3): leer `crates/chronos-services/src/output.rs:1587-1592` + consumidores en `chronos-services/events_read.rs` y `chronos-mcp/server.rs:640`, producir envelope con (a) el cambio mínimo de derive, (b) tests negativos que fallen sin Serialize y pasen con él, (c) un JSON-RPC end-to-end ejercitando el dispatcher. No aplicar hasta revisar el envelope. |
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
