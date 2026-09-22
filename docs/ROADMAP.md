# Chronos — Roadmap operativo único (2026-09-21)

**Estado:** propuesta de ejecución y criterios de aceptación, no certificación de código ni declaración de preparación para producción. **Fuente de estado vivo:** [STATE.md](roadmap/STATE.md). **Política de calidad:** [CERTIFICATION.md](roadmap/CERTIFICATION.md). **Casos UAT:** [UAT_CATALOG.md](roadmap/UAT_CATALOG.md). **Diario:** [JOURNAL.md](roadmap/JOURNAL.md).

## 0. Autoridad, alcance y línea base

1. Este documento es el **único roadmap operativo** para el trabajo nuevo. Los roadmaps de REC-C0..REC-C7 y los backlogs de reconstrucción se conservan en [docs/historico/](historico/README.md) como **historia**, no como órdenes actuales. Los ADR aceptados, especificaciones, contratos de evidencia y criterios de aceptación históricos siguen siendo restricciones normativas hasta su supersesión explícita.
2. El estado de cada requisito se registra en el ledger existente [reconstruction-contracts.toml](../reconstruction-contracts.toml); un hito/PR archivado, un tag o un test unitario verde **no convierten** un requisito en verificado. El estado de ejecución de este plan se actualiza en STATE.md con SHA, UAT y evidencias verificables.
3. Baseline revisada el 2026-09-21: main 59c9b1eb0fac9fec63ac766e02ad84dac10a3719. REC-C0..REC-C7 **figuran archivados**; 19/25 requisitos figuran verified y 6 planned (M4A/B, OTEL, DIFF, CONC, UI). En ese SHA fallaban CI, Coverage y Vault Drift; Architecture Contracts y Sandbox Debt Sentinel terminaban correctamente. **No dar por revalidado REC-C7, ni anunciar “production-ready”.**
4. El cierre del gate documental REC-C7 y la **recertificación operativa del HEAD** son hechos diferentes. Se conserva la historia; G0 recupera la garantía operativa sin reescribir cierres históricos. M4A/B no se convierten en completos por haberse reasignado a M4-future.
5. No confundir ciclos internos antiguos `m6-*`, `m7-*`, `m8-*`, `m9-*`, `m10-*` con hitos **oficiales** M6–M11. Las nuevas tareas usan IDs únicos G0.*, H1.*, M4G.*, M4R.*, M6.*, ..., OPS.*.
6. Un solo gate de producto **activo** por vez. Spikes aislados o mantenimiento de gobernanza pueden ejecutarse en paralelo si no alteran la línea base ni se presentan como certificación. Las fases son dependencias, no promesas calendáricas; no inventar % de avance.

## 1. Invariantes innegociables

- ExecutionLog por sesión es la evidencia autoritativa; lecturas independientes con EventSeq, gap, cursor, procedencia, replay y sellado veraces. Ninguna cola destructiva equivale a evidencia histórica.
- No Silent Lies: `unknown`, `unsupported`, `incomplete`, pérdida y heurísticas distinguibles de `complete`/éxito. Las afirmaciones de capacidad se acotan por backend, versión y entorno.
- Dominio sin infraestructura; servicios consumen puertos y la raíz de composición conecta adaptadores; evitar duplicación de políticas entre MCP, CLI y servicios.
- Las identidades estáticas de símbolos, las invocaciones dinámicas y los trace/span externos son distintas. Nunca convertir EventSeq o tiempo monotónico en timestamp Unix.
- Seguridad de ejecución y de artefactos, reproducción local-first, límites de recursos, aislamiento de procesos, retención/recuperación y protección de datos forman parte de la definición de terminado.
- La interfaz agent-first debe ser tipada y coherente con el JSON Schema; no eliminar herramientas si todavía no tienen sustituto verificado; preferir verticales a infraestructuras especulativas.

## 2. Camino crítico temporal y hitos

