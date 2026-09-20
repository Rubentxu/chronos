# Tren B — Audit Traceability Matrix (post-cycle)

**Cycle**: rec-c3.3-train-b (REC-C3.3.3 Tren B)
**Base**: `fa5eb5827e997c874047c6c2318ad2b5b1c8f323`
**HEAD**: `c9fbf0549f64e337254f5c33a5813c6795222aab` (this matrix)
**Audit baseline**: `65754dcb85e2c7b6d3902a78fe70a6bca4871b77` (origin/main pre-merge)

## Hallazgos del auditor → comprobación Tren B → estado actual

### 1. `services → native` (audit §3.2 A2)

**Comprobación**: ¿qué referencias directas a `chronos_native` quedan en `services/*`?

**Evidencia post-Tren-B** (grep en `crates/chronos-services/src/`):

```
crates/chronos-services/src/observe.rs:1475   let backend = chronos_native::probe_backend::NativeProbeBackend::new();
crates/chronos-services/src/probe.rs:22       use chronos_native::probe_backend::NativeProbeBackend;
crates/chronos-services/src/probe.rs:44       pub backend: NativeProbeBackend,           ← LiveProbeSession struct field
crates/chronos-services/src/probe.rs:296      let b = NativeProbeBackend::new()
crates/chronos-services/src/probe.rs:414      let b = NativeProbeBackend::new()
crates/chronos-services/src/probe.rs:1074     let b = NativeProbeBackend::new()
```

**Estado**: **NO RESUELTO**. `services → native` sigue presente. `LiveProbeSession`
contiene directamente `NativeProbeBackend` (el ejemplo exacto que el auditor
citó en §3.2 A2).

**Avance parcial Tren B**: el slice E partial (`c96d513a`) añadió
`NativeProbeBackend::advance/step` y `ProbeService::advance/step` — pero esto
amplió la API del backend concreto en lugar de invertir la dependencia. El
camino crítico (definir un port `LiveProbeBackend` y hacer que `LiveProbeSession`
lo consuma) no se tocó.

**Owner**: REC-C3.3.4-native (siguiente ciclo, scope ya documented en
`session-handoff/rec-c3.3-train-b-2026-09-20.md`).

### 2. `services → store` (audit §3.2 A1)

**Comprobación**: ¿consumen los servicios contratos independientes de
infraestructura, o siguen accediendo directamente a `SessionStore`?

**Evidencia post-Tren-B** (servicios que **siguen** importando `SessionStore`
directamente, post-slice F + E rewire):

```
crates/chronos-services/src/ce_services_tests.rs     ← test helpers only
crates/chronos-services/src/diff.rs:18,30           ← DiffContext: &SessionStore
crates/chronos-services/src/session_compare.rs      ← import
crates/chronos-services/src/session_explain.rs:30,34 ← ExplainContext: &SessionStore
crates/chronos-services/src/session_lifecycle.rs    ← import
crates/chronos-services/src/sessions.rs             ← SOLO TESTS post-slice F (production uses archive port)
```

**Estado**: **PARCIALMENTE RESUELTO**.

- ✅ `SessionsService` (production) ya consume `&dyn SessionArchive` (port).
  Verificado por 8 integration tests + 377 services lib tests. Es el contrato
  que el auditor §3.2 A1 describe: "el contrato debe describir necesidades
  de aplicación, no replicar la firma del store".
- ⚠️ `DiffService`, `ExplainService`, `CompareService`, `LifecycleService`
  **siguen** con `&SessionStore` directo. Estos servicios **no fueron
  tocados** por Tren B (fuera del task-graph).
- ⚠️ `ChronosCounterexampleService` (production) sigue con
  `&chronos_store::SessionStore` (DEBT-01).

**Owner del resto**: hay 4-5 servicios adicionales que necesitan el mismo
tratamiento que `SessionsService` (port + adapter + rewire). No es trabajo
de Tren B ni de REC-C3.3.4 (que es native). Propongo un ciclo dedicado
`REC-C3.5-services-store` después de que native esté cerrado, o repartirlo
entre ciclos de servicios individuales.

