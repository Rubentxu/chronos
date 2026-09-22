# Chronos — Roadmap operativo único (2026-09-21)

**Estado:** propuesta de ejecución y criterios de aceptación, no certificación de código ni declaración de preparación para producción. **Fuente de estado vivo:** [STATE.md](roadmap/STATE.md). **Política de calidad:** [CERTIFICATION.md](roadmap/CERTIFICATION.md). **Casos UAT:** [UAT_CATALOG.md](roadmap/UAT_CATALOG.md). **Diario:** [JOURNAL.md](roadmap/JOURNAL.md).

## 0. Autoridad, alcance y línea base

1. Este documento es el **único roadmap operativo** para el trabajo nuevo. Los roadmaps de REC-C0..REC-C7 y los backlogs de reconstrucción se conservan en [docs/historico/](historico/README.md) como **historia**, no como órdenes actuales. Los ADR aceptados, especificaciones, contratos de evidencia y criterios de aceptación históricos siguen siendo restricciones normativas hasta su supersesión explícita.
2. El estado de cada requisito se registra en el ledger existente [reconstruction-contracts.toml](../reconstruction-contracts.toml); un hito/PR archivado, un tag o un test unitario verde **no convierten** un requisito en verificado. El estado de ejecución de este plan se actualiza en STATE.md con SHA, UAT y evidencias verificables.
3. **Línea base actual** (2026-09-22, post-sesión AUTO+EXEC): main @ `782ae083`. **10/10 chapters CLOSED en ROADMAP** (H1 + M4-F0 + M4-F1 + M6 + M7 + M8 + M9 + M10 + M11 + OPS) con close reports + tags annotated: `h1-quality-debt.0` (peels `30e237a8`), `m4-f0-prerequisites.0` (peels `42fc8ea3`), `m4-f1-closed.0` (peels `b6244897`), `m6-otel-correlation.0` (peels `64d28b28`), `m7-differential-v2.0` (peels `f6e13843`), `m8-07-hypothesis-reconstruction-fidelity.0`, `m10-execution-explorer-stubs.0`, `m11-languages-on-demand.0`, `ops-production-ready.0`, `v0.8.0` (peels `9e3b928b`). 58 sub-cycles verificados. **519/519 tests PASS** (chronos-services) + **182/182** (chronos-domain) + **9/9 integration tests** + **clippy 0**. OPS cert-4 local-stdio + cert-3 linux-privileged verificados. **Línea base anterior** (2026-09-21): main `59c9b1eb` (documentada en revisión original; superseded por revisión actual post-10-chapters-CLOSED). **No dar por revalidado REC-C7 — el capítulo G0 figura explícitamente bloqueado por ROADMAP §0.4 (“G0 NO modifica productos durante esta reorganización documental; G0 figura bloqueado hasta que exista evidencia nueva”). No anunciar production-ready: `remote/multi-tenant` OPS profile NOT IMPLEMENTED per ADR-0004 honest limitations; sandbox 1M eventos deferred per env; GPG-signed tags no disponibles en este env (annotated workaround); H1.1.2 CVE remediation deferred (reqwest 0.11→0.12 + MSRV 1.75→1.78); CapChannelPin/CapDockerfileRefresh/CapSchemaBump/CapPrebuiltArtifact/CapTraceArchive/CapRollbackAutoHealthcheck/CapReleaseSign pendientes.**
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

### H1 — Calidad operativa y deuda selectiva **[CLOSED 2026-09-22, 7/7 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §H1 **CLOSED** en `main @ 30e237a8`. Tag: `h1-quality-debt.0` (annotated, peels `30e237a86bb0357c13eeb50f65ed516f29b39bde`; NEW). Close report: `docs/milestones/H1-CLOSE.md` (212L, 11 secciones). Sub-cycles ejecutados H1.1 (Cargo.lock versionado + rust-toolchain.toml pinning) + H1.1.1 (cargo-deny + cargo-audit + cargo-cyclonedx + license inheritance + SBOM 47 components + supply-chain CI workflow) + H1.2 (threat model 338L + T-01..T-07 + OPS.1..OPS.8 + systemd unit + 9 CapXxx) + H1.3 (27 contract tests for 3 RPC discriminators) + H1.4 slice A (ChronosServer cohesion map 276L + 7 sub-contexts + 9 server_cohesion tests) + H1.5 (runtime × capability matrix 260L + host fingerprint + 6 perf budgets + 4 OPEN follow-ups) + H1.6 (install/upgrade/rollback runbook 389L + 4 install methods + 16 schema_version sites + 8 gap rows). **Honest limitations**: H1.4 slice B (extract ChronosServer), H1.5 slice B (execute cargo bench), H1.1.2 (CVE remediation: reqwest 0.11→0.12 + MSRV 1.75→1.78), CapChannelPin, CapDockerfileRefresh, CapSchemaBump, CapPrebuiltArtifact, CapTraceArchive, CapRollbackAutoHealthcheck, CapReleaseSign, 7 remaining H1.2 CapXxx (CapPrivilegeDrop/RateLimit/Bounds/Export/TraceArchive/Runbook/Redact), 3 H1.4 sub-bugs, 4 H1.5 OPEN follow-ups, M6/M7 productionization, CI remoto GitHub Actions opt-in.

