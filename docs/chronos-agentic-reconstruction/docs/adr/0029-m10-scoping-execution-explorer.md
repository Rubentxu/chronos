# ADR-0029 — M10 chapter scoping ADR: Execution Explorer architecture over EventsCursorV1 + REC-C1/C2 foundation

**Cycle:** M10 / M10.0-prep (Execution Explorer — formal scoping ADR before execution)
**Status:** `verified` post-write (docs-only; no code change; branch HEAD == `main @ a315fa1d`)

---

## 1. Context

ROADMAP §M10 §95 — "M10.1 contrato de lectura/paginación y permisos; M10.2 live/evidence/provenance; M10.3 causality/mutation/properties/compare cuando cada fuente se haya certificada; M10.4 virtualización de trazas grandes; M10.5 accesibilidad y validación UX/seguridad; UAT-M10-01/02."

Tras M9.1 (ADR-0027) + M9 scoping (ADR-0028) + M9 ROADMAP refactor (slice docs-only), M9 está **completamente preparado** para execute (foundation CausalityIndex + 5 architecture decisions + 6 sub-cycles definidos). M10 (Execution Explorer) es el siguiente capítulo del ROADMAP.

A diferencia de M9 (donde ROADMAP §M9 §91 indicaba "M9.1..M9.4" sin más contexto), ROADMAP §M10 §95 menciona 5 sub-cycles pero NO hay scoping formal. Hay un **product design** (EXECUTION_EXPLORER.md, 39L, 7 vistas) + **REC-C1/REC-C2 acceptance criteria** (8 UATs en MILESTONE_ACCEPTANCE.md §99-§124) + **foundation de cursor + read services masiva** (no inspeccionada en sesión previa).

Este ADR formaliza la decisión arquitectónica de M10, incluyendo qué se construye (consolidación de wire shape + permissions + live streaming + virtualization + a11y/UX), qué se reusa (EventsCursorV1 + read services + REC UATs), qué NO se hace, y cómo se subdivide en 6 sub-cycles verificables (M10.1 inventory + M10.2..M10.5 execution + M10.6 close).

## 2. Decision

M10 (Execution Explorer) se construye como **6 sub-cycles verificables** (M10.1 inventory + M10.2..M10.5 execution + M10.6 close), consolidando **sobre** la foundation pre-existente de cursor + read services (no desde cero).

### §2.1 Foundation pre-existente (reusable)

Cinco componentes en `main @ a315fa1d`, totalizando **3,662 LoC + 54 unit tests**:

1. **`EventsCursorV1`** (`crates/chronos-services/src/events_cursor.rs`, 386L, 13 unit tests):
   - REC-C1.1 authoritative cursor. Value type `(schema_version: u16, session_id: SessionId, next_seq: EventSeq)`.
   - Opaque on wire via `encode()` (prefix `ecv1`); rechaza offsets/timestamps/filters (deliberate).
   - Errores tipados: `Malformed`, `UnsupportedVersion`, `WrongSession`.
   - API: `start(session_id)`, `advanced_to(next_seq) -> Result<Self, _>`, `encode()`, `decode()`.
   - Monotonic invariant: `new.next_seq >= old.next_seq` (re-checkpointing idempotente, no artificial error).

2. **`events_log_read::read_page`** (`crates/chronos-services/src/events_log_read.rs`, 1507L, 30 unit tests):
   - `read_page(log, cursor, limit, filters) -> (page, next_cursor)`.
   - Cursor advance idempotente; filtros aplicados DESPUÉS (no afectan posición).
   - Production-grade: cursor stale detection, gap detection, fail-closed semantics (no Silent Lies per ADR-0004).

3. **`CanonicalDrainPage`** (`crates/chronos-services/src/canonical_drain.rs`, 737L, 11 unit tests):
   - `read_canonical_drain_page` streaming canónico.
   - Constantes: `DEFAULT_MAX_RAW_EVENTS=512`, `DEFAULT_MAX_EXAMINED_RECORDS=50_000`, `DEFAULT_MAX_DERIVED_PER_SOURCE=4_096`.

