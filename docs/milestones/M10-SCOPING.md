# M10-SCOPING — Execution Explorer: contrato de lectura/paginación, live/evidence/provenance, virtualization

> **Estado del slice (2026-09-22)**: docs-only scoping. NO se ha ejecutado ningún sub-cycle de M10 en `main @ 678827e9`. Foundation pre-existente **sustancial** identificada. Scoping propuesto con 5 sub-cycles M10.2..M10.6 + M10.1 inventory.

## §1 ROADMAP §M9 §87 ref

ROADMAP §M10 §95 — "M10.1 contrato de lectura/paginación y permisos; M10.2 live/evidence/provenance; M10.3 causality/mutation/properties/compare cuando cada fuente se haya certificado; M10.4 virtualización de trazas grandes; M10.5 accesibilidad y validación UX/seguridad; UAT-M10-01/02."

## §2 Estado actual del repo (inspección directa)

`main @ 678827e9` tiene **foundation pre-existente masiva** para M10. NO es "construir desde cero"; es "consolidar y exponer lo construido". Inventario:

### §2.1 Pagination + cursor (M10.1)

- **`EventsCursorV1`** (`crates/chronos-services/src/events_cursor.rs`, 386L, 13 unit tests) — REC-C1.1 authoritative cursor. Value type `(schema_version, session_id, next_seq)`; opaque on wire via `encode()`. Errores tipados: `Malformed`, `UnsupportedVersion`, `WrongSession`. Documentado como "position in an authoritative log, not a serialized query" (deliberadamente rechaza offsets, timestamps, filters, query state, page sizes).
- **`events_log_read::read_page`** (`crates/chronos-services/src/events_log_read.rs`, 1507L, 30 unit tests) — `read_page(log, cursor, limit, filters) -> (page, next_cursor)`. Cursor advance idempotente; filtros aplicados DESPUÉS de la selección (nunca afectan la posición del cursor). 1507L + 30 tests = production-grade.
- **`CanonicalDrainPage`** (`crates/chronos-services/src/canonical_drain.rs`, 737L, 11 unit tests) — `read_canonical_drain_page` para streaming canónico. Constantes: `DEFAULT_MAX_RAW_EVENTS=512`, `DEFAULT_MAX_EXAMINED_RECORDS=50_000`, `DEFAULT_MAX_DERIVED_PER_SOURCE=4_096`. 11 tests.

### §2.2 Read services (M10.2)

- **`ChronosEventsReadService`** (`crates/chronos-services/src/events_read.rs`, 426L, 0 tests propios — REC-C1.3 path). Entry-point único para agent-visible event reads. Despacha v1 `query_events` (mode=Query) + v1 `get_event` (mode=ById). Cursor opaco en wire. Completeness reportada como "unknown" hasta REC-C1.4 (nunca "complete" — sería Silent Lie per ADR-0004).
- **`DebugReadService`** (`crates/chronos-services/src/debug_read.rs`, 606L, 0 tests propios). 7 métodos read-only: `evaluate_expression`, `get_scope_variables`, `get_audit_entry`, `get_memory_access`, `get_register_read`, `get_state_diff`, `get_variable_change`. Mutex held only for duration of sync call.

### §2.3 REC-C1 / REC-C2 acceptance criteria

`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` §99-§124 define **5 UAT-REC-C1** + **3 UAT-REC-C2**:

- **UAT-REC-C1-01**: two independent consumers (independencia de readers).
- **UAT-REC-C1-02**: real cursor semantics (no offset/limit leak).
- **UAT-REC-C1-03**: gap truth (gaps reportados honestamente, no llenados).
- **UAT-REC-C1-04**: cursor invalid/stale (errores tipados correctos).
- **UAT-REC-C1-05**: time semantics (timestamps monotónicos).
- **UAT-REC-C2-01**: EventBus cannot steal agent evidence (aislamiento).
- **UAT-REC-C2-02**: tripwire evidence is replayable (replay-safe).
- **UAT-REC-C2-03**: no dual-truth divergence (single source of truth).