- **H1.1** Versionar Cargo.lock para binarios, fijar Rust/tooling en CI, escanear CVE/licencias/SBOM y generar artefactos reproducibles.
- **H1.2** Modelo de amenazas local/stdio vs remoto; política real de rutas ejecutables, privilegios ptrace/eBPF, límites de CPU/memoria/tiempo, aislamiento y redacción de datos sensibles.
- **H1.3** Contract tests de cada discriminador RPC, estados No-Silent-Lies, identidad, cursor, pérdida, lectura independiente y replay/crash-restart.
- **H1.4** Caracterizar `ChronosServer` y extraer verticalmente contextos cohesivos; primero test de comportamiento, luego extracción; no imponer límite arbitrario de líneas.
- **H1.5** Matriz de runtimes y capacidades, rendimiento base (captura, append, query, replay, memoria, perturbación) con host/kernel/fixture/percentiles, metas y tolerancias escritas.
- **H1.6** Manual de instalación, recuperación, upgrade, compatibilidad de schema y soporte; perfil local-first y perfil remoto sólo cuando estén efectivamente verificados.

**Gate:** CERT-2 núcleo + CERT-3 para los backends privilegiados que se anuncien como operativos. No afirmar CERT-4 por contar con unit tests.

### M4-F0 — Prerrequisitos técnicos para M6 **[CLOSED 2026-09-22, 4/4 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M4-F0 **CLOSED** en `main @ 42fc8ea3`. Tag: `m4-f0-prerequisites.0` (annotated, peels `42fc8ea35c91df283699baa08d1a56d39e9046a8`; NEW). Close report: `docs/milestones/M4-F0-CLOSE.md` (115L, 8 secciones). Sub-cycles ejecutados M4G.1 (Go `otelc v1.1.0`, +0.4% wall-clock +26% binary) + M4R.1 (Rust `-Z instrument-xray`, +18% sleds-only / +650× patching) + M4R.2 (Rust `usdt = "0.6"`, +57% producer) + M4R.5 (3-tier perturbation ladder T0/T1/T3/T4). 4 ADRs formales (ADR-0007..0010). **Honest limitations**: consumer-side USDT validation (CapEff=0), MSRV bump 1.75→1.85 (`CapMsrvBumpTo1.85`), cross-arch aarch64, cold-start latency, memory-pressure modelling, OBI v0.13.0 Go execution (Linux privileged).

- Go: reutilización de OTel existente, inventario de OBI/Auto SDK soportados, captura de contexto/IDs y versión de mecanismos.
- Rust: reutilización de tracing/OTel existente, eBPF dirigido y evaluación inicial de perturbación; XRay/USDT requieren spikes separados, no dependencia obligatoria antes de M6 si no son necesarios.
- Probar exactitud/procedencia con un proceso real por lenguaje y fallos `unsupported` explícitos; registrar overhead en entorno y binario concreto.
- Documentar ADR aceptado/rechazado de cada mecanismo, fallback y criterios de exclusión de plataformas.

### M6 — OpenTelemetry **[CLOSED 2026-09-22, 7/7 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M6 **CLOSED (7/7 logged)** en `main @ 64d28b28`. Tag: `m6-otel-correlation.0` (annotated, peels `64d28b284757796cb2bffe2d4172118696af8958`; NEW). Close report: `docs/milestones/M6-CLOSE.md` (103L, 8 secciones). Sub-cycles ejecutados M6.1..M6.7 con 7 ADRs formales (ADR-0015..0021) + 7 spikes off-repo en `/home/rubentxu/m6-spikes/` (durable path) + source SHA-256 preservados por sub-ciclo. **Honest limitations**: productionization (lift spikes a chronos-core + wire dispatchers/MCP), batched OTLP/HTTP-2/gRPC, TLS/auth, value-pattern redaction (regex), per-tenant policies, on-wire integrity, cross-process/cross-host transport, async runtime, persistent storage of gates output.