| Orden | Gate / entregable | Dependencias | Condición de salida verificable |
|---|---|---|---|
| **G0** | Baseline operativa y veracidad | Ninguna | Mismo SHA: CI, Coverage, Architecture Contracts, Vault Drift, pruebas contractuales y UAT obligatorias verdes; evidencia de privilegios separada |
| **H1** | Endurecimiento transversal M1–M5 | G0 | Seguridad y reproducibilidad controladas; ciclo de vida y API v2 sin regresiones; deuda categorizada, no “todo refactorizado” |
| **M4-F0** | M4 Go/Rust: mecanismos mínimos y contratos de captura reutilizables | H1 | Spikes OTel/OBI/eBPF y matriz de capacidades con resultados medidos; no reclamar M4 completo |
| **M6** | OTLP ingestión, correlación y exportación | M4-F0 y G0/H1 | Dos servicios reales: trace/span externo -> invocación -> mutación -> violación, con procedencia y tiempos correctos |
| **M7** | Ejecución diferencial semántica | M6 para alineación distribuida; M1/M2/M3 acreditados | Comparación tolerante a ruido de tiempo/orden detecta la divergencia causal conocida |
| **M8** | Contraejemplos y test intelligence end-to-end | M3 y G0/H1; M7 sólo para los flujos que usan comparación | Input mínimo conserva **la misma** violación al reejecutar; artefactos reproducibles y procedencia persistida |
| **M4-F1** | Instrumentación adaptativa Go y Rust completa | M4-F0, M6; M8 para fixtures de reproducción | Go y Rust, individualmente, pasan deepening/overlay + comparación de perturbación + fallback y UAT de bug real |
| **M9** | Inteligencia de concurrencia happens-before | M1/M2 y medidas de perturbación M4-F1 | Fixture sincronizado no se clasifica como carrera confirmada; fixture sin sincronizar produce evidencia causal, no sólo ventanas temporales |
| **M10** | Execution Explorer | M6–M9: sólo las vistas cuya evidencia ya esté verificada | Trazas voluminosas mediante paginación/virtualización, no materialización completa; renderiza incertidumbre/procedencia |
| **M11** | Profundidad multilenguaje | Núcleo y plataforma certificados; M4 para capacidades adaptativas | Matriz por lenguaje y runtime con UAT real y límites declarados, sin equivalencia ficticia |
| **OPS** | Certificación de publicación/production-ready **por perfil de despliegue** | Funcionalidades incluidas certificadas y plataforma comprobada | CERT-4, seguridad, recuperación, rendimiento, documentación, instalación y operación probados en entorno representativo |

**Dependencias laterales:** M8 puede avanzar tras H1 sin esperar toda M7 si no consume su comparador; M4-F1 puede diseñarse por slices después de M6 y antes de cerrar M8, pero **no se declara completo** hasta cubrir sus dependencias de prueba. OPS es un trabajo transversal desde G0 y un gate de publicación, no una “fase final de endurecimiento”. No ampliar GUI, protocolo remoto ni nuevos lenguajes para ocultar un fallo del núcleo.

### G0 — Recuperar la confianza en main (primera iteración obligatoria)

- **G0.1** Reparar el enum `EventsReadKind`: serde y schemars deben aceptar/publicar `query` y `by_id`; tests negativos y JSON-RPC end-to-end.
- **G0.2** Migrar las aserciones antiguas de `probe_inject` al contrato tipado de `observe`, conservando tests reales de error y una UAT privilegiada de inyección.
- **G0.3** Diagnosticar y resolver los ocho controles de Vault Drift sin fabricar SHAs ni relajar la validación.
- **G0.4** CI, Coverage y contratos sobre el mismo SHA; caracterizar pruebas privilegiadas PTR-001..003. La CI normal no finge cubrirlas.
- **G0.5** Rectificar discrepancias de documentación/ledger (p. ej. M8 != CONC-001/M9, M10=UI-001); añadir contratos de M8 y M11 cuando haya UAT ejecutable. Reconciliar alcance de los 29 tools adicionales.
- **G0.6** Evidencia de revalidación REC-C7 posterior a las correcciones, sin alterar el cierre histórico. Certificar C0–C2 del baseline conforme a CERTIFICATION.md.
- **G0.7** Establecer SHA, duración de ejecución, perfil de test, artefactos y matriz de fallos en el registro de STATE; no sustituir tests rojos por skips silenciosos.

**G0 NO modifica productos durante esta reorganización documental.** G0 figura bloqueado hasta que exista evidencia nueva; documentar sus tareas no equivale a ejecutarlas.

### H1 — Calidad operativa y deuda selectiva