4. **`ChronosEventsReadService`** (`crates/chronos-services/src/events_read.rs`, 426L):
   - REC-C1.3 path: v2 `events_read` dispatcher para agent-visible event reads.
   - Mode=Query (v1 `query_events`) + Mode=ById (v1 `get_event`).
   - Completeness reportada como "unknown" hasta REC-C1.4 (nunca "complete" — Silent Lie prevention).

5. **`DebugReadService`** (`crates/chronos-services/src/debug_read.rs`, 606L):
   - 7 métodos read-only: `evaluate_expression`, `get_scope_variables`, `get_audit_entry`, `get_memory_access`, `get_register_read`, `get_state_diff`, `get_variable_change`.
   - Mutex held only for sync call duration (latency + contention optimizados).

### §2.2 REC-C1/REC-C2 acceptance criteria (regression foundation)

`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` §99-§124 define 8 UATs:

- **UAT-REC-C1-01 (two independent consumers)**: readers independientes no se pisan.
- **UAT-REC-C1-02 (real cursor semantics)**: no offset/limit leak; cursor es position-in-log.
- **UAT-REC-C1-03 (gap truth)**: gaps reportados honestamente, no llenados.
- **UAT-REC-C1-04 (cursor invalid/stale)**: errores tipados correctos.
- **UAT-REC-C1-05 (time semantics)**: timestamps monotónicos.
- **UAT-REC-C2-01 (EventBus cannot steal evidence)**: aislamiento.
- **UAT-REC-C2-02 (tripwire replayable)**: replay-safe.
- **UAT-REC-C2-03 (no dual-truth divergence)**: single source of truth.

Estas 8 UATs son acceptance criteria del foundation M10 pre-existente. M10.4 las corre como **regression suite** (no duplica).

### §2.3 Sub-cycles M10.2..M10.5 execution

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **M10.2** | Wire shape unificado + permissions | `chronos-mcp::execution_explorer` module consolidando `read_page` + `read_canonical_drain_page` + `events_read` en 1 contrato MCP unificado (v3); `Permissions` enum (`ReadEvents`/`ReadDebug`/`ReadCompare`); deny-by-default + per-tool checks; sandbox test con permission denial scenarios. | +8 unit + 4 sandbox |
| **M10.3** | Live streaming + causality wiring | Live event stream push-based sobre `EventsCursorV1` + `session_log::SessionExecutionLogRegistry` (sin polling); integration con `detect_concurrent_access` (M9) vía stub "Unsupported until M9 certified"; integration con `session_fingerprint` (M7.3) + `align_sessions` (M7.2) — ambos ya CLOSED. | +6 unit + 3 sandbox + 2 integration |
| **M10.4** | Virtualization + REC regression | Aggregation layers (time-bucketed summaries + per-invocation rollups); threshold switch page↔summary (default 100_000, env `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`); sandbox test con trazas de 1M eventos; **REC-C1/REC-C2 regression suite** (8 UATs ejecutables). | +8 unit + 3 sandbox + 8 REC regression |
| **M10.5** | a11y + UX validation + UAT-M10-01/02 | JSON Schema validation de wire shapes (sentando bases para GUI a11y-compliant futura); error message consistency across read services; UAT-M10-01 executor (permission denial + cursor monotonicity); UAT-M10-02 executor (live stream catches new events + summary fires at threshold). | +5 unit + 2 UAT |
| **M10.6** (close) | Integration + close report | full T1/T2/T3 sobre `main`; close report `docs/milestones/M10-CLOSE.md`; tag `m10-execution-explorer.0`; ROADMAP §M10 §95 check + STATE + JOURNAL actualizados. | T1+T2+T3 |

### §2.4 Naming convention (re-confirmada)

Tras M9.1 ADR-0027 §2.3 + ADR-0028 §2.3:

- **Vault cycles / storage refactors** futuros usan prefijo `cc-m9-NN` o `vault-m9-NN`.
- **ROADMAP §M9 sub-cycles** usan prefijo `M9.N`.
- **ROADMAP §M10 sub-cycles** usan prefijo `M10.N` (sigue convención M7/M8/M9).
- **M10.1 inventory + este ADR** son docs-only sin tag.
- **M10.2..M10.5** son sub-cycles de ejecución (uno por slice, con tag `m10-execution-explorer.0` único al cierre M10.6).

