# ADR-0028 — M9 chapter scoping ADR: causal concurrency architecture over CausalityIndex foundation

**Cycle:** M9 / M9.0-prep (causal concurrency — formal scoping ADR before execution)
**Status:** `verified` post-write (docs-only; no code change; branch HEAD == `main @ 19235eb3`)

---

## 1. Context

ROADMAP §M9 §87 — "M9.1 modelo typed de lock/atomic/task/goroutine/message con procedencia; M9.2 happens-before projection incremental/replay; M9.3 clasificador suspicious/confirmed/unsupported; M9.4 fixtures con y sin sincronización, pérdidas de evidencia y perturbación; UAT-M9-01/02."

Tras M9.1 (ADR-0027) se confirmó que ROADMAP §M9 (causal concurrency) **NO se ha ejecutado** en `main`. Ese ADR describió el problema y caracterizó el estado real (incluyendo la confusión de naming entre m9-NN = storage refactor vs M9 = causal concurrency). Quedó pendiente la decisión arquitectónica formal de **cómo construir M9** antes de empezar a escribir código.

**Discovery crítica durante M9 scoping** (`docs/milestones/M9-SCOPING.md`, slice previo commit `f9b1e4df`): `CausalityIndex` (NEW, 216L, 5 unit tests en `crates/chronos-domain/src/index/causality.rs`) **ya existe** como infrastructure de indexación causal en el repo. Es un paso importante porque cambia la pregunta de "¿cómo construimos causal concurrency desde cero?" a "¿cómo construimos causal concurrency SOBRE el foundation existente?".

Este ADR formaliza la decisión arquitectónica de M9, incluyendo qué se construye, qué se reusa, qué NO se hace, y cómo se subdivide en 6 sub-cycles verificables (M9.2..M9.6 + M9.1 inventory + este scoping ADR). Sustituye al M9-SCOPING.md como fuente única de verdad arquitectónica; el milestone scoping queda como referencia operacional con detalles de implementación.

## 2. Decision

M9 (causal concurrency) se construye como **6 sub-cycles verificables** (M9.1 inventory + M9.2..M9.5 execution + M9.6 close), construyendo **sobre** `CausalityIndex` pre-existente (no desde cero).

### §2.1 Foundation pre-existente (reusable)

Tres componentes ya en `main @ 19235eb3`:

1. **`CausalityIndex`** (`crates/chronos-domain/src/index/causality.rs`, 216L, 5 unit tests):
   - `CausalityEntry { event_id, timestamp, thread_id, function: Option<String>, file: Option<String>, line: Option<u32>, value_before: Option<Vec<u8>>, value_after: Option<Vec<u8>> }`.
   - Métodos: `record_write(addr, entry)`, `find_last_mutation(addr)`, `trace_lineage(addr)`, `writes_at(addr)`, `name_to_addr: HashMap<String, u64>`.
   - Tests: `test_record_write_and_find_last_mutation`, `test_trace_lineage_exact_match`, `test_writes_at`, `test_name_to_addr_tracking`, `test_find_last_mutation_unknown_addr`.
   - **Es infraestructura de indexación**, no clasificador de races.

2. **`IndexBuilder.causality`** (`crates/chronos-index/src/builder.rs:13,21,32`, 1 test `test_build_causality_index`):
   - Integra `CausalityIndex` en el pipeline de indexación. Los eventos del trace se insertan automáticamente en el index.

3. **`detect_concurrent_access`** (`crates/chronos-query/src/engine.rs:766`, 1 unit test):
   - Heurística de triage: "two writes to the same address from different threads within threshold_ns".
   - Renombrado de `detect_races` en m0-09 (commit `25f698e8`, 2026-09-08) precisamente para subrayar que NO prueba races.
   - **Doc-string explícito**: "we do not run happens-before analysis, so this is a triage signal, not a verdict".
   - Internamente usa `CausalityIndex` (correlaciona events por address + thread + timestamp).

4. **`detect_races` MCP tool** (`crates/chronos-services/src/debug_trace_specialized.rs:152`):
   - Wrapper sobre `detect_concurrent_access` para exposición MCP.
   - Devuelve JSON con la lista de candidatos; ya clasifica como "triage signal" en su doc-string.

### §2.2 Sub-cycles M9.2..M9.5 execution