Estas 8 UATs son acceptance criteria del foundation M10 pre-existente. M10 debe **reusar** estas UATs como regression checks (no duplicar).

### §2.4 Product design (M10.5)

`docs/chronos-agentic-reconstruction/docs/gui/EXECUTION_EXPLORER.md` (39L) define las 7 vistas del Explorer: Live + Execution + Causality + Mutation Lens + Hypotheses/Properties + Compare + Evidence inspector. Diseñado como "not a VS Code/GDB clone"; visualiza lo que las herramientas genéricas no muestran bien.

Es decir: hay **foundation** (EventsCursorV1 + read services), **UATs** (REC-C1/REC-C2), y **product vision** (EXECUTION_EXPLORER.md). Falta:

1. **Wire shape canónico unificado** (consolidar read_page + read_canonical_drain_page + events_read en un solo contrato MCP).
2. **Permissions** (M10.1 mencionaba "permisos" — no hay model de permisos todavía).
3. **Live streaming** (events_log_read es page-based; falta streaming de eventos nuevos sin polling).
4. **Virtualization de trazas grandes** (M10.4 — vista aggregada cuando la traza tiene >1M eventos).
5. **Accessibility + UX validation** (M10.5 — patrón describe pero no testea).
6. **UAT-M10-01/02 ejecutables** (ROADMAP §M10 §95 menciona 2 UATs pero no están en MILESTONE_ACCEPTANCE.md §M10 todavía).

## §3 Sub-cycles propuestos M10.2..M10.6

Cada sub-cycle entrega capacidad verificable + tests incrementales (per ADR-0004). Patrón seguido: m8-01..m8-06.

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **M10.1** (inventory) | ESTE slice | M10-SCOPING.md + ADR-0029 foundation inventory (incluye discovery de EventsCursorV1 386L+13 tests + events_log_read 1507L+30 tests + canonical_drain 737L+11 tests + events_read 426L + debug_read 606L = **~3,662 LoC + 54 tests** pre-existente) + REC-C1/REC-C2 mapping | (ADR docs-only) |
| **M10.2** | Wire shape unificado + permissions | `chronos-mcp::execution_explorer` module consolidando `read_page` + `read_canonical_drain_page` + `events_read` en un solo contrato MCP; `Permissions` enum (ReadEvents/ReadDebug/ReadCompare); sandbox test fixture con permission denial | +8 unit + 4 sandbox |
| **M10.3** | Live streaming + causality wiring | Live event stream (push-based sobre EventsCursorV1 + session_log registry); integration con `detect_concurrent_access` cuando M9 esté certified; integration con `session_fingerprint` (M7.3) + `align_sessions` (M7.2) cuando M7 esté CLOSED (ya está). | +6 unit + 3 sandbox + 2 integration |
| **M10.4** | Virtualization de trazas grandes | Aggregation layers (time-bucketed summaries + per-invocation rollups); threshold-based switch page↔summary; UAT-REC-C1-01..05 regression runs; UAT-REC-C2-01..03 regression runs; sandbox test con trazas de 1M eventos | +8 unit + 3 sandbox + 8 REC regression |
| **M10.5** | Accessibility + UX validation | a11y audit per WCAG 2.2 (no somos GUI todavía pero sentamos bases); JSON Schema validation de wire shapes; error message consistency across read services; UAT-M10-01/02 ejecutores | +5 unit + 2 UAT |
| **M10.6** (close) | Integration + close report | full T1/T2/T3 sobre `main`; close report `docs/milestones/M10-CLOSE.md`; tag `m10-execution-explorer.0`; actualizar STATE + JOURNAL + ROADMAP §M10 §95 con check de cierre | T1+T2+T3 |