## 3. Alternatives considered

Seis alternativas consideradas; una aceptada, cinco rechazadas.

### §3.1 Construir pagination desde cero (rechazado)

Ignorar `EventsCursorV1` + `read_page` + `CanonicalDrainPage`; implementar offset/limit. **Por qué rechazada**: (a) duplica 2,630 LoC de código production-grade + 54 tests; (b) `chronos-sandbox/tests/rec_c1_characterization.rs` ya documenta que el legacy `query_events` ignora `offset` (anti-pattern que queremos dejar atrás); (c) `EventsCursorV1` fue diseñado precisamente para evitar offset/limit (REC-C1.1 doc: "position in an authoritative log, not a serialized query"); (d) rompe el invariante de ROADMAP §0.4 (no inventar abstracciones si las existentes sirven).

### §3.2 Reemplazar DebugReadService con nueva API (rechazado)

Eliminar DebugReadService (606L) y re-implementar con API moderna. **Por qué rechazada**: (a) ya está production-grade; (b) consume `QueryEngine` directamente (tightly-coupled pero funcional); (c) 7 métodos read-only cubren los casos de uso de EXECUTION_EXPLORER.md §Views (Evidence inspector + Hypotheses + Compare); (d) M10.2 envuelve, NO reemplaza.

### §3.3 Multi-tenant ABAC permissions (rechazado)

Construir ABAC/RBAC completo para Execution Explorer en M10.2. **Por qué rechazada**: (a) ROADMAP §M10 §95 menciona "permisos" pero es single-user scope; (b) multi-tenant es M11+ (M11.1 prioriza nuevos lenguajes, no multi-user); (c) Permissions enum (D3) es mínimo viable honesto; (d) ABAC prematuro = over-engineering (ADR-0004).

### §3.4 Build GUI implementation (rechazado para M10 chapter)

Construir la GUI del Execution Explorer en M10.5. **Por qué rechazada**: (a) ROADMAP §M10 §95 scope es API + UX validation, no GUI; (b) EXECUTION_EXPLORER.md es product vision, no implementación; (c) GUI consume wire shapes — sentamos bases via JSON Schema validation (D5); (d) GUI es scope futuro post-M10 closed.

### §3.5 Skip REC-C1/REC-C2 regression (rechazado)

No correr las 8 UATs de REC como regression en M10.4. **Por qué rechazada**: (a) REC-C1/REC-C2 son acceptance criteria del foundation M10 (D1); (b) skip = Silent Lie (pretender M10 funciona sin probar el foundation); (c) tests ya existen (54 unit) pero las 8 UATs end-to-end no se ejecutan automáticamente; (d) M10.4 las integra a CI.

### §3.6 Consolidate over foundation + 5 execution sub-cycles + 1 close (aceptado)

Opción adoptada. Justificación:

1. **Honesta**: reconoce foundation 3,662 LoC + 54 tests, no la reinventa.
2. **Incremental**: cada sub-cycle entrega capacidad verificable (per ADR-0004).
3. **Composable**: M10.3 depends-on M10.2 (wire shape); M10.4 depends-on M10.2 + M10.3; M10.5 depends-on M10.2 + M10.3 + M10.4; M10.6 depends-on all.
4. **Risk-managed**: stub explícito para M9 dependency (D2) + REC regression (D1) + env-overridable threshold (D4).
5. **UAT-aligned**: UAT-M10-01/02 ejecutables en M10.5; REC-C1/REC-C2 ejecutables como regression en M10.4.

## 4. Consequences

### §4.1 Positive

- **Scope reducido por foundation masiva**: ~3,662 LoC pre-existentes cubren M10.1 (cursor contract) + M10.2 (read paths partial) + M10.4 (REC acceptance). 5 sub-cycles execution vs ~7 si construyéramos desde cero.
- **Menos código nuevo**: ~1,500 LoC estimados para M10.2..M10.5 (vs ~5,000+ desde cero).
- **Más tests por menos código**: ratio tests/LoC > 0.6 mantenido (m8-04 fue 0.5 por integración externa).
- **UAT-M10-01/02 nuevos** + **REC-C1/REC-C2 existentes** = coverage integral sin duplicación.
- **Compatible con M9 forward path**: M10.3 stub "Unsupported until M9 certified" mantiene honesty; integración real cuando M9.6 esté CLOSED.