Cada sub-cycle entrega capacidad verificable + tests incrementales (per ADR-0004). Patrón seguido: m8-01..m8-06.

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **M9.2** (D1) | `chronos-domain::concurrency` typed model | `ConcurrencyPrimitive` enum (Lock/Atomic/Task/Goroutine/Message); `Provenance` struct (source_runtime, lock_addr, lock_type, thread_id, timestamp); `TypedConcurrencyEvent` struct; conversion helpers desde `CausalityEntry`; integration con `IndexBuilder.causality`. | +6 unit + 2 integration |
| **M9.3** (D2+D4) | Happens-before graph + prefilter integration | `chronos-services::concurrency::HappensBeforeGraph` construido SOBRE `CausalityIndex` (no re-implementar tracking); incremental update path; integration con `detect_concurrent_access` para reducir falsos positivos antes de projection; sandbox test fixture (sync + unsync). | +8 unit + 3 sandbox |
| **M9.4** (D3+D5) | Classifier + MCP wire tool | `chronos-services::concurrency::classifier::RaceClassification` enum (NotARace/Suspicious/Confirmed/Unsupported) + state machine; `chronos-mcp::concurrency_tools` con 3 nuevos tools (`classify_concurrent_access`, `explain_classification`, `list_concurrent_access_candidates`); wire DTOs (serde + JSON schema); UAT-M9-01 executor. | +12 unit + 4 sandbox + 5 MCP |
| **M9.5** (D6) | Perturbation + missing-evidence handling | Loss injection tests (drop events, drop timestamps, drop thread_ids); `Unsupported` state propagation (no crash, no false classification); UAT-M9-02 executor (unsynchronized + Fork reclassification + Unsupported on perturbation); ADR (M9). | +6 unit + 3 sandbox + 2 UAT |
| **M9.6** (close) | Integration + close report | full T1/T2/T3 sobre `main`; close report `docs/milestones/M9-CLOSE.md`; tag `m9-causal-concurrency.0`; actualizar STATE + JOURNAL + ROADMAP. | T1+T2+T3 |

### §2.3 Naming convention (re-confirmada)

Tras M9.1 ADR-0027 §2.3 y este ADR §2.2:

- **Vault cycles / storage refactors** futuros usan prefijo `cc-m9-NN` o `vault-m9-NN` (no `feat(m9-NN)`).
- **ROADMAP §M9 sub-cycles** usan prefijo `M9.N` (capital M + dot, matching M7/M8).
- **M9.1 inventory + este ADR** son docs-only sin tag.
- **M9.2..M9.5** son sub-cycles de ejecución (uno por slice, con tag `m9-causal-concurrency.0` único al cierre M9.6).

## 3. Alternatives considered

Seis alternativas consideradas; una aceptada, cinco rechazadas.

### §3.1 Construir happens-before graph desde cero (rechazado)

Ignorar `CausalityIndex` pre-existente; implementar un `VectorClock` + `LamportTime` en un módulo nuevo. **Por qué rechazada**: (a) duplica trabajo ya hecho (216L de código + 5 tests); (b) introduce dos sistemas de tracking causal en el repo (confusión operacional); (c) rompe el invariante de ROADMAP §0.4 (no inventar abstracciones si las existentes sirven). ADR-0004 refuerza: "no falsear una entrega" — usar CausalityIndex es honestidad sobre el foundation.

### §3.2 Replace `detect_concurrent_access` con happens-before (rechazado)

Eliminar la heurística de triage y reemplazarla completamente con el grafo de happens-before. **Por qué rechazada**: (a) M9.3 debe **reducir falsos positivos**, no reemplazar; (b) la heurística tiene 1 unit test + sandbox test (M9.3 puede agregar más); (c) `detect_concurrent_access` es cheap O(n) sobre el index, happens-before projection es O(n²) en el peor caso (pequeños N); mantener prefilter reduce coste.

### §3.3 Inventariar y NO ejecutar (rechazado)

Limitarse a M9.1 (inventory) sin planificar sub-cycles de ejecución. **Por qué rechazada**: (a) ROADMAP §M9 §87 es **capacidad adoptada** que debe completarse; (b) AGENTS.md §1 dice "trabaja hasta completar todas las capacidades adoptadas del roadmap"; (c) el inventario sin plan es ops-admin, no entrega. M9.1 + este ADR ya cumplen el rol de inventario; M9.2..M9.6 son la entrega.

### §3.4 Skip discovery y ejecutar M9.2 directamente (rechazado)

Asumir que no hay foundation y arrancar a programar `chronos-domain::concurrency`. **Por qué rechazada**: (a) habría reinventado `CausalityIndex` (216L); (b) habría duplicado tracking causal; (c) viola ADR-0004 (entrega falsa: "construí causal concurrency" cuando realmente duplicaste algo existente).