**Rationale para 5 sub-cycles (M10.2..M10.6)**: sigue el patrón m8-01..m8-06. M10.6 es close-of-record. Foundation masiva reduce scope de M10.2 (wire shape unificado, NO construcción desde cero) + M10.4 (virtualization es additive, no replacement). M10.3 depende de M9.3+M9.4 (causality) cuando M9 esté CLOSED — pero la integración puede arrancar antes con stub que retorna "Unsupported" hasta que M9 esté certificado.

## §4 Architecture decisions

### §4.1 D1: Reusar REC-C1/REC-C2 UATs como regression checks

**Decisión**: M10.4 incluye los 8 UAT-REC-C1/REC-C2 como regression suite. NO crear nuevos UATs hasta que los REC estén certificados.

**Rationale**: REC-C1/REC-C2 ya cubren cursor semantics, gap truth, isolation, replay safety, single-source-of-truth. Duplicar = mentira.

### §4.2 D2: M10.3 NO bloquea sobre M9 closed

**Decisión**: M10.3 implementa el wire path para causality con stub "Unsupported until M9 certified" en lugar de bloquear hasta M9.6.

**Rationale**: AGENTS.md §1: "trabaja hasta completar todas las capacidades adoptadas del roadmap". M10 puede entregar valor independiente mientras M9 se ejecuta en paralelo.

### §4.3 D3: Permissions model mínimo viable

**Decisión**: M10.2 introduce `Permissions { ReadEvents, ReadDebug, ReadCompare }` enum + deny-by-default + per-tool checks. NO construir ABAC/RBAC completo.

**Rationale**: ROADMAP §M10 §95 menciona "permisos" pero el scope es Execution Explorer (single-user mode), no multi-tenant. Mínimo viable = enum + deny + check; evolucionar a ABAC es M11+.

### §4.4 D4: Virtualization threshold-driven

**Decisión**: M10.4 introduce switch page↔summary cuando `event_count > 100_000` (configurable via env `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`).

**Rationale**: M7.4 cost baseline mostró 1.12M invocations/sec; un trace de 1M eventos cabe en memoria (~25 KB peak) pero la GUI no puede renderizar 1M rows. Threshold-driven switch = honest UX (mostrar summary honesto cuando el detalle es demasiado).

### §4.5 D5: a11y vía schema validation, no GUI

**Decisión**: M10.5 entrega JSON Schema validation de wire shapes + error message consistency. NO construir GUI todavía (es scope futuro).

**Rationale**: M10 es sobre la **API** del Execution Explorer, no la GUI. La GUI consume los wire shapes; validar schemas = sentar bases para GUI a11y-compliant futura.

### §4.6 D6: UAT-M10-01/02 nuevos + REC-C1/REC-C2 regression

**Decisión**: UAT-M10-01 = "permission denial produces typed error" + "pagination respects cursor monotonicity"; UAT-M10-02 = "live stream catches new events without polling" + "summary mode fires at threshold". Ejecutables en M10.5. REC-C1/REC-C2 ejecutables como regression en M10.4.

**Rationale**: UAT-M10-01 cubre M10.2 (permissions) + M10.1 (cursor monotonicity); UAT-M10-02 cubre M10.3 (live streaming) + M10.4 (virtualization threshold).

## §5 Risks

### §5.1 R1: M10.3 depende de M9 cerrado

**Riesgo**: M10.3 wire path para causality requiere `detect_concurrent_access` certified. Si M9.6 NO está CLOSED, M10.3 retorna "Unsupported".

**Mitigación**: Stub explícito (D2) + ADR-0004 honest reporting. M10.3 ejecutable sin M9; integración real cuando M9 esté CLOSED.

### §5.2 R2: REC-C1/REC-C2 regression puede fallar

**Riesgo**: Si el foundation pre-existente cambia (e.g., `EventsCursorV1` rompe monotonicity), REC-C1/REC-C2 fallan.

**Mitigación**: REC tests en CI obligatorio; cualquier cambio al cursor contract requiere ADR + bump schema version. ADR-0028 §4.2 ya documentó esta dependencia.