### §4.2 Negative

- **Dependencia en `EventsCursorV1` contrato**: si cambia la API, M10.2..M10.5 pueden romperse. Mitigación: 13 unit tests + REC-C1 regression + bump schema version = `v2` cuando evolucione.
- **Permission model coarse**: `ReadEvents`/`ReadDebug`/`ReadCompare` puede no cubrir todos los casos. Mitigación: mínimo viable honesto (D3) + evolución a PermissionSet en M11+.
- **Threshold heurístico**: 100_000 eventos default puede ser poco/mucho. Mitigación: env-overridable + multi-threshold tests (1K, 10K, 100K, 1M).
- **Wire shape v3 puede romper consumers v2**: consolidar 3 read paths en 1 MCP tool cambia contrato visible. Mitigación: v2 deprecado pero soportado + JSON Schema strict + compatibility tests.
- **M10.3 causality stub**: si M9 tarda en cerrar, M10.3 queda con "Unsupported" en la integration. Mitigación: stub explícito (D2) + ADR-0004 honest reporting.

### §4.3 Neutral

- **6 sub-cycles = 6 commits** en main (uno por slice) + 1 commit de close = 7 commits totales M10.x.
- **Tests sandbox crecen** (M10.2 +4, M10.3 +3, M10.4 +3 = +10 sandbox tests). Sandbox total pre-M10: ~90 tests. Post-M10: ~100.
- **ROADMAP §M10 §95 NO se modifica**: este ADR ejecuta lo que ya está descrito. Si M10.x encuentra especificación incompleta, refinement ADR (similar a M6 §0.4).
- **8 REC UATs ejecutables como regression**: tests integration no-unit; corre en CI completo, no en T1 subset.

## 5. Verification evidence

Inspección directa sobre `main @ a315fa1d` (post-M10-SCOPING commit):

- **T0** `cargo clippy --workspace --all-targets --no-deps -- -D warnings` exit=0 (no se tocó código, ADR es docs-only).
- **T1** `cargo test -p chronos-mcp --lib --no-fail-fast` 84/84 PASS en 0.88s (no regresión post-ADR-0029).
- **`git cat-file -e a315fa1d + 678827e9 + 5951ec0d`** exit=0; SHAs accesibles.
- **EventsCursorV1 verificada**: 386L + 13 tests (grep `\[test\]` count) en `events_cursor.rs`.
- **events_log_read verificada**: 1507L + 30 tests en `events_log_read.rs`.
- **canonical_drain verificada**: 737L + 11 tests en `canonical_drain.rs`.
- **events_read verificada**: 426L en `events_read.rs` (REC-C1.3 dispatcher).
- **debug_read verificada**: 606L + 7 métodos read-only en `debug_read.rs`.
- **REC-C1/REC-C2 UATs verificadas**: MILESTONE_ACCEPTANCE.md §99-§124 = 8 UATs.
- **EXECUTION_EXPLORER.md verificada**: 39L, 7 vistas (Live + Execution + Causality + Mutation Lens + Hypotheses/Properties + Compare + Evidence inspector).

## 6. Mapping to UAT

| UAT | Source | Sub-cycle |
|---|---|---|
| UAT-M10-01 (permission denial + cursor monotonicity) | New, ROADMAP §M10 §95 | M10.5 |
| UAT-M10-02 (live stream + summary threshold) | New, ROADMAP §M10 §95 | M10.5 |
| UAT-REC-C1-01..05 (cursor semantics + gap truth + isolation + replay + single-truth) | MILESTONE_ACCEPTANCE.md §99-§116 | M10.4 regression |
| UAT-REC-C2-01..03 (EventBus isolation + replay safety + no dual-truth) | MILESTONE_ACCEPTANCE.md §118-§124 | M10.4 regression |