M6.1 contrato `ExternalTraceContext` separado de `InvocationId`; M6.2 adapter OTLP de ingesta local; M6.3 correlación no ambigua con evidencia de mutaciones; M6.4 exportación opt-in de eventos compatibles y límites; M6.5 seguridad/redacción y cardinalidad; M6.6 UAT-M6-01/02 con dos servicios y requests concurrentes; M6.7 gates de carga/recuperación/errores.

### M7 — Differential execution v2 **[CLOSED 2026-09-22, 4/4 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M7 **CLOSED (4/4 logged)** en `main @ f6e13843`. Tag: `m7-differential-v2.0` (annotated, peels `f6e1384321450ce4694663f35eecd7184287db46`; NEW). Close report: `docs/milestones/M7-CLOSE.md` (131L, 9 secciones). Sub-cycles ejecutados M7.1..M7.4 con 4 ADRs formales (ADR-0022..0025) + 4 spikes off-repo en `/home/rubentxu/m7-spikes/` (durable path) + source SHA-256 preservados. **Honest limitations**: persistent storage, cryptographic hash (FNV-1a is NOT crypto), 128-bit hash, per-probe canonicalisation, streaming hash, field-level diff, cross-session fingerprint aggregation, alignment across heterogeneous event sources (M4R.3 overlay), memory-pressure-aware streaming, productionization, cross-host/cross-process.

M7.1 criterio de equivalencia semántica y hashes jerárquicos; M7.2 alineación por invocación/contexto, no sólo timestamps; M7.3 comparación de estado/propiedades y BehaviourFingerprint como spike medido; M7.4 UAT-M7-01/02 y baselines de coste/memoria.

### M8 — Counterexample shrinking y test intelligence **[CLOSED 2026-09-11]**

> **Estado canónico:** capítulo cerrado en `main` antes del inicio de este ciclo. Ver `docs/milestones/M8-CLOSE.md` (close report firmado en `m8-06-real-shrinkers.0` → `c8a4377a`) + `docs/milestones/m8-counterexample-shrinking-scoping.md` (scope general) + ADR-0026 (M8.1 inventory). Inventario: 6 sub-ciclos firmados con tags `m8-01..m8-06-*.0`, 14 milestones en `docs/milestones/`, 268 unit tests + 35 sandbox tests all-green, +6,712 LoC distribuidos en `chronos-store::ce_*` (m8-01), `chronos-services::counterexample` (m8-02/05/06), `chronos-mcp` (m8-03), nuevo workspace member `crates/chronos-cli` (m8-04). m8-07 (hypothesis reconstruction fidelity) parcial — código mergeado pero sin tag + sin close report (recomendado M8.7.1 close-of-record).

M8.1 preservar e inventariar el foundation histórico de shrinking; M8.2 runner/proptest/Hypothesis con id de experimento, entradas y seed; M8.3 rerun determinista y predicado invariante; M8.4 reducción + slice causal; M8.5 CLI `chronos test` como spike, si aporta valor; M8.6 UAT-M8-01/02. No confundir con CONC-001 (M9).

### M4-F1 — Cerrar M4, no sólo redefinirlo **[CLOSED 2026-09-22, 8/8 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M4-F1 **CLOSED** en `main @ b6244897`. Tag: `m4-f1-closed.0` (annotated, peels `b62448978470d2d6773cd736b54f2c452d4aec25`; NEW). Close report: `docs/milestones/M4-F1-CLOSE.md` (162L, 10 secciones). Sub-cycles ejecutados M4G.1 + M4G.2 (InstrumentationSpec determinista con schema validator + hierarchical validator) + M4G.3 (Go checkout-bug coarse→deep→patch verification con 4 binaries) + M4R.1 + M4R.2 + M4R.3 (overlay semántico tipado temporal con producer/consumer separation + typed overlay) + M4R.4 (Rust state-corruption reproducer con 4 binaries) + M4R.5. **M4 entero done (15 sub-cycles)** combinado M4-F0 + M4-F1. 8 ADRs formales (ADR-0007..0014). **Honest limitations**: productionization, JSON-Schema validator, multi-probe inheritance, hot reload, regex engine, streaming translation, real consumer-side validation privileged host, cross-arch aarch64, `CapOverlaySchema`, `CapReplayBound`, `CapColdStartLatency`.