### §3.5 Multi-language beyond Rust+Go (rechazado)

Soportar primitivos de Python (asyncio locks), Java (synchronized/ReentrantLock), Erlang (processes), etc. **Por qué rechazada**: (a) ROADMAP §M9 §87 menciona "lock/atomic/task/goroutine/message" — task/goroutine son Rust+Go; (b) cross-runtime primitives son M11 (out-of-scope); (c) añadir más lenguajes expande la matriz de tests sin valor inmediato.

### §3.6 Build on `CausalityIndex` + 6 sub-cycles verificables (aceptado)

Opción adoptada. Justificación:

1. **Honesta**: reconoce foundation pre-existente, no lo reinventa.
2. **Incremental**: cada sub-cycle entrega capacidad verificable (per ADR-0004).
3. **Composable**: M9.3 depends-on M9.2 (typed model); M9.4 depends-on M9.3 (graph); M9.5 depends-on M9.4 (classifier); M9.6 depends-on all (integration).
4. **Risk-managed**: prefilter reduce coste de happens-before; CausalityIndex reduce duplicación.
5. **UAT-aligned**: UAT-M9-01 (lock-protected) executable en M9.4; UAT-M9-02 (unsync + perturbation + Unsupported) executable en M9.5.

## 4. Consequences

### §4.1 Positive

- **Scope reducido por foundation**: 4 sub-cycles (M9.2..M9.5) en lugar de 5+ porque CausalityIndex cubre write tracking.
- **Menos código nuevo**: ~2,000 LoC estimados (vs ~3,000 si duplicara CausalityIndex).
- **Más tests por menos código**: ratio tests/LoC > 1.0 mantenido (m8-04 fue 0.5 por integración externa).
- **Compatible con m0-09 forward path**: `detect_concurrent_access` sigue siendo el prefilter; M9.3 lo mejora, no lo reemplaza.
- **Per-provenance**: UAT-M9-01/02 ejercitan clasificación con procedencia (lock_addr, thread_id, timestamp), no solo "race detected".

### §4.2 Negative

- **Dependencia en `CausalityIndex` contrato**: si en el futuro `causality.rs` cambia su API, M9.3+ pueden romperse. Mitigación: M9.3 wrapper + integration tests + 1 contract test pinning API.
- **CausalityIndex no expone threading primitives**: `CausalityEntry` tiene `thread_id` pero no el **tipo de primitivo** (lock vs atomic). Mitigación: M9.2 introduce `ConcurrencyPrimitive` como type discriminator; el wrapper convierte desde `CausalityEntry` + el contexto del trace.
- **`detect_concurrent_access` threshold_ns es heurístico**: 100ns por defecto; configurable pero no validado empíricamente. Mitigación: M9.5 perturbation tests pueden medir sensibilidad al threshold.
- **M9.4 wire shape nuevo**: introduce 3 nuevos MCP tools; los consumers existentes pueden romperse si esperan API v2. Mitigación: versioning de tools + JSON schema + compatibility tests.

### §4.3 Neutral

- **6 sub-cycles = 6 commits** en main (uno por slice) + 1 commit de close = 7 commits totales M9.x.
- **Tests sandbox crecen** (M9.3 +3, M9.4 +4, M9.5 +3 = +10 sandbox tests). Sandbox total pre-M9: ~80 tests. Post-M9: ~90.
- **ROADMAP §M9 §87 NO se modifica**: este ADR ejecuta lo que ya está descrito. Si M9.x encuentra que la especificación es incompleta, se delega un refinement ADR (similar a M6 §0.4).

## 5. Verification evidence

Inspección directa sobre `main @ 19235eb3` (post-M9 scoping commit `f9b1e4df` + docs-align `19235eb3`):

- **T0** `cargo clippy --workspace --all-targets --no-deps -- -D warnings` exit=0 (no se tocó código, ADR es docs-only).
- **T1** `cargo test -p chronos-mcp --lib --no-fail-fast` 84/84 PASS en 0.88s (no regresión post-ADR-0028).
- **`git cat-file -e f9b1e4df + 19235eb3`** exit=0; M9-SCOPING.md + STATE.md + JOURNAL.md commits accesibles.
- **CausalityIndex API verificada**: 5 tests en `causality.rs:126,154,176,192,210` (test_record_write_and_find_last_mutation + test_trace_lineage_exact_match + test_writes_at + test_name_to_addr_tracking + test_find_last_mutation_unknown_addr).
- **IndexBuilder.causality integration verificada**: `crates/chronos-index/src/builder.rs:13,21,32` + 1 test `test_build_causality_index`.
- **detect_concurrent_access verificada**: `crates/chronos-query/src/engine.rs:766` + 1 test `test_detect_concurrent_access_100ns_threshold` en `engine.rs:1363`.
- **detect_races MCP tool verificada**: `crates/chronos-services/src/debug_trace_specialized.rs:152`.