- **H1.1** Versionar Cargo.lock para binarios, fijar Rust/tooling en CI, escanear CVE/licencias/SBOM y generar artefactos reproducibles.
- **H1.2** Modelo de amenazas local/stdio vs remoto; política real de rutas ejecutables, privilegios ptrace/eBPF, límites de CPU/memoria/tiempo, aislamiento y redacción de datos sensibles.
- **H1.3** Contract tests de cada discriminador RPC, estados No-Silent-Lies, identidad, cursor, pérdida, lectura independiente y replay/crash-restart.
- **H1.4** Caracterizar `ChronosServer` y extraer verticalmente contextos cohesivos; primero test de comportamiento, luego extracción; no imponer límite arbitrario de líneas.
- **H1.5** Matriz de runtimes y capacidades, rendimiento base (captura, append, query, replay, memoria, perturbación) con host/kernel/fixture/percentiles, metas y tolerancias escritas.
- **H1.6** Manual de instalación, recuperación, upgrade, compatibilidad de schema y soporte; perfil local-first y perfil remoto sólo cuando estén efectivamente verificados.

**Gate:** CERT-2 núcleo + CERT-3 para los backends privilegiados que se anuncien como operativos. No afirmar CERT-4 por contar con unit tests.

### M4-F0 — Prerrequisitos técnicos para M6

- Go: reutilización de OTel existente, inventario de OBI/Auto SDK soportados, captura de contexto/IDs y versión de mecanismos.
- Rust: reutilización de tracing/OTel existente, eBPF dirigido y evaluación inicial de perturbación; XRay/USDT requieren spikes separados, no dependencia obligatoria antes de M6 si no son necesarios.
- Probar exactitud/procedencia con un proceso real por lenguaje y fallos `unsupported` explícitos; registrar overhead en entorno y binario concreto.
- Documentar ADR aceptado/rechazado de cada mecanismo, fallback y criterios de exclusión de plataformas.

### M6 — OpenTelemetry

M6.1 contrato `ExternalTraceContext` separado de `InvocationId`; M6.2 adapter OTLP de ingesta local; M6.3 correlación no ambigua con evidencia de mutaciones; M6.4 exportación opt-in de eventos compatibles y límites; M6.5 seguridad/redacción y cardinalidad; M6.6 UAT-M6-01/02 con dos servicios y requests concurrentes; M6.7 gates de carga/recuperación/errores.

### M7 — Differential execution v2

M7.1 criterio de equivalencia semántica y hashes jerárquicos; M7.2 alineación por invocación/contexto, no sólo timestamps; M7.3 comparación de estado/propiedades y BehaviourFingerprint como spike medido; M7.4 UAT-M7-01/02 y baselines de coste/memoria.

### M8 — Counterexample shrinking y test intelligence **[CLOSED 2026-09-11]**

> **Estado canónico:** capítulo cerrado en `main` antes del inicio de este ciclo. Ver `docs/milestones/M8-CLOSE.md` (close report firmado en `m8-06-real-shrinkers.0` → `c8a4377a`) + `docs/milestones/m8-counterexample-shrinking-scoping.md` (scope general) + ADR-0026 (M8.1 inventory). Inventario: 6 sub-ciclos firmados con tags `m8-01..m8-06-*.0`, 14 milestones en `docs/milestones/`, 268 unit tests + 35 sandbox tests all-green, +6,712 LoC distribuidos en `chronos-store::ce_*` (m8-01), `chronos-services::counterexample` (m8-02/05/06), `chronos-mcp` (m8-03), nuevo workspace member `crates/chronos-cli` (m8-04). m8-07 (hypothesis reconstruction fidelity) parcial — código mergeado pero sin tag + sin close report (recomendado M8.7.1 close-of-record).

M8.1 preservar e inventariar el foundation histórico de shrinking; M8.2 runner/proptest/Hypothesis con id de experimento, entradas y seed; M8.3 rerun determinista y predicado invariante; M8.4 reducción + slice causal; M8.5 CLI `chronos test` como spike, si aporta valor; M8.6 UAT-M8-01/02. No confundir con CONC-001 (M9).

### M4-F1 — Cerrar M4, no sólo redefinirlo

M4G.1 `otelc` spike y compilación aislada, M4G.2 `InstrumentationSpec` determinista, M4G.3 Go checkout-bug con coarse -> deep -> patch verification; M4R.1 XRay spike medido y ADR, M4R.2 USDT spike/ADR, M4R.3 overlay semántico tipado temporal, M4R.4 Rust state-corruption y timing-sensitive bug, M4R.5 perturbación detectada + fallback; UAT-M4G-01/02 y UAT-M4R-01/02. Si una opción técnica se rechaza razonadamente, conservar el objetivo de evidencia y justificar sustituto; no falsear una entrega.