UAT-M10-01 cubre M10.2 (permissions) + M10.1 (cursor monotonicity via `EventsCursorV1::advanced_to`). UAT-M10-02 cubre M10.3 (live streaming) + M10.4 (summary threshold). REC-C1/REC-C2 son regression suite que valida que el foundation pre-existente sigue honrando acceptance criteria.

## 7. M10 chapter status

- **M10.1**: `verified` (M10-SCOPING.md slice previo + este ADR-0029).
- **M10.2..M10.5**: NOT STARTED en `main @ a315fa1d`. Listos para ejecución tras OK operador.
- **M10.6**: NOT STARTED (close-of-record condicional).

**Post-condición de M10 chapter CLOSED** (M10.6 close):
- T1+T2+T3 verdes sobre `main`.
- UAT-M10-01 + UAT-M10-02 PASS (executable evidence en `evidence/m10/`).
- REC-C1/REC-C2 regression suite PASS (8 UATs).
- Close report `docs/milestones/M10-CLOSE.md`.
- Tag `m10-execution-explorer.0` firmado apuntando al merge commit.
- ROADMAP §M10 §95 con check mark de cierre + ref a M10-CLOSE.
- STATE.md con sub-cycle rows prepended + "M10 chapter CLOSED (6/6)".

## 8. Out-of-scope (M10 chapter)

1. **GUI implementation** (M10 sienta la API; GUI es scope futuro post-M10 closed).
2. **Multi-tenant ABAC permissions** (M11+).
3. **Cross-trace comparison** (M10 cubre single-trace Compare; cross-trace stitching es scope futuro).
4. **Replace `EventsCursorV1`** (REC-C1.1 es foundation; no se reemplaza).
5. **Replace `events_log_read`** (production-grade 1507L + 30 tests; no se reemplaza).
6. **Replace `DebugReadService`** (production-grade 606L; se mantiene y se envuelve).
7. **Probabilistic event prediction** (nunca; los eventos son facts del trace).
8. **Browser-side rendering** (frontend scope, no API).

## 9. References

- ROADMAP §M10 §95 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §99-§124 (`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`) — REC-C1/REC-C2 UATs.
- UAT_CATALOG.md §M10 (`docs/roadmap/UAT_CATALOG.md`) — UAT-M10-01/02 (a crear en M10.5).
- ADR-0027 §7 — M9 chapter status (NOT STARTED execution).
- ADR-0028 §2.2 — M9 sub-cycles (peer reference).
- `docs/milestones/M10-SCOPING.md` (171L, commit `a315fa1d`) — operacional reference.
- `docs/chronos-agentic-reconstruction/docs/gui/EXECUTION_EXPLORER.md` (39L) — product design (7 views).
- `crates/chronos-services/src/events_cursor.rs` (386L, 13 tests) — `EventsCursorV1` cursor foundation (REC-C1.1).
- `crates/chronos-services/src/events_log_read.rs` (1507L, 30 tests) — `read_page` paginación (REC-C1.2).
- `crates/chronos-services/src/canonical_drain.rs` (737L, 11 tests) — `CanonicalDrainPage` streaming canónico.
- `crates/chronos-services/src/events_read.rs` (426L) — `ChronosEventsReadService` v2 events_read dispatcher (REC-C1.3).
- `crates/chronos-services/src/debug_read.rs` (606L) — `DebugReadService` 7 read-only methods.
- `docs/chronos-agentic-reconstruction/docs/testing/ARCHITECTURE_FITNESS_FUNCTIONS.md` — REC-C1/REC-C2 acceptance criteria reference.
- ADR-0004 (no falsear una entrega) — aplicado en §3 + §4 (cada sub-cycle verifica su scope antes de declarar DONE).

---

## 10. Corrección posterior (2026-10-03): D4 y el threshold por tamaño quedan SUPERADOS

> **Esta sección no borra nada de §§2.3, §3.6, §4.2, §6. El texto original se conserva palabra por
> word** para que se vea qué se afirmó y por qué se retira. Quien lea §2.3 fila M10.4 y §4.2 debe
> leer esta sección junto a él.
>
> **Estado: las dos afirmaciones siguientes son `documentadas` pero `falsas` respecto al código.
> Nada se ha integrado ni certificado aquí; esto es una corrección documental, verificada por
> lectura y por búsqueda sobre `main @ 3aa99c0b`.**

