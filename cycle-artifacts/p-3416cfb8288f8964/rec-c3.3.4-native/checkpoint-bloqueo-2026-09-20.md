# REC-C3.3.4-native — Checkpoint de Bloqueo Operativo (2026-09-20)

## Estado del prerrequisito

| Prerrequisito | Estado | Evidencia |
|---|---|---|
| Tren B integrado en `origin/main` | **NO CUMPLIDO** | `origin/main = 65754dcb` (audit baseline). `feat/rec-c3.3-train-b` está en `0d3b05dc` (21 commits ahead). Tren B **NO merged**. |
| CI obligatoria finalizada correctamente sobre Tren B | **NO EJECUTABLE** | depende del merge a `origin/main`; sin merge, CI no corre. |
| `python3 scripts/regen_manifest_index_shas.py --check` pasa sobre estado integrado | **NO EJECUTABLE** | depende del merge; chequeo sobre Tren B local muestra drift pre-existente (no causado por Tren B). |

## Decisión tomada

Por instrucción explícita del operador:
> "Si no se cumple, detente en un checkpoint de bloqueo operativo; no simules la integración ni sustituyas el baseline remoto por el de la rama local."

**Detenido.** Sin código modificado en baseline. Sin sustitución de baseline.

Esta sesión ejecutó **solo las fases read-only** del método permitido:
1. ✅ Leer handoff de Tren B (accedido vía `git show feat/rec-c3.3-train-b:session-handoff/...`)
2. ✅ Leer `audit-traceability-matrix.md` (idéntico método)
3. ✅ Reconstruir grafo de dependencias sobre el **nuevo baseline = `origin/main = 65754dcb`**
4. ✅ Caracterizar comportamiento existente (`LiveProbeSession` shape, ProbeService surface, ports existentes)
5. ✅ Evaluar reutilización de ports existentes (`ProbeController`/`ProbeFactory`/`ProbeRegistry`)

## Hallazgos de la fase read-only (pre-implementación)

### Grafo de dependencias sobre `main = 65754dcb`

**services → native (direct)**:
- `crates/chronos-services/src/probe.rs:22`: `use chronos_native::probe_backend::NativeProbeBackend;`
- `crates/chronos-services/src/probe.rs:41`: `pub backend: NativeProbeBackend` (field of `LiveProbeSession`)
- `crates/chronos-services/src/probe.rs:293,411,1022`: `let b = NativeProbeBackend::new()` (3 tests)
- `crates/chronos-services/src/probe.rs:420`: `backend.attach_probe(input.pid, config)` (production)
- `crates/chronos-services/src/probe.rs:467`: returns `chronos_native::probe_backend::AcceptedRawObserver`
- `crates/chronos-services/src/probe.rs:513`: `live_probe.backend.stop_probe(&live_probe.session)` (production)
- `crates/chronos-services/src/probe.rs:607`: `live_probe.backend.clone_resolver_pipeline()` (production)
- `crates/chronos-services/src/probe.rs:649`: `chronos_native::read_log_with_stats` (production)
- `crates/chronos-services/src/probe.rs:1055`: `live.backend.execution_log()` (production)
- `crates/chronos-services/src/observe.rs:1475`: `let backend = chronos_native::probe_backend::NativeProbeBackend::new();` (test)

**11 referencias directas** a `chronos_native::probe_backend` en `services/*`. 6 son production code paths.

**services Cargo.toml**:
```
chronos-query, chronos-domain, chronos-store, chronos-index, chronos-native, chronos-log
```
(`chronos-native` es dep directa, no transitiva.)

### Puertos existentes que podrían cubrir el contrato

**`chronos-domain/src/ports/probe.rs` declara 3 traits**:

| Trait | Métodos | Cobertura |
|---|---|---|
| `ProbeController` (línea 29) | `session_id()`, `stop()`, `detach(self)`, `backend() -> &dyn ProbeBackend` | parcial |
| `ProbeFactory` (línea 53) | `create(session_id, config, requirements) -> Box<dyn ProbeController>` | ok para creación |
| `ProbeRegistry` (línea 72) | `attach()`, `detach()`, `list_active()`, `is_active()` | ok para registro |

