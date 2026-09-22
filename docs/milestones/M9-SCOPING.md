# M9 scoping — Causal concurrency para Chronos

**Cycle:** `m9-causal-concurrency` (scoping)
**Branch:** TBD on execute approval
**Base:** `c7fc9c11` (post M9.1 inventory, ADR-0027)
**Status:** PROPOSED — 2026-09-22 (post M9.1)
**Precedence:** ROADMAP §M9 §87; `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` §M9; `docs/roadmap/UAT_CATALOG.md` §M9; ADR-0027 §7 (NOT STARTED declaration).

## §1 Problem statement

ROADMAP §M9 §87 — "M9.1 modelo typed de lock/atomic/task/goroutine/message con procedencia; M9.2 happens-before projection incremental/replay; M9.3 clasificador suspicious/confirmed/unsupported; M9.4 fixtures con y sin sincronización, pérdidas de evidencia y perturbación; UAT-M9-01/02."

El estado actual del repo, según ADR-0027 §2.1 y §2.2 + inspección directa:

- **0 commits ROADMAP §M9 ejecutados.** Causal concurrency NO se ha construido como clasificador ni como happens-before projection.
- **Existe `CausalityIndex` en `crates/chronos-domain/src/index/causality.rs`** (NEW, 216L, 5 unit tests) — foundation pre-existente para tracking de write mutations con provenancia (`CausalityEntry` con `event_id`, `timestamp`, `thread_id`, `function`, `file:line`, `value_before`/`value_after`). Métodos: `record_write`, `find_last_mutation`, `trace_lineage`, `writes_at`. Es **infraestructura de indexación causal**, NO clasificación de races; usable como building block para M9 D2 (happens-before graph) y D3 (classifier). Integrado en `chronos-index::builder::IndexBuilder.causality` + 1 test `test_build_causality_index`.
- **Existe `detect_concurrent_access` en `crates/chronos-query/src/engine.rs:766`** — **heurística de triage** ("two writes to the same address from different threads within threshold_ns"; el doc-string explícitamente dice "we do not run happens-before analysis, so this is a triage signal, not a verdict"). Renombrado de `detect_races` a `detect_concurrent_access` en m0-09 (commit `25f698e8`, 2026-09-08) precisamente para subrayar que NO prueba races. Internamente usa `CausalityIndex`.
- **`detect_concurrent_access` tiene 1 unit test** (`test_detect_concurrent_access_100ns_threshold` en `engine.rs:1363`).
- **Hay sandbox tests**: `chronos-sandbox/tests/race_depth.rs` ejercita el path de race detection.
- **`crates/chronos-services/src/debug_trace_specialized.rs:152` `detect_races`** es el MCP tool que envuelve `detect_concurrent_access`; ya clasifica como "triage signal" en su doc-string.

Es decir: hay una heurística de **detección naive** (address+thread+time) + un index causal con provenancia, pero NO hay:

1. **Modelo typed de concurrencia** (lock/atomic/task/goroutine/message con procedencia).
2. **Happens-before projection** (derivación de orden parcial desde eventos del runtime).
3. **Clasificador suspicious/confirmed/unsupported** (state machine explícito).
4. **Fixtures con y sin sincronización** (positivos para no-reportar-false-positive + negativos para reportar-real-race).
5. **Tolerancia a perturbación + pérdida de evidencia** (qué pasa cuando faltan eventos sync).

## §2 Goal

Construir el sub-sistema de causal concurrency de Chronos que:

1. **Modela typed concurrency primitives** con procedencia (origen: probe + thread + timestamp + owning subsystem).
2. **Deriva un partial order (happens-before)** incremental y reproducible desde los eventos observados.
3. **Clasifica cada acceso a memoria compartida** en una de tres categorías explícitas: `suspicious` (no se puede probar), `confirmed` (prueba por happens-before ausencia), `unsupported` (evidencia insuficiente).
4. **Cumple UAT-M9-01 y UAT-M9-02** (ver §6).

## §3 Non-goals

Estos quedan explícitamente fuera del scope de M9:

- **Construir un runtime tracer de locks/atomics/tasks propio**. Chronos no intercepta syscalls de sincronización; lee eventos post-hoc del runtime observado (ptrace, USDT, eBPF, OTel — todos capabilities ya certificados en M4/M6). El "modelo typed" se construye sobre los eventos que el tracer ya emite, no sobre tracing nuevo.
- **Reemplazar `detect_concurrent_access` en m0**. La heurística naive (address+thread+time) sigue siendo útil como **fast pre-filter** que reduce la entrada a la fase de happens-before analysis. M9 la envuelve, no la reemplaza.
- **Soporte para todos los lenguajes**. M9 entrega para los lenguajes ya certificados en M4/M6 (Rust + Go mínimo, vía eBPF/OTel). Otros lenguajes (Python, JVM, JS) entran en M11.
- **Confirmar races en producción**. M9 entrega el clasificador; la decisión de "esto es un bug real" sigue siendo del developer/agent. M9 nunca dice "esto es un bug", solo "esto NO tiene happens-before suficiente".
- **Cross-session causality**. Sesión es la unidad de análisis; cross-session es M10+.
- **Probabilistic race detection**. ThreadSanitizer-style; no es causal concurrency.