### §10.1 Afirmación retirada #1 — el override por despliegue (env var que no existe)

**Afirmación original retirada**, conservada arriba en:

- **§2.3, fila M10.4 (línea 72)**: *"threshold switch page↔summary (default 100_000, env
  `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`)"*.
- **§3.6, punto 4 (línea 117)**: *"env-overridable threshold (D4)"*.
- **§4.2, "Threshold heurístico" (línea 134)**: *"Mitigación: env-overridable + multi-threshold
  tests (1K, 10K, 100K, 1M)"*.

**Motivo de la retirada**: `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD` **no se lee en ninguna parte del
código**. No es una variable configurable: el nombre sólo aparece en prosa.

**Evidencia (verificada por búsqueda el 2026-10-03 sobre `main @ 3aa99c0b`)**:

- Las **únicas 3 apariciones en código Rust** son comentarios de documentación, ninguno una
  lectura: `crates/chronos-services/src/virtualization.rs:22` (`//!` de módulo),
  `crates/chronos-services/src/virtualization.rs:64` (doc comment de la constante) y
  `crates/chronos-mcp/src/tools_params.rs:391` (doc comment del ancho de cubo por defecto).
- La constante real es `DEFAULT_VIRTUALIZATION_THRESHOLD: u64 = 100_000` en
  `crates/chronos-services/src/virtualization.rs:65`. **No es configurable**: sus únicas
  referencias en todo el workspace son su propia definición (`:65`) y **una** aserción de test
  (`crates/chronos-services/src/virtualization.rs:405`).
- La ausencia no es una convención del repo: `env::var` se usa en **34 ocurrencias repartidas en 14
  ficheros** del workspace. Es simplemente que esta variable no está entre ellas.

### §10.2 Afirmación retirada #2 — el switch page↔summary por tamaño NO está cableado (más grave)

**Afirmación original retirada**, conservada arriba en:

- **§2.3, fila M10.4 (línea 72)**: *"threshold switch page↔summary (default 100_000, …)"*.
- **§4.2, "Threshold heurístico" (línea 134)**: *"100_000 eventos default puede ser poco/mucho"*.
- **§6 (líneas 165, 169)**: *"UAT-M10-02 (live stream + summary threshold)"* y *"UAT-M10-02 … cubre
  M10.4 (summary threshold)"*.

**Motivo de la retirada**: el comportamiento descrito —que una sesión con más de N eventos haga que
el read path devuelva un resumen en vez de paginar— **no existe en el read path**.

**Evidencia (verificada por lectura el 2026-10-03 sobre `main @ 3aa99c0b`)**:

- `should_summarize` es una función **pura que recibe `threshold` como parámetro**
  (`crates/chronos-services/src/virtualization.rs:71`, cuerpo: `event_count > threshold`). **No tiene
  ningún call site de producción.** Sus referencias son su definición, los doc comments del mismo
  fichero (`:47`, `:48`, `:201`), los tests del mismo fichero (`:384`–`:401`) y **una mención en
  prosa** en `crates/chronos-services/src/read_path.rs:178` que justamente **difiere la decisión al
  llamador**: *"the caller owns the `should_summarize` threshold decision"*. Es API pública
  (`pub mod virtualization`, `crates/chronos-services/src/lib.rs:148`) pero **no invocada**.
- El handler MCP `execution_log_read` enruta **exclusivamente sobre el discriminante `mode`**
  (`crates/chronos-mcp/src/server.rs:3654`, `match params.mode`), con cuatro brazos: `Poll`
  (`server.rs:3655`), `Summarize` (`server.rs:3681`), `Rollup` (`server.rs:3700`) y `Causality`
  (`server.rs:3714`). El discriminante es `ExecutionLogReadKind`
  (`crates/chronos-mcp/src/tools_params.rs:478`-`487`). **En ningún punto se compara `event_count`
  contra el umbral.** `event_count` aparece una sola vez, y sólo como **salida reportada** del brazo
  `poll` (`crates/chronos-mcp/src/server.rs:3676`, `"event_count": events.len()`), nunca como
  entrada de una decisión.
