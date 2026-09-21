# STATE — Puntero único de continuidad operativa

> Leer **primero** al iniciar sesión, después de `AGENTS.md §0`. No equivale a un resultado de tests. La información siguiente es el snapshot de la auditoría del **2026-09-21**, no una promesa de que el HEAD futuro permanezca igual.

| Campo | Estado |
|---|---|
| Roadmap único | [docs/ROADMAP.md](../ROADMAP.md) |
| Gate operativo siguiente | **G0 — Baseline y recertificación operativa de REC-C7** |
| Estado | **not_started** en este plan; documentación reorganizada, ninguna corrección de producto implementada en esta entrega |
| Baseline auditado | `main` @ `59c9b1eb0fac9fec63ac766e02ad84dac10a3719` (2026-09-21) |
| HEAD real al retomar | **NO FIJAR AQUÍ:** ejecutar `git fetch origin; git rev-parse HEAD; git rev-parse origin/main; git status --short` |
| Hecho histórico | REC-C0..REC-C7 declarados archivados; 19/25 contratos en `verified`, 6 `planned` al baseline; NO representa certificación global |
| Bloqueos conocidos del baseline | CI `35580556913` (boundary_conditions); Coverage `35580556941` (probe_inject); Vault `35580556922` (8 controles drift); Architecture `35580556933` y Debt Sentinel `35580556898` green |
| Última modificación de planificación | Reorganización documental / creación del nuevo roadmap y sistema de certificación, **sin pruebas ni cambios de código** |
| Próximas tareas | G0.1 enum, G0.2 observe, G0.3 drift, G0.4 CI/coverage/UAT, G0.5 coherencia ledger, G0.6 recertificación |
| Siguiente paso inmediato | Revisar si main avanzó y reproducir fallos del HEAD actual antes de corregir; seleccionar **una** tarea G0 |
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