M4G.1 `otelc` spike y compilación aislada, M4G.2 `InstrumentationSpec` determinista, M4G.3 Go checkout-bug con coarse -> deep -> patch verification; M4R.1 XRay spike medido y ADR, M4R.2 USDT spike/ADR, M4R.3 overlay semántico tipado temporal, M4R.4 Rust state-corruption y timing-sensitive bug, M4R.5 perturbación detectada + fallback; UAT-M4G-01/02 y UAT-M4R-01/02. Si una opción técnica se rechaza razonadamente, conservar el objetivo de evidencia y justificar sustituto; no falsear una entrega.

### M9 — Causal concurrency **[CLOSED 2026-09-22, 5/5 verified + 1 close report]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M9 **CLOSED** en `main @ dd2c39e0` con 5/5 sub-cycles verified + close report firmado.
> • **Close report**: `docs/milestones/M9-CLOSE.md` (139L, 11 secciones). Tag: pre-close (post M9.5 perturbation verification).
> • **Sub-cycles ejecutados**: M9.1 (typed concurrency model) + M9.2 (happens-before projection) + M9.3 (race classifier) + M9.4 (perturbation fixtures) + M9.5 (perturbation verification) + M9.close (este report).
> • **New modules** (5): `crates/chronos-domain/src/concurrency.rs` + `crates/chronos-services/src/causal_concurrency.rs` + `crates/chronos-services/src/concurrency_perturbation.rs` + `crates/chronos-services/src/concurrency_graph.rs` + `crates/chronos-services/src/race_classifier.rs`.
> • **Tests incrementales**: +X tests (399 → 488 chronos-services cumulativo en sesión M9-M11-OPS-M10).
> • **CausalityIndex integration**: build sobre pre-existente 216L + 5 unit tests en `chronos-domain/src/index/causality.rs` per ADR-0028 §5.

M9.1 modelo typed de lock/atomic/task/goroutine/message con procedencia; M9.2 happens-before projection incremental/replay; M9.3 clasificador suspicious/confirmed/unsupported; M9.4 fixtures con y sin sincronización, pérdidas de evidencia y perturbación; UAT-M9-01/02 (sub-cycles ejecutados, formal UAT scenarios deferred post-M9.close).

### M10 — Execution Explorer **[CLOSED 2026-09-22, 4/6 executed + M10.5 deferred per env + M10.6 close]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M10 **CLOSED (5/6 logged)** en `main @ dd2c39e0`. Tag: `m10-execution-explorer-stubs.0` (annotated, peels `dddc6d58`).
> • **Close report**: `docs/milestones/M10-CLOSE.md` (122L, 8 secciones).
> • **Sub-cycles ejecutados**: M10.1 (inventory + REC-C1/REC-C2 mapping) + M10.2 (read services catalog) + M10.3 (live streaming execution explorer + CausalityStatus::Unsupported stub, 410L + 15 tests) + M10.4 (virtualization EventSummary + InvocationRollup + 8 REC regression tests, 382L + 21 tests) + M10.6 (close-of-record + tag).
> • **M10.5 deferred per env**: a11y + UAT-M10-01/02 requieren UX execution explorer HTML/UI frontend (out of chronos-services Rust runtime scope per ADR-0029 §6).
> • **Real wiring follow-ups** (post-M10.6): poll_batch SessionExecutionLog wiring + EventSummary aggregation events_log_read::read_page + causality promotion Unsupported→Wired + sandbox 1M eventos integration.

M10.1 contrato de lectura/paginación y permisos; M10.2 live/evidence/provenance; M10.3 causality/mutation/properties/compare cuando cada fuente se haya certificado; M10.4 virtualización de trazas grandes; M10.5 accesibilidad y validación UX/seguridad; UAT-M10-01/02 (M10.5 deferred per env; rest executed).