## §4 Architecture decisions (proposed)

### Decision D1 — Modelo typed por evento, no por primitiva global

```rust
// crates/chronos-domain/src/concurrency.rs (NEW)
pub enum ConcurrencyPrimitive {
    Lock { id: LockId, address: u64 },
    Atomic { id: AtomicId, address: u64, op: AtomicOp },
    Task { id: TaskId, kind: TaskKind },  // thread/goroutine/task
    Message { id: MessageId, channel: ChannelId },
}

pub struct Provenance {
    pub probe: ProbeId,
    pub thread: ThreadId,
    pub timestamp: TimestampNs,
    pub subsystem: SubsystemId,  // chronos-native, OTel, USDT, etc.
}

pub struct TypedConcurrencyEvent {
    pub primitive: ConcurrencyPrimitive,
    pub access: MemoryAccess,   // read/write/read-modify-write
    pub address: u64,
    pub provenance: Provenance,
}
```

**Rationale:** modelar la primitiva en el evento (no en un registro global de locks) permite reconstruir happens-before desde eventos observados sin asumir que el runtime expone una tabla de locks. El tracer puede no emitir eventos de lock explícitos (p. ej. futex en Linux) pero igual registrar accesos — la reconstrucción infiere la sincronización.

**Rejected alternatives:**
- (a) Tabla global de locks sincronizada con el tracer — requiere tracer cooperation + sincronización adicional; rompe cuando hay pérdida de eventos.
- (b) Modelo basado en happens-before edges explícitos (release/acquire pairs) — pierde eventos que el tracer no emite.
- (c) Vector clocks / Lamport timestamps — overhead y complejidad; útil en sistemas distribuidos, no en single-process tracing.

### Decision D2 — Happens-before via partial order sobre eventos

```rust
// crates/chronos-services/src/concurrency/mod.rs (NEW)
pub struct HappensBeforeGraph {
    events: Vec<TypedConcurrencyEvent>,
    edges: Vec<(EventId, EventId)>,  // partial order
}

impl HappensBeforeGraph {
    pub fn build(events: Vec<TypedConcurrencyEvent>) -> Self { /* ... */ }
    pub fn happens_before(&self, a: EventId, b: EventId) -> bool { /* transitive closure */ }
    pub fn concurrent(&self, a: EventId, b: EventId) -> bool {
        !self.happens_before(a, b) && !self.happens_before(b, a)
    }
}
```

**Rationale:** happens-before clásico (Lamport 1978, "Time, Clocks, and the Ordering of Events") es el estándar para detectar races. La construcción del grafo es O(n²) en el peor caso (transitive closure) pero se amortiza con incremental update: añadir un evento solo re-evalúa edges desde/hacia ese evento, no todo el grafo.

**Rejected alternatives:**
- (a) Full vector clocks — útil para distributed systems, overkill para single-process.
- (b) "Lock-set" detection (Eraser-style) — limitado a locks exclusivamente; pierde atomics y mensajes.
- (c) Predicated execution tracing — depende de LLVM/compiler instrumentation; fuera del alcance de un tracer no-instrumenting.

### Decision D3 — Clasificador suspicious/confirmed/unsupported como state machine explícito

```rust
// crates/chronos-services/src/concurrency/classifier.rs (NEW)
pub enum RaceClassification {
    /// Dos accesos concurrentes sin happens-before suficiente; puede ser race pero falta evidencia.
    Suspicious { reason: SuspiciousReason, evidence_count: usize },
    /// Happens-before prueba ausencia de race (lock visible en ambos lados, message send antes de receive, etc.).
    NotARace { reason: NotARaceReason, evidence: Vec<EventId> },
    /// Happens-before prueba race (sin sync entre los accesos, orden total del programa no lo impide).
    Confirmed { reason: ConfirmedReason, evidence: Vec<EventId> },
    /// No hay suficiente evidencia (eventos perdidos, perturbation, race_depth limitado).
    Unsupported { reason: UnsupportedReason, missing_events: usize },
}
```