### §5.3 R3: Threshold de virtualization es heurístico

**Riesgo**: 100_000 eventos puede ser poco (algunos traces tienen 10M+) o mucho (algunos consumers solo quieren 1K).

**Mitigación**: Env-overridable + tests con múltiples thresholds (1K, 10K, 100K, 1M). Documentado en D4.

### §5.4 R4: Wire shape unificado puede romper consumers

**Riesgo**: Consolidar read_page + read_canonical_drain_page + events_read en un solo MCP tool cambia el contrato visible.

**Mitigación**: Versioning de tools (v3 read shape, v2 deprecado pero soportado 6 meses); JSON Schema strict; compatibility tests con consumers existentes.

### §5.5 R5: Permissions enum puede ser demasiado coarse

**Riesgo**: `Permissions { ReadEvents, ReadDebug, ReadCompare }` puede no cubrir todos los casos (e.g., "read events but not debug", "read compare but only with permission X").

**Mitigación**: Mínimo viable en M10.2; evolución a PermissionSet o claims-based en M11+ cuando haya multi-user evidence.

## §6 UAT mapping

| UAT | Source | Sub-cycle |
|---|---|---|
| UAT-M10-01 (permission denial + cursor monotonicity) | New, ROADMAP §M10 §95 | M10.5 |
| UAT-M10-02 (live stream + summary threshold) | New, ROADMAP §M10 §95 | M10.5 |
| UAT-REC-C1-01..05 (cursor semantics + gap truth) | MILESTONE_ACCEPTANCE.md §99-§116 | M10.4 regression |
| UAT-REC-C2-01..03 (isolation + replay + single-truth) | MILESTONE_ACCEPTANCE.md §118-§124 | M10.4 regression |

## §7 Out-of-scope (M10 chapter)

1. **GUI implementation** (scope futuro; M10 sienta la API).
2. **Multi-tenant permissions** (M11+).
3. **Cross-trace comparison** (M10 cubre single-trace Compare; cross-trace es scope futuro).
4. **Replace EventsCursorV1** (REC-C1.1 es foundation; no se reemplaza).
5. **Replace events_log_read** (es production-grade, 1507L+30 tests; no se reemplaza).
6. **Probabilistic event prediction** (nunca; los eventos son facts del trace).
7. **Browser-side rendering** (es scope frontend, no API).
8. **Replace DebugReadService** (es production-grade, 606L; se mantiene).

## §8 References

- ROADMAP §M10 §95 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §99-§124 (`docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md`) — REC-C1/REC-C2 UATs.
- UAT_CATALOG.md §M10 (`docs/roadmap/UAT_CATALOG.md`) — UAT-M10-01/02 (a crear en M10.5).
- ADR-0028 §7 — M9 chapter status (NOT STARTED execution).
- ADR-0027 §2.3 — naming convention.
- ADR-0004 (no falsear una entrega) — aplicado en §4 + §5 (cada sub-cycle verifica su scope antes de declarar DONE).
- `docs/chronos-agentic-reconstruction/docs/gui/EXECUTION_EXPLORER.md` (39L) — product design.
- `crates/chronos-services/src/events_cursor.rs` (386L, 13 tests) — `EventsCursorV1` cursor foundation (REC-C1.1).
- `crates/chronos-services/src/events_log_read.rs` (1507L, 30 tests) — `read_page` paginación (REC-C1.2).
- `crates/chronos-services/src/canonical_drain.rs` (737L, 11 tests) — `CanonicalDrainPage` streaming canónico.
- `crates/chronos-services/src/events_read.rs` (426L) — `ChronosEventsReadService` v2 events_read dispatcher (REC-C1.3).
- `crates/chronos-services/src/debug_read.rs` (606L) — `DebugReadService` 7 read-only methods.
- `docs/chronos-agentic-reconstruction/docs/testing/ARCHITECTURE_FITNESS_FUNCTIONS.md` — REC-C1/REC-C2 acceptance criteria reference.
