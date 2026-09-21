# STATE — Puntero único de continuidad operativa

> Leer **primero** al iniciar sesión, después de `AGENTS.md §0`. No equivale a un resultado de tests. La información siguiente es el snapshot de la auditoría del **2026-09-21**, no una promesa de que el HEAD futuro permanezca igual.

| Campo | Estado |
|---|---|
| Roadmap único | [docs/ROADMAP.md](../ROADMAP.md) |
| Gate operativo siguiente | **G0 — Baseline y recertificación operativa de REC-C7** |
| Estado | **not_started** en este plan; documentación reorganizada, ninguna corrección de producto implementada en esta entrega |
| Baseline auditado | `main` @ `59c9b1eb0fac9fec63ac766e02ad84dac10a3719` (2026-09-21) |
| HEAD real al retomar | **NO FIJAR AQUÍ:** ejecutar `git fetch origin; git rev-parse HEAD; git rev-parse origin/main; git status --short` |
| HEAD real observado en sesión de recuperación | `5981d12f5b500c9030464b8859cc4f1ff97a201f` == `origin/main` tras el slice docs-only (commits `cf9b3a0a` corrección M8/M9/M10 + `5981d12f` regen CC#4). HEAD real se revalida al inicio de cada sesión según §0.1. |
| Hecho histórico | REC-C0..REC-C7 declarados archivados; 19/25 contratos en `verified`, 6 `planned` al baseline; NO representa certificación global |
| Bloqueos conocidos del baseline | CI `35580556913` (boundary_conditions); Coverage `35580556941` (probe_inject); Vault `35580556922` (8 controles drift); Architecture `35580556933` y Debt Sentinel `35580556898` green |
| Bloqueos adicionales identificados en recuperación | Drift de mapping M8/M9/M10 en archive-manifest.md y apply-checkpoint.json de REC-C7 (CONC-001→M9, UI-001→M10 según ledger canónico; mi reporte previo decía M8/M9). **CORREGIDO aditivamente en `cf9b3a0a`** (§0.4): sección "Correction filed 2026-09-21" en archive-manifest.md + campo `notes.post_convergence_roadmap_correction_2026_09_21` en apply-checkpoint.json. Pendiente opcional: reemplazar el array `notes.post_convergence_roadmap` con el mapeo correcto en un commit `docs(archive): correct M8/M9/M10 mapping` enlazando JOURNAL row 3. No bloquea G0; el array viejo ya está marcado como `do_not_act_on_old_array`. |
| Última modificación de planificación | Reorganización documental / creación del nuevo roadmap y sistema de certificación, **sin pruebas ni cambios de código** |
| Próximas tareas | G0.1 enum, G0.2 observe, G0.3 drift, G0.4 CI/coverage/UAT, G0.5 coherencia ledger, G0.6 recertificación |
| Siguiente paso inmediato | Slice docs-only **completado** en este turn: corrección M8/M9/M10 + CC#4 regen (`cf9b3a0a` + `5981d12f`); JOURNAL row 3 + STATE actualizados. Gate activo sigue siendo G0 `not_started`. Próximos slices posibles, en orden de menor a mayor coste: (a) opcional — reemplazar `notes.post_convergence_roadmap` array en el JSON con el mapeo correcto (commit docs-only, enlaza JOURNAL row 3); (b) G0.3 vault drift sweep de los 8 CCs pre-existentes (mecánico, ~30 min); (c) G0.1 reparar enum `EventsReadKind` (serde+schemars, tests, UAT-G0-01/02, ~1-2 h); (d) G0.2 observe uprobe contract (~2-3 h). G0.4–G0.7 dependen de G0.1/G0.2. |
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