**`chronos-domain/src/adapter.rs::ProbeBackend`**:
| Método | En ProbeBackend? |
|---|---|
| `is_available()` | ✅ |
| `name()` | ✅ |
| `stop_probe()` | ✅ |
| `attach_probe()` | ❌ NO |
| `clone_resolver_pipeline()` | ❌ NO |
| `execution_log()` | ❌ NO |
| `attach_execution_log()` | ❌ NO |
| `with_accepted_raw_observer()` | ❌ NO |
| `with_language()` | ❌ NO |
| `with_resolver()` | ❌ NO |
| `accept_and_publish()` | ❌ NO |

### Decisión sobre reutilización de ports existentes (paso 3)

**`ProbeBackend` NO puede cubrir el contrato.** Le faltan 8 métodos que `ProbeService` invoca sobre `LiveProbeSession.backend`.

**`ProbeController.backend() -> &dyn ProbeBackend`** es exactamente el patrón que el auditor §4.5 S4 flaggea como mecanismo para evitar separación de responsabilidades. Reusarlo significaría mantener el smell.

**`ProbeFactory`** + **`ProbeRegistry`** son útiles para **crear/registrar** controllers, pero la question aquí es qué capabilities expone cada controller. Si creamos un nuevo `NativeProbeController` que implementa `ProbeController` con la superficie correcta, podemos reusar `ProbeFactory` (con un impl que produzca `NativeProbeController`) y `ProbeRegistry` (que acepta cualquier `Box<dyn ProbeController>`).

**Conclusión**: **se justifica un nuevo trait** (propuesto: `NativeProbeController` o ampliar el existente `ProbeController` con la superficie correcta — pendiente decisión de diseño). La justificación contractual es concreta: 8 métodos que `services/*` invocan sobre el backend y que no están en `ProbeBackend`.

### Caracterización del comportamiento (paso 2, parcial)

`LiveProbeSession` (services/probe.rs:39) tiene 6 campos:
- `backend: NativeProbeBackend` ← **target del refactor**
- `session: CaptureSession`
- `language: Language`
- `target: String`
- `attached: bool`
- `uprobe_handle: Option<Arc<UprobeHandle>>` (REC-C3.3.2.3)

`ProbeService` (services/probe.rs) expone 11 métodos públicos que usan `backend`:
- `start()` (no usa backend directamente — solo crea config)
- `start_attach()`: usa `backend.attach_probe`
- `stop()`: usa `backend.stop_probe`
- `drain()`: usa `backend.execution_log()`
- `advance()`: usa `NativeProbeBackend::advance` (slice E partial de Tren B)
- `step()`: usa `NativeProbeBackend::step` (slice E partial de Tren B)
- `drain_log()`: usa `chronos_native::read_log_with_stats`
- `compaction_metrics()`: no usa backend
- `session_snapshot()`: usa `backend.clone_resolver_pipeline()`
- `inject()`: usa uprobe (no backend directo)
- `status()`: usa `backend.execution_log()`

`LiveProbeSession.backend` se accede desde `services/probe.rs` (6 lugares) y `services/observe.rs` (1 lugar, test).

## Plan tentativo de implementación (NO ejecutado)

### Fase A: definir `NativeProbeController` trait (chronos-domain/src/ports/probe.rs)

```rust
pub trait NativeProbeController: ProbeController + Send + Sync {
    fn attach_to_pid(&self, pid: i32, config: &CaptureConfig) -> Result<CaptureSession, TraceError>;
    fn advance(&self) -> Result<AdvanceOutput, TraceError>;
    fn step(&self) -> Result<StepOutput, TraceError>;
    fn drain_log(&self) -> Result<...>;
    fn session_snapshot(&self) -> Result<ResolverPipeline>;
    fn execution_log(&self) -> Option<Arc<dyn ExecutionLogProvider>>;
    // NO `backend() -> &dyn ProbeBackend` (audit §4.5 S4)
}
```

