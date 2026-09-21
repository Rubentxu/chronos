# STATE — Puntero único de continuidad operativa

> Leer **primero** al iniciar sesión, después de `AGENTS.md §0`. No equivale a un resultado de tests. La información siguiente es el snapshot de la auditoría del **2026-09-21**, no una promesa de que el HEAD futuro permanezca igual.

| Campo | Estado |
|---|---|
| Roadmap único | [docs/ROADMAP.md](../ROADMAP.md) |
| Gate operativo siguiente | **G0 — Baseline y recertificación operativa de REC-C7** |
| Estado | **G0.3 sub-slice `verified`** (mergeado en `90655a69`); **G0.1 en caracterización**: descubrimiento de drift pre-existente entre JsonSchema (snake_case `"query"`/`"by_id"`), Serde (PascalCase `"Query"`/`"ById"` — sin `rename_all`), y cliente JSON-RPC (`"mode":"query"` snake_case). El cliente recibe `-32602 "unknown variant query, expected Query or ById"`. T1 sandbox integration **NO estaba verde** — el test `query_tools::test_query_events_after_probe_stop` falla con ese error; mi reporte previo "12/12 verde" sólo cubría `--lib`, no integration. Operador reencuadre 2026-09-21T13:06Z: T1 no figura como verde; controles residuales del vault se mantienen visibles; G0.1 = caracterización → revisión propuesta subagente → implementación + UAT → G0.2; T1 pendiente y controles residuales no bloquean tareas independientes pero tampoco desaparecen del estado. |
| Baseline auditado | `main` @ `59c9b1eb0fac9fec63ac766e02ad84dac10a3719` (2026-09-21) |
| HEAD real al retomar | **NO FIJAR AQUÍ:** ejecutar `git fetch origin; git rev-parse HEAD; git rev-parse origin/main; git status --short` |
| HEAD real observado en sesión de recuperación más reciente | Branch `main` @ `90655a69`; tag `v0.7.112` peels a `0be2ec2d` (intacto). HEAD real se revalida al inicio de cada sesión según §0.1. |
| Hecho histórico | REC-C0..REC-C7 declarados archivados; 19/25 contratos en `verified`, 6 `planned` al baseline; NO representa certificación global |
| Bloqueos conocidos del baseline | CI `35580556913` (boundary_conditions); Coverage `35580556941` (probe_inject); Vault `35580556922` (8 controles drift → 3 restantes: CC#11 meta + CC#18 OOS + CC#22 intencional, no cerradas per directiva); Architecture `35580556933` y Debt Sentinel `35580556898` green |
| Bloqueos adicionales identificados en este turno (2026-09-21T13:06Z) | **T1 sandbox integration: `not_run` por stall + por bug pre-existente** — el intento de `cargo test --workspace --lib` quedó 22 min stalled en `chronos_native` (cancelado). El intento de `cargo test -p chronos-sandbox --test query_tools` después del rebuild expone que `test_query_events_after_probe_stop` **falla con `-32602 "unknown variant query, expected Query or ById"`** — drift entre JsonSchema (snake_case) y Serde (PascalCase, sin `rename_all`) en `EventsReadKind`. **El bug es pre-existente en `main` (HEAD `90655a69`), no introducido por mi merge G0.3** — verificable con `git log --all -- crates/chronos-services/src/output.rs` que los commits de mi rama sólo tocan derive del CC#56 (verify_expected_sha), no este enum. Los 19 ficheros de test del sandbox que llaman `query_events`/`get_event` están en el mismo estado roto silenciosamente. Esto NO bloquea el ciclo siguiente (G0.1 ataca la causa raíz) pero T1 no debe figurar como verde en ningún reporte futuro hasta que la integración pase. |
| Estado de G0.3 | **`verified`**. Merge commit `48e6aefa` en `main` (20 commits ahead of `2c454e0d`). CC#8 ✅, CC#11 1 línea (rec-c3.3-train-b suspended — bloqueador meta fuera de scope), CC#13 ✅, CC#17 ✅, CC#18 4 líneas (4 directorios locales untracked, fuera de scope), CC#22 4 líneas (todas en `rec-c3.3-train-b`, intencionalmente intacto per directiva), CC#39 ✅, CC#56 ✅. |
| Última modificación de planificación | Reorganización documental / creación del nuevo roadmap y sistema de certificación, **sin pruebas ni cambios de código**; G0.3 abierto en branch dedicado, **CC#39 y CC#56 cerrados mecánicamente en `08a78c49`+`743fcc7c`+`156736de`, CC#39 canonical-name fix en `3c666091`+`9707d8ac`** |
| Próximas tareas | **G0.1 de extremo a extremo** (per re-encuadre 2026-09-21T13:06Z): (1) **caracterización independiente** ✅ hecha en JOURNAL row 9 — drift entre JsonSchema/Serde/cliente en `EventsReadKind`; (2) **revisión de propuesta subagente** — re-delegar con nuevo encuadre (alinear los 3 contratos, no trivialmente "añadir Serialize"); (3) **implementación + UAT** — fix mínimo que iguala Schema+Serde+cliente al mismo string, con tests negativos + JSON-RPC e2e (`test_query_events_after_probe_stop` debe pasar); (4) tras G0.1 verde, **G0.2** migrar aserciones `probe_inject` → `observe`; (5) **G0.4** CI/Coverage/UAT; (6) **G0.5** coherencia ledger; (7) **G0.6** recertificación; (8) **G0.7** evidencias. CC#11/CC#18/CC#22 quedan `not_run` per directiva — **visibles, no desaparecidos**. T1 sandbox integration sigue `not_run` hasta que G0.1 cierre el bug y los 19 tests pasen. |
| Siguiente paso inmediato | Re-delegar G0.1 a subagente de arquitectura (modelo: mismo, `claude-sonnet-4-6`) con contexto completo: (a) los 3 contratos observados (Schema snake_case, Serde PascalCase, cliente snake_case); (b) el resultado del experimento aislado `/tmp/serde_test`; (c) el panic real de `test_query_events_after_probe_stop`; (d) el requisito UAT-G0-01 literal; (e) el requisito explícito del operador: "Serde, JSON Schema y cliente JSON-RPC comparten exactamente el mismo contrato". El envelope debe demostrar con código/tests que el mismo string `"query"`/`"by_id"` se acepta y se publica en los tres puntos. **No aplicar hasta revisar el envelope.** En paralelo, delegar al subagente de T1 el barrido completo de los 19 integration tests del sandbox (no aplicar fix, sólo inventariar cuáles fallan y por qué) para no bloquear G0.1 con un solo test. |
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