**Rationale:** el clasificador debe ser **explícito** y **tri-state** (no binario race/no-race) porque el estado `unsupported` es el más común en práctica (pérdida de eventos sync, sampling, futex opaco). Confundir `unsupported` con `not-a-race` sería un **false negative catastrófico** (silently miss real races); confundirlo con `suspicious` sería un false positive (muchos falsos alarms).

**Rejected alternatives:**
- (a) Binario race/no-race — pierde la distinción crítico.
- (b) Probabilistic score (0..1) — útil para CV, pero pierde la decisión explícita que el agente/developer necesita.
- (c) Single classifier global — no permite clasificar el mismo (event_a, event_b) pair en diferentes contextos (cross-thread vs same-thread-with-sync).

### Decision D4 — Pre-filter via `detect_concurrent_access` (m0-09)

```rust
// crates/chronos-services/src/concurrency/prefilter.rs (NEW)
pub fn prefilter(
    trace_events: &[TraceEvent],
    threshold_ns: u64,
) -> Vec<(EventId, EventId)> {
    // Llama a engine.detect_concurrent_access() (ya existe, m0-09).
    // Retorna pairs sospechosos para análisis profundo.
}
```

**Rationale:** para traces de millones de eventos, ejecutar happens-before sobre todos los pares es prohibitivo. `detect_concurrent_access` (m0-09) reduce el espacio a los pares sospechosos. M9 no reemplaza m0 — lo usa como input.

**Rejected alternative:** ejecutar happens-before sobre todos los pares — O(n²) intractable para traces reales.

### Decision D5 — Wire surface y UAT executors

```rust
// crates/chronos-mcp/src/concurrency_tools.rs (NEW) — wire shape
#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConcurrencyAnalysisRequest {
    pub session_id: String,
    pub address_filter: Option<u64>,
    pub time_range: Option<(TimestampNs, TimestampNs)>,
    pub min_evidence_count: Option<usize>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ConcurrencyAnalysisResponse {
    pub classifications: Vec<RaceClassificationReport>,
    pub graph_stats: GraphStats,
    pub prefilter_pairs: usize,
    pub analyzed_pairs: usize,
}
```

UAT-M9-01 / UAT-M9-02 se ejecutan como **wire smokes** contra este tool (similar a M6/M7 UAT executors).

### Decision D6 — Perturbation handling via explicit `Unsupported` state

Cuando un sync event falta (p. ej. lock acquire no emitido por el tracer), el clasificador **NO infiere** sync. Devuelve `Unsupported { missing_events }` con la lista de eventos esperados. Esto es honestidad: el developer ve "no tengo evidencia, no puedo clasificar" en vez de "todo OK".

**Rationale:** §3 del MILESTONE_ACCEPTANCE — "pérdidas de datos nunca generan confirmación silenciosa". Si el tracer pierde un evento, la respuesta es `unsupported`, no `not-a-race`.

## §5 Sub-cycles (proposed breakdown)

Basado en ROADMAP §M9.1..M9.4 y las decisions D1-D6:

| Sub-cycle | Scope | Deliverables |
|---|---|---|
| **M9.1** (scoping) | ESTE slice | Este documento + ADR-0027 foundation inventory (incluye discovery de `CausalityIndex` 216L + 5 tests pre-existente) |
| **M9.2** | D1: typed model | `chronos-domain::concurrency` module (ConcurrencyPrimitive, Provenance, TypedConcurrencyEvent) + integration con `CausalityIndex` existente + unit tests + UAT-M9 model fixture |
| **M9.3** | D2 + D4: happens-before graph + prefilter integration | `chronos-services::concurrency::HappensBeforeGraph` construido sobre `CausalityIndex` (NO re-implementar tracking de writes) + incremental update + integration con `detect_concurrent_access` + unit tests |
| **M9.4** | D3: classifier + D5 wire tool | `chronos-services::concurrency::classifier` (RaceClassification enum + state machine) + `chronos-mcp::concurrency_tools` (wire shape) + sandbox test fixtures (sync + unsync) + UAT-M9-01 executor |
| **M9.5** | D6: perturbation + missing-evidence handling + UAT-M9-02 | loss injection tests + `Unsupported` state propagation + UAT-M9-02 executor + ADR (M9) |
| **M9.6** (close) | Integration + verification + close report | full T1/T2/T3 + UAT-M9-01/02 GREEN + close report + tag `m9-causal-concurrency.0` |

**Rationale para 4 sub-cycles (M9.2..M9.5):** sigue el patrón m8-01..m8-04 (cada sub-cycle con delivery verificable + tests incrementales). M9.6 es close-of-record (similar a M8-CLOSE). El uso de `CausalityIndex` pre-existente reduce el scope de M9.3 (no re-implementar write tracking) pero no elimina M9.2 (typed model + integration sigue siendo trabajo nuevo).