- La única validación de tamaño que hace el handler es un rechazo de `bucket_size_ns == 0`
  (`server.rs:3682`-`3686`), que no es el guard de tamaño descrito.

**Matiz que importa, y que evita el falsehood simétrico**: **no todo esto está ausente.** El
discriminante `mode` **sí** está cableado y **sí** tiene implementaciones de producción reales:
`poll_batch` lee **una página acotada** (`crates/chronos-services/src/read_path.rs:155`-`173`),
`Summarize` y `Rollup` agregan el log real (`read_path.rs:179`-`197`). Lo que **no** existe es la
**decisión automática** de cambiar de paginación a resumen en función del tamaño de la sesión. La
afirmación retirada es esa decisión, no la existencia del read path ni de las vistas.

### §10.3 Por qué se retiran y no se "arreglan": D1 + D2 ya pronunciaron la decisión

Este hallazgo **no es trabajo de implementación pendiente**, sino una decisión ya tomada en otro
sitio que este ADR nunca recogió. `docs/roadmap/SCALE_BUDGETS.md` (contrato de entrada de R2.1,
§10, ítems 2 y 3) registra exactamente esta corrección como pendiente, y la R2.1 ya la formuló:
*"un documento que promete un guard inexistente es peor que un documento que no lo promete"*.

Las dos decisiones del operador que **superseden** el diseño de §2.3 fila M10.4 y de §4.2:

- **D1 — la lectura es de VENTANA ACOTADA** (`docs/roadmap/SCALE_BUDGETS.md:270`-`276`): *"El agente
  lee franjas. **NO** se promete drenar la sesión entera de forma secuencial."* Consecuencia directa:
  el número de páginas que el agente recorrerá **no es una constante del producto**, sino una
  elección suya.
- **D2 — el techo de 60 s se extiende de captura a lecturas** (`docs/roadmap/SCALE_BUDGETS.md:278`-`284`):
  `ResourceLimits { max_events: 1_000_000, timeout_secs: 60 }`
  (`crates/chronos-mcp/src/server.rs:123`-`137`) pasa a ser **techo duro de cualquier operación del
  read path**, no sólo de captura.

**Por qué esto deja al umbral por tamaño sin objeto, y no simplemente "sin implementar":** una
política de page-vs-summary **por tamaño de sesión** presupone que el read path conoce el tamaño de
la sesión y decide por el llamante si merece un agregado. D1 retira esa premisa — es el **llamante**
quien elige el modo, leyendo franjas acotadas. D2 pone el techo **dentro** de la operación: el
límite que gobierna una lectura pasa a ser un máximo duro de eventos y de tiempo, no un punto de
conmutación a un modo distinto. Juntas, la pregunta "¿la sesión supera 100_000 eventos, así que
devuelvo un resumen?" deja de tener sentido como política: la respuesta la da el agente, y el
límite que le protege ya no es el umbral sino el techo de D2.

Por eso este ADR **no cablea el guard**, y por eso la lectura correcta para quien abra §2.3 dentro de
seis meses no es "falta código": es que **faltaba una decisión, y esa decisión ya se tomó en
`SCALE_BUDGETS.md`**. Cablear ahora un switch por threshold reintroduciría una política que D1 y D2
ya reemplazaron.

### §10.4 Qué queda vigente de §2.3 y §4.2

Se retiran **sólo** el override por env var y el switch por tamaño. El resto de la fila M10.4 sigue
lo que dice: aggregation layers (time-bucketed summaries + per-invocation rollups), sandbox de
trazas grandes, y la **REC-C1/REC-C2 regression suite** de 8 UATs. `EventSummary`,
`InvocationRollup`, `summarize_log` y `rollup_log` existen y son reales
(`crates/chronos-services/src/virtualization.rs:81`, `:126`, `:202`, `:252`).

**Pendiente, y no resuelto por esta sección**: el enforcement de D2 (techo duro en el read path)
sigue **decidido y sin mecanismo** (`SCALE_BUDGETS.md` §9.3 y §10, ítem 1). Esta corrección
documental **no** lo implementa ni lo certifica.