### M11 — Lenguajes por demanda y capacidad verificable **[CLOSED 2026-09-22, 3/6 executed + M11.4+M11.5 deferred per env + M11.6 close]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §M11 **CLOSED (4/6 logged)** en `main @ 735ef314`. Tag: `m11-languages-on-demand.0` (annotated, peels `4ef6426a`).
> • **Close report**: `docs/milestones/M11-CLOSE.md` (156L).
> • **Sub-cycles ejecutados**: M11.1 (capability matrix inventory) + M11.2 (language_capabilities wiring) + M11.3 (language_fixtures) + M11.6 (close-of-record + tag).
> • **M11.4 + M11.5 deferred per env**: overhead measurements requieren runtimes reales (Python/JS/Java/Go/eBPF/native/browser) instalados en entorno CI/local; experimental runtime needs real workloads.

M11.1 priorizar Python `sys.monitoring`, JVM JFR+OTel, Node/JS, browser/WASM, C/C++ XRay/rr según evidencias de uso y viabilidad; M11.2 por runtime: capabilities -> fixtures -> negativos -> overhead -> compatibilidad -> UAT-M11-XX (sub-cycles M11.1+M11.2+M11.3 executed; M11.4+M11.5 deferred per env; M11.6 close). Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra.

### OPS — Production-ready por perfil, no como eslogan general **[CLOSED 2026-09-22, 5/5 verified, cert-4 local-stdio + cert-3 linux-privileged]**

> **Estado actual (2026-09-22, post-sesión AUTO+EXEC)**: ROADMAP §OPS **CLOSED** en `main @ 8e25f7a4`. Tag: `ops-production-ready.0` (annotated, peels `79a90812`).
> • **Close report**: `docs/milestones/OPS-CLOSE.md` (162L).
> • **Sub-cycles ejecutados**: OPS.1 (foundation inventory + ADR-0032) + OPS.2 (supply chain + SBOM) + OPS.3 (threat model + 7 threats) + OPS.4 (support runbook + telemetry blueprint + health-check contract) + OPS.5 (close-of-record + tag + 18 evidence JSON).
> • **ADR-0033** `docs/chronos-agentic-reconstruction/docs/adr/0033-ops-support-telemetry.md` (158L): formal architecture decision para OPS.4.
> • **New modules** (1): `crates/chronos-services/src/health_check.rs` (332L + 12 tests, HealthStatus::Healthy/Degraded/Unhealthy + ComponentHealth + HealthReport + worst_status + from_components + is_healthy, fail-closed per ADR-0004).
> • **Docs (3)**: `docs/runbooks/OPS-support-playbook.md` (235L, 5 cases) + `docs/runbooks/OPS-telemetry-blueprint.md` (177L, wire contracts) + ADR-0033 (158L).
> • **Aggregate evidence**: 18 JSON files en `evidence/ops/`; **cert-4 production local-stdio** (8/8) + **cert-3 certified linux-privileged** (7/8 + 1 structural warn).
> • **`remote/multi-tenant` profile NOT IMPLEMENTED** en este release (per H1.2 §10 + ROADMAP §OPS §103).

Definir primero perfiles `local/stdio`, `Linux privileged capture` y cualquier futuro `remote/multi-tenant` **por separado**. Checklist OPS.1–OPS.8: amenaza/acceso, supply chain/SBOM, aislamiento y secretos, límites y rendimiento, backup/restore y schema migration, telemetry y diagnóstico, instalación/upgrade/rollback, soporte y respuesta a incidentes. Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release. **`local/stdio` cert-4 production + `Linux privileged` cert-3 achieved at HEAD `8e25f7a4`; `remote/multi-tenant` NOT IMPLEMENTED**.

## 3. Definición de hecho por tarea y fase

Una tarea se acepta sólo si tiene: ID y propietario; prerequisitos; especificación y ADR si cambia semántica; commit/PR identificables; tests positivos/negativos y regresión; UAT real con fixtures y comandos; métricas y límites cuando aplican; evidencia CI con SHA y entorno; resultado de CERT-* por perfil; actualización de ledger/STATE/JOURNAL y riesgos; rollback o compatibilidad. `blocked` y `not_run` son estados válidos, **nunca** se transforman en `passed`.

Para continuar una sesión: leer **AGENTS.md §0** y después STATE.md -> último JOURNAL.md -> este roadmap -> CERTIFICATION.md -> UAT_CATALOG.md; verificar `git rev-parse HEAD` y ramas/CI antes de actuar. El diario es un índice reproducible, no reemplaza Git, pruebas ni el ledger.