## §6 UAT mapping

### UAT-M9-01 (lock-protected same-address is NOT confirmed race)

**Wire smoke scenario:** tracer emite 4 eventos: `Lock(0x1000).Acquire(thread=1, t=100ns)`, `Write(0x1000, thread=1, t=200ns)`, `Write(0x1000, thread=1, t=300ns)`, `Lock(0x1000).Release(thread=1, t=400ns)`. Concurrency analysis tool debe clasificar el par (Write@200, Write@300) como **`NotARace`** con razón `SameThreadAfterLock`.

**Acceptance:** JSON response incluye `classifications: [{pair: (Write@200, Write@300), classification: NotARace, reason: SameThreadAfterLock, evidence: [Lock.Acquire@100]}]`.

### UAT-M9-02 (unsynchronized fixture is supported by happens-before evidence + tri-state classification + no silent confirmation)

**Wire smoke scenario:** tracer emite 6 eventos: dos threads (T1, T2) escriben a `0x2000` sin lock. M9 debe:
1. Clasificar el par (Write@T1@100ns, Write@T2@200ns) como `Suspicious` (concurrente, sin happens-before).
2. Si luego tracer emite `Fork(T1, T2, t=50ns)` (T1 forkea T2), reclasificar a `NotARace` (happens-before via fork edge).
3. Si el tracer pierde `Fork` (perturbation), reclasificar a `Unsupported` con `missing_events: [Fork(T1, T2)]` — NO a `NotARace` silenciosamente.

**Acceptance:** JSON response incluye las 3 transiciones + audit log + el caso unsupported tiene `missing_events` populated.

## §7 Risks

- **R1: Tracer coverage gap.** Si el tracer no emite eventos de lock/atomic/task (p. ej. procesos sin USDT), M9 devuelve `Unsupported` para casi todo. **Mitigation:** capability matrix en UAT-M9-02 + docs sobre qué tracers soportan M9.
- **R2: Perturbation underestimate.** Si la heurística prefilter descarta pares legítimos, M9 los pierde. **Mitigation:** configurable `min_evidence_count` en wire surface (default = 1, override = 0 para exhaustive).
- **R3: Graph size blowup.** Happens-before graph puede explotar para traces largos. **Mitigation:** incremental update + pruning de nodos unreachable + cap configurable.
- **R4: Cross-language inconsistency.** Cada lenguaje tiene su modelo de concurrencia (Rust: Send/Sync; Go: goroutines + channels; Python: GIL; JVM: threads + executors). **Mitigation:** scope inicial solo Rust + Go (ya certificados), expansión en M11.
- **R5: Conflict con heurística m0-09.** `detect_concurrent_access` y M9 classifier pueden dar respuestas diferentes. **Mitigation:** M9 supersedes m0-09 cuando ambos están disponibles; m0-09 sigue como prefilter.

## §8 Out-of-scope (M9 chapter)

- **Cross-session causality** (sesión → sesión) → M10.
- **Causal slicing sobre queries específicas** → M10 (execution explorer).
- **Multi-language support beyond Rust + Go** → M11.
- **Probabilistic race detection** → nunca (fuera del scope de causal concurrency).
- **Replace m0-09 `detect_concurrent_access`** → m0-09 sigue como prefilter.
- **Productionize como "race bug detector"** → M9 clasifica, no sentencia; la decisión sigue siendo del developer.

## §9 References

- ROADMAP §M9 §87 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §M9 (`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`).
- UAT_CATALOG.md §M9 (`docs/roadmap/UAT_CATALOG.md`) — UAT-M9-01 + UAT-M9-02.
- ADR-0027 §7 — M9 chapter status declaration (NOT STARTED).
- ADR-0027 §2.3 — naming convention proposal (M9.1 docs-only, M9.2+ execution).
- `crates/chronos-domain/src/index/causality.rs` (216L) — `CausalityIndex` + `CausalityEntry` pre-existente, foundation para M9.3 happens-before graph.
- `crates/chronos-index/src/builder.rs` — `IndexBuilder.causality: CausalityIndex` integration (1 test).
- `crates/chronos-query/src/engine.rs:766` `detect_concurrent_access` — existing prefilter (m0-09, heurística triage, no happens-before).
- `crates/chronos-services/src/debug_trace_specialized.rs:152` `detect_races` MCP tool — wraps prefilter.
- `chronos-sandbox/tests/race_depth.rs` — existing race depth tests.
- Lamport, L. (1978). "Time, Clocks, and the Ordering of Events in a Distributed System". CACM 21(7): 558-565.
- ADR-0004 (no falsear una entrega) — aplicado en §3 + §5 (cada sub-cycle verifica su scope antes de declarar DONE).