### 3. `store → native` (audit §3.2 A3)

**Comprobación**: ¿continúa como excepción `Cargo.toml`?

**Evidencia** (`crates/chronos-store/Cargo.toml`):

```toml
# For address normalization (optional)
chronos-native = { path = "../chronos-native", optional = true }

[features]
default = []
address_normalization = ["dep:chronos-native"]
```

**Estado**: **NO TOCADO** por Tren B (correctamente — fuera de scope).
La dependencia opcional `address_normalization` sigue registrada. Owner:
REC-C3.4 (existe en el roadmap previo, audit §3.2 A3 confirma scope).

### 4. Estado distribuido de sesiones (audit §5.1)

**Comprobación**: ¿modificó Tren B la propiedad o coordinación del estado?

**Evidencia**: el slice F (`d7cdbae9`) **añadió** un campo `archive:
Arc<dyn SessionArchive>` al struct `ChronosServer`. El estado de archive
es ahora propiedad del struct. Los 5 `SessionsContext` constructions
en handlers MCP propagan el port vía `&*self.archive`.

Sin embargo, el campo `store: Arc<SessionStore>` **sigue existiendo** —
la auditoría §6.2 lo señala como parte del god-object, y Tren B no tocó
el resto del struct (intencionalmente: el audit §13 dice "no dividir
todos los módulos grandes en un único ciclo").

**Estado**: **MEJORA PARCIAL**.
- ✅ SessionsContext ya no depende de `&SessionStore` directo.
- ⚠️ `ChronosServer` sigue siendo god-object (389KB), sigue manteniendo
  tanto `store` como `archive` (paralelismo temporal hasta que el resto
  de servicios se migre).

**Owner**: REC-C4 (vertical extraction por flujo, audit §6.2).

### 5. Contratos de instrumentación (audit §4.4, §14.1)

**Comprobación**: ¿preservan los nuevos ports el ciclo de vida y los errores
existentes?

**Evidencia — ProbeService::advance/step** (slice E partial `c96d513a`):

```rust
// AdvanceOutput / StepOutput nuevos
// ServiceError::SessionRunning (variante nueva)
// ServiceError::SessionStopped (variante nueva)
// From<TraceError> for ServiceError (mapping impl)
```

**Evidencia — handler MCP probe_advance/probe_step** (slice G `d6509b9a`):

```
ProbeNotFound  -> session_not_found (typed error)
SessionStopped -> session_stopped  (advance requiere paused/stopped)
SessionRunning -> session_running  (step requiere paused)
generic        -> advance/step failed: {e}
```

**Estado**: **RESUELTO** para `advance/step`. Los errores preservan la
información semántica (no `String` opaque, sino variantes tipadas). El
ciclo de vida de probe ahora distingue explícitamente running/stopped
en el port.

**Caveat (audit §4.3 LSP)**: el auditor señala que la sustitución entre
implementaciones (in-memory vs production) debe demostrarse con la misma
batería de tests. Esto **no se hizo** en Tren B porque el adapter
`InMemoryProbeBackend` no existe como tal — los tests usan `MockNative`
inline. Owner: REC-C3.3.4-native cuando se defina el port.

## Resumen ejecutivo (lo que sí cambió vs lo que no)

### Cambió ✓

| Hallazgo | Acción Tren B | Evidencia |
|---|---|---|
| `SessionMetadata` en `services/*` usa tipo de infra | Lift a `chronos-domain::SessionMetadata` (re-export via store) | `crates/chronos-domain/src/session.rs:28`, 8 call-sites swapped |
| `SessionsService` consume `&SessionStore` directo | Migrado a `&dyn SessionArchive` port | `crates/chronos-services/src/sessions.rs:44`, 8 integration tests |
| `composition::default_session_archive` factory | Añadido + usado en ambos constructores `ChronosServer` | `crates/chronos-mcp/src/composition.rs:99`, `server.rs:1794` |
| `probe_advance` + `probe_step` no existen en MCP | Handlers añadidos con errores tipados | `server.rs:~4564-4646`, sandbox 4/4 verde |
| `ServiceError::{SessionRunning, SessionStopped}` | Variantes nuevas con mapping `From<TraceError>` | `crates/chronos-services/src/error.rs:20` |
| NativeProbeBackend::advance/step | Métodos añadidos al backend concreto | `crates/chronos-native/src/probe_backend.rs:30` |

### No cambió ✗ (con owner)

| Hallazgo | Razón | Owner |
|---|---|---|
| `services → native` (LiveProbeSession tiene `NativeProbeBackend`) | Tren B no atacó el rewire completo; slice E partial solo extendió el backend concreto | REC-C3.3.4-native |
| `services → store` para Diff/Explain/Compare/Lifecycle/Counterexample | Fuera del task-graph de Tren B (que solo cubrió `SessionsService`) | REC-C3.5-services-store (propuesto) o distribuir entre ciclos |
| `store → native` (Cargo.toml `address_normalization`) | Fuera de scope Tren B (correctamente) | REC-C3.4 (existe en roadmap) |
| `ChronosServer` god-object (389KB) | Audit §6.2 + §13: no extracción masiva | REC-C4 |
| `CounterexampleRepository` port sin consumidor real | Audit §13: marcado EXPERIMENTAL; sin servicio que lo consuma todavía | REC-C3.5-services-store (cuando se migre counterexample) |
| `capture_session` MCP tool | NO implementado (sandbox test assert method-not-found) | REC-C3.3.4 (propuesto, prioridad P1) |
| Harness binary identity check (audit §8.3) | Cross-cutting, no en Tren B | REC-C0.5-harness (propuesto) |

## Lecciones para el siguiente ciclo (REC-C3.3.4-native)

1. **Verificar el grafo actualizado, no el histórico.** El auditor tiene razón:
   las dependencias pueden haber cambiado desde el último baseline. Recomiendo
   correr una herramienta de grafo de deps (cargo metadata o cargo-deps) sobre
   el nuevo main antes de empezar a mover servicios.

2. **Reutilizar puertos existentes antes de añadir nuevos.** El audit §3.2 A2
   recomienda explícitamente: "Primero comprobaría si esas interfaces
   expresan el contrato que necesita el caso de uso. Solo introduciría una
   abstracción adicional si aparece una necesidad funcional que no pueda
   representarse correctamente mediante los puertos existentes."
   Hay que revisar `ProbeController`, `ProbeFactory`, `ProbeRegistry` antes
   de definir un `LiveProbeBackend` port.

3. **Inversión con consumidor real, no especulativa.** El error honesto de
   Tren B fue crear `CounterexampleRepository` antes de tener un servicio
   que lo consuma. REC-C3.3.4-native debería seguir el orden inverso:
   primero identificar el servicio que va a consumir el port, luego
   definir el port para ese consumidor.

4. **Tests contractuales parametrizados.** El audit §4.3 (LSP) dice:
   "Propondría un conjunto de pruebas contractuales parametrizadas por
   implementación que opere exclusivamente mediante los puertos públicos."
   Esto es debt para REC-C3.3.4-native: una vez definido el port,
   escribir tests que operen sobre el port (no sobre el backend) y
   ejecuten la misma batería contra `InMemoryLiveProbe` y
   `NativeLiveProbe`.

## Archivos para la siguiente auditoría

Si la siguiente auditoría quiere revisar el estado real, estos son los
archivos a inspeccionar:

- `crates/chronos-domain/src/ports/session.rs` — `SessionArchive` port (nuevo)
- `crates/chronos-domain/src/ports/counterexample.rs` — `CounterexampleRepository` port (nuevo, EXPERIMENTAL)
- `crates/chronos-store/src/session_archive.rs` — adapter (nuevo)
- `crates/chronos-store/src/counterexample_repository.rs` — adapter (nuevo, EXPERIMENTAL)
- `crates/chronos-services/src/sessions.rs` — SessionsService consumption (modificado)
- `crates/chronos-mcp/src/composition.rs` — factories (modificado)
- `crates/chronos-mcp/src/server.rs` — handlers + ChronosServer field (modificado)
- `crates/chronos-native/src/probe_backend.rs` — advance/step (modificado, NO port — DEBT-04)