## 6. Mapping to UAT

| UAT | Source | Sub-cycle |
|---|---|---|
| UAT-M9-01 (lock-protected same-address → NotARace) | UAT_CATALOG.md §M9 + MILESTONE_ACCEPTANCE.md §M9 | M9.4 (classifier + wire tool + executor) |
| UAT-M9-02 (unsynchronized + Fork reclassification + Unsupported on perturbation) | UAT_CATALOG.md §M9 + MILESTONE_ACCEPTANCE.md §M9 | M9.5 (perturbation + executor) |

UAT-M9-01 y UAT-M9-02 son los únicos dos UAT catalogados para §M9. Ambos son end-to-end: ingieren trace con concurrencia + primitivos sincronizados/NO + perturbation, esperan clasificación binaria (race/no-race) + report de incertidumbre (Unsupported).

## 7. M9 chapter status

- **M9.1**: `verified` (ADR-0027, este ADR-0028 lo referencia).
- **M9 scoping (este ADR-0028)**: `verified` post-write (este slice).
- **M9.2..M9.6**: NOT STARTED en `main @ 19235eb3`. Listos para ejecución tras OK operador.

**Post-condición de M9 chapter CLOSED** (M9.6 close):
- T1+T2+T3 verdes sobre `main`.
- UAT-M9-01 + UAT-M9-02 PASS (executable evidence en `evidence/m9/`).
- Close report `docs/milestones/M9-CLOSE.md`.
- Tag `m9-causal-concurrency.0` firmado apuntando al merge commit.
- ROADMAP §M9 §87 con check mark de cierre + ref a M9-CLOSE.
- STATE.md con sub-cycle rows prepended + "M9 chapter CLOSED (6/6)".

## 8. Out-of-scope (M9 chapter)

1. **Cross-session causality** (M10 — eventos a través de procesos/distribuidos, requiere consensus protocol out-of-band).
2. **Multi-language beyond Rust+Go** (M11 — primitives Python asyncio/Java synchronized/Erlang processes/etc.).
3. **Probabilistic race detection** (never — sería false positive por diseño; race es binario o Unsupported).
4. **Replace m0-09 prefilter** (m9-02..m9-05 storage refactors + m0-09 detect_concurrent_access se mantienen).
5. **Causal-clock protocols** (Lamport/Vector clocks para multi-host; son protocol-level, M9 es single-trace analysis).
6. **Lock-set inference** (requeriría inter-procedural analysis + alias analysis; es trabajo de compilador, no de trace analysis).
7. **Per-instruction memory model** (x86 TSO, ARM weak, etc. — M9 es operativo sobre traces, no modela hardware).
8. **Cross-trace correlation** (M9 es per-trace; cross-trace stitching es M10+).

## 9. References

- ROADMAP §M9 §87 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §M9 (`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`).
- UAT_CATALOG.md §M9 (`docs/roadmap/UAT_CATALOG.md`) — UAT-M9-01 + UAT-M9-02.
- ADR-0027 §7 — M9 chapter status declaration (NOT STARTED + storage scope).
- ADR-0027 §2.3 — naming convention proposal.
- `docs/milestones/M9-SCOPING.md` (237L, commit `f9b1e4df`) — operacional reference.
- `crates/chronos-domain/src/index/causality.rs` (216L, 5 tests) — `CausalityIndex` foundation.
- `crates/chronos-index/src/builder.rs:13,21,32` — `IndexBuilder.causality` integration.
- `crates/chronos-query/src/engine.rs:766` — `detect_concurrent_access` prefilter (heurística triage).
- `crates/chronos-query/src/engine.rs:1363` — `test_detect_concurrent_access_100ns_threshold`.
- `crates/chronos-services/src/debug_trace_specialized.rs:152` — `detect_races` MCP tool.
- `chronos-sandbox/tests/race_depth.rs` — existing race depth sandbox tests.
- m8-counterexample-shrinking-scoping.md (199L) — pattern reference for M9 scoping style.
- Lamport, L. (1978). "Time, Clocks, and the Ordering of Events in a Distributed System". CACM 21(7): 558-565.
- ADR-0004 (no falsear una entrega) — aplicado en §2 + §3 (cada sub-cycle verifica su scope antes de declarar DONE).