### Fase B: impl para `NativeProbeBackend` en `chronos-native`

```rust
// chronos-native/src/probe_controller.rs (NEW)
pub struct NativeProbeController {
    backend: NativeProbeBackend,
    session_id: SessionId,
}
impl NativeProbeController for NativeProbeController { ... }
impl ProbeController for NativeProbeController { ... }  // session_id(), stop(), detach()
```

### Fase C: factory + registry wiring en `chronos-mcp::composition`

```rust
pub fn default_native_probe_factory() -> Arc<dyn ProbeFactory>;
// impl returns NativeProbeController
```

### Fase D: rewire `services::probe`

- `LiveProbeSession.backend: NativeProbeBackend` → `controller: Box<dyn NativeProbeController>`
- Cada `backend.attach_probe(...)` → `controller.attach_to_pid(...)`
- Cada `backend.execution_log()` → `controller.execution_log()`
- Tests: añadir `MockNativeProbeController` y tests contractuales parametrizados (audit §4.3 LSP)

### Fase E: rewire `services::observe` test que usa `NativeProbeBackend::new()`

### Fase F: validate + commit + vault row

Cada commit con T0 (fmt + clippy -D warnings) + T2 (services 377/377) + T4-smoke (native_probe_tools + e2e_connectivity).

## Bloqueo documentado

**Acción de recuperación precisa para el operador**:

1. Completar la integración remota de Tren B (per `session-handoff/rec-c3.3-train-b-2026-09-20.md`):
   - `git push origin feat/rec-c3.3-train-b`
   - abrir PR o ff-merge según política del repo
2. Esperar CI obligatoria en `origin/main` (5 workflows deben pasar).
3. Ejecutar `python3 scripts/regen_manifest_index_shas.py --check` sobre el estado integrado. Si drift, regenerar.
4. Reanudar este ciclo desde la Fase A del plan.

Mientras el bloqueo persista, **no hay trabajo de código que pueda avanzar** sin violar la condición "no sustituyas el baseline remoto por el de la rama local".

## Lecciones para el próximo intento (cuando se reanude)

1. **No subestimes el bloqueo de integración**: Tren B dejó 21 commits sin mergear 18 horas (suspendió 19 sep 11:58Z, este ciclo arrancó 20 sep 08:50Z). El bloqueo se arrastró.

2. **El audit §4.5 S4 es accionable ahora**: `ProbeController.backend()` es el smell exacto que el audit señala. El nuevo trait debe diseñarse sin ese accessor.

3. **Tests contractuales LSP** (audit §4.3) requieren:
   - `MockNativeProbeController` con la misma superficie que el real
   - `#[test_case]` o `proptest!` con la misma batería ejecutada contra ambos
   - Esto debe existir ANTES del rewire, como contrato a verificar

4. **Caracterización previa del comportamiento** (paso 2) reveló que `ProbeService` tiene 11 métodos públicos, 6 usan `backend`. El rewire afecta ~6 lugares en `services/probe.rs` + 1 test en `services/observe.rs`. Estimación: 3-4 commits atómicos (Fase B, C, D en commits separados para revisión atómica per B7).

## Archivos de evidencia (read-only)

- `crates/chronos-domain/src/ports/probe.rs` (95 líneas leídas) — 3 traits existentes
- `crates/chronos-domain/src/adapter.rs` (80 líneas leídas) — ProbeBackend trait
- `crates/chronos-services/src/probe.rs` (1-60, 264-911, 1022-1075 leídos) — ProbeService + LiveProbeSession
- `crates/chronos-services/src/observe.rs` (línea 1475 leída) — único test reference
- `crates/chronos-native/src/probe_backend.rs` (métodos públicos leídos) — superficie completa de NativeProbeBackend
- `crates/chronos-services/Cargo.toml` (deps leídas) — confirmación de `chronos-native` directo

Ningún archivo fue modificado.