### M9 — Causal concurrency **[SCOPED 2026-09-22, NOT STARTED execution]**

> **Estado actual (2026-09-22)**: ROADMAP §M9 §91 está **scope + architectured**, listo para ejecución, pero **NO se ha ejecutado** ningún sub-cycle de causal concurrency en `main @ bce07690`.  
> **Punteros canónicos**:  
> • **ADR-0028** `docs/chronos-agentic-reconstruction/docs/adr/0028-m9-scoping-causal-concurrency.md` (NEW, 185L, 9 secciones §1..§9) — formal architecture decision: build on `CausalityIndex` pre-existente (216L + 5 unit tests en `crates/chronos-domain/src/index/causality.rs`), NO reinventar desde cero. 6 sub-cycles M9.2..M9.6 + M9.1 inventory.  
> • **M9.1 ADR-0027** `docs/chronos-agentic-reconstruction/docs/adr/0027-m9.1-inventory-causal-concurrency.md` (284L, 9 secciones) — inventory + naming convention proposal (vault cycles `cc-m9-NN` vs ROADMAP §M9 `M9.N`).  
> • **M9-SCOPING.md** `docs/milestones/M9-SCOPING.md` (237L, 6 secciones) — operacional reference con 6 sub-cycles propuestos M9.2..M9.6, integration con foundation pre-existente.  
> • **Foundation pre-existente reutilizable** (descubierto durante M9 scoping): `CausalityIndex` (216L + 5 tests) + `IndexBuilder.causality` (1 test) + `detect_concurrent_access` heurística (1 test) + `detect_races` MCP tool — todos verificados en ADR-0028 §5.  
> • **Próximos pasos**: M9.2 execute (D1 typed model en `chronos-domain::concurrency`) ó decisión operador entre execute vs OPS push.  
> **Convención de naming** (ADR-0027 §2.3): ROADMAP §M9 sub-cycles usan prefijo `M9.N` (capital M + dot, matching M7/M8); vault cycles futuros usan `cc-m9-NN` o `vault-m9-NN` (no `feat(m9-NN)`) para evitar colisión.

M9.1 modelo typed de lock/atomic/task/goroutine/message con procedencia; M9.2 happens-before projection incremental/replay; M9.3 clasificador suspicious/confirmed/unsupported; M9.4 fixtures con y sin sincronización, pérdidas de evidencia y perturbación; UAT-M9-01/02.

### M10 — Execution Explorer **[SCOPED 2026-09-22, NOT STARTED execution]**

> **Estado actual (2026-09-22)**: ROADMAP §M10 §95 está **scope + architectured**, listo para ejecución, pero **NO se ha ejecutado** ningún sub-cycle de Execution Explorer en `main @ ce61e16b`.  
> **Punteros canónicos**:  
> • **ADR-0029** `docs/chronos-agentic-reconstruction/docs/adr/0029-m10-scoping-execution-explorer.md` (NEW, 212L, 9 secciones §1..§9) — formal architecture decision: consolidar sobre foundation pre-existente (~3,662 LoC + 54 unit tests) en lugar de reinventar pagination/reading desde cero. 6 sub-cycles M10.2..M10.6 + M10.1 inventory.  
> • **M10-SCOPING.md** `docs/milestones/M10-SCOPING.md` (171L, 8 secciones) — operacional reference con 6 sub-cycles propuestos M10.2..M10.6, integration con foundation pre-existente.  
> • **Foundation pre-existente reutilizable** (descubierto durante M10 scoping): `EventsCursorV1` (386L + 13 tests, REC-C1.1 cursor canónico) + `events_log_read::read_page` (1507L + 30 tests, paginación production-grade) + `CanonicalDrainPage` (737L + 11 tests, streaming canónico) + `ChronosEventsReadService` (426L, REC-C1.3 v2 dispatcher) + `DebugReadService` (606L, 7 métodos read-only) — todos verificados en ADR-0029 §5.  
> • **REC-C1/REC-C2 acceptance**: 8 UATs en `MILESTONE_ACCEPTANCE.md §99-§124` (cursor semantics + gap truth + isolation + replay safety + single-truth). M10.4 las corre como regression suite.  
> • **Product design**: `docs/chronos-agentic-reconstruction/docs/gui/EXECUTION_EXPLORER.md` (39L, 7 vistas: Live + Execution + Causality + Mutation Lens + Hypotheses/Properties + Compare + Evidence inspector).  
> • **Próximos pasos**: M10.2 execute (wire shape unificado + permissions enum) ó decisión operador entre execute vs OPS push.  
> **Convención de naming** (ADR-0027 §2.3 + ADR-0028 §2.4 + ADR-0029 §2.4): ROADMAP §M10 sub-cycles usan prefijo `M10.N` (capital M + dot, matching M7/M8/M9).

M10.1 contrato de lectura/paginación y permisos; M10.2 live/evidence/provenance; M10.3 causality/mutation/properties/compare cuando cada fuente se haya certificado; M10.4 virtualización de trazas grandes; M10.5 accesibilidad y validación UX/seguridad; UAT-M10-01/02.

### M11 — Lenguajes por demanda y capacidad verificable **[SCOPED 2026-09-22, NOT STARTED execution]**

> **Estado actual (2026-09-22)**: ROADMAP §M11 §99 está **scope + architectured**, listo para ejecución, pero **NO se ha ejecutado** ningún sub-cycle de M11 en `main @ 1663aada`.  
> **Punteros canónicos**:  
> • **ADR-0031** `docs/chronos-agentic-reconstruction/docs/adr/0031-m11-scoping-languages-on-demand.md` (NEW, 206L, 9 secciones §1..§9) — formal architecture decision: consolidar sobre **7 adapter crates pre-existentes** (~19,854 LoC + 274 unit tests) en lugar de reinventar adapters desde cero. 6 sub-cycles M11.2..M11.6 + M11.1 inventory.  
> • **M11-SCOPING.md** `docs/milestones/M11-SCOPING.md` (175L, 8 secciones) — operacional reference con 6 sub-cycles propuestos M11.2..M11.6, integration con foundation masiva pre-existente.  
> • **Foundation pre-existente reutilizable** (descubierto durante M11 scoping): 7 adapter crates (chronos-python 1,653L+27 tests + chronos-java 2,562L+44 + chronos-js 1,683L+12 + chronos-go 1,703L+24 + chronos-ebpf 2,034L+35 + chronos-native 7,086L+87 + chronos-browser 3,133L+45) = **19,854 LoC + 274 tests** + `Language` enum canónico de 14 variants en `crates/chronos-domain/src/trace/session.rs:10` + `LanguageAdapterStatus` wiring en `crates/chronos-services/src/output.rs:2231` + manual-ai docs (EN+ES, 523L total) — todos verificados en ADR-0031 §5.  
> • **Próximos pasos**: M11.2 execute (capability matrix ejecutable + certification tier CERT-1..CERT-4) ó decisión operador entre execute vs OPS push.  
> **Convención de naming** (ADR-0027 §2.3 + ADR-0028 §2.4 + ADR-0029 §2.4 + ADR-0031 §2.3): ROADMAP §M11 sub-cycles usan prefijo `M11.N` (capital M + dot, matching M7/M8/M9/M10).

M11.1 priorizar Python `sys.monitoring`, JVM JFR+OTel, Node/JS, browser/WASM, C/C++ XRay/rr según evidencias de uso y viabilidad; M11.2 por runtime: capabilities -> fixtures -> negativos -> overhead -> compatibilidad -> UAT-M11-XX. Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra.

### OPS — Production-ready por perfil, no como eslogan general

Definir primero perfiles `local/stdio`, `Linux privileged capture` y cualquier futuro `remote/multi-tenant` **por separado**. Checklist OPS.1–OPS.8: amenaza/acceso, supply chain/SBOM, aislamiento y secretos, límites y rendimiento, backup/restore y schema migration, telemetry y diagnóstico, instalación/upgrade/rollback, soporte y respuesta a incidentes. Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release.

## 3. Definición de hecho por tarea y fase

Una tarea se acepta sólo si tiene: ID y propietario; prerequisitos; especificación y ADR si cambia semántica; commit/PR identificables; tests positivos/negativos y regresión; UAT real con fixtures y comandos; métricas y límites cuando aplican; evidencia CI con SHA y entorno; resultado de CERT-* por perfil; actualización de ledger/STATE/JOURNAL y riesgos; rollback o compatibilidad. `blocked` y `not_run` son estados válidos, **nunca** se transforman en `passed`.

Para continuar una sesión: leer **AGENTS.md §0** y después STATE.md -> último JOURNAL.md -> este roadmap -> CERTIFICATION.md -> UAT_CATALOG.md; verificar `git rev-parse HEAD` y ramas/CI antes de actuar. El diario es un índice reproducible, no reemplaza Git, pruebas ni el ledger.
