# Plan de Implementación y Limpieza — Chronos Semantic Event Bus

> Refactorización de "grabadora de caja negra" → "bus de eventos semánticos para LLMs"

---

## Resumen Ejecutivo

Chronos tiene **37,412 LOC** distribuidas en **17 crates**. La mayoría del código está bien diseñado pero refleja el paradigma anterior: "grabar todo, analizar después". El nuevo paradigma es "inyectar sondas dinámicamente, resolver semánticamente, consultar bajo demanda".

**Lo que se elimina (~10,000 LOC):**
- `chronos-sandbox` completo (6,274 LOC) — harness CI que reemplaza al LLM como orquestador
- `BreakpointManager` + `HardwareWatchpointManager` (764 LOC) — INT3 interactivo
- Paths interactivos DAP/CDP en los 5 adaptadores de lenguaje (Python, Java, Go, JS, Browser)
- `chronos-format` (540 LOC) — formato flat "grabar todo"
- `SessionEvalDispatcher` + `eval_dispatcher.rs` (376 LOC) — evaluación en proceso vivo
- 3 tools MCP huérfanas ya eliminadas (`compare_sessions`, `performance_regression_audit`, `debug_orchestrate`)

**Lo que se refactoriza (~15,000 LOC):**
- `debug_run` (548 LOC en server.rs) →拆分→ 3 tools MCP
- `TraceAdapter` trait duplication → colapsar en `ProbeBackend`
- `CaptureRunner` ptrace loop → alimentador de `EventBus`
- `QueryEngine` → `LiveQueryEngine` para sesiones en vivo

**Lo que se construye (~5,000 LOC):**
- `ProbeBackend` trait (reemplaza `TraceAdapter`)
- `EventBus` con ring buffer in-memory
- `SemanticResolver` pipeline (por lenguaje)
- `Tripwire` / condition system
- eBPF `probe_inject` API
- 3 tools MCP faltantes

---

## FASE 0 — Limpieza Inicial (Semana 1, ~2 días)

### T0.1 — Eliminar `chronos-sandbox` (6,274 LOC)

**Orden seguro:**，没有任何 otro crate depende de `chronos-sandbox` directamente.

```bash
# Verificar que nada lo reference
grep -r "chronos-sandbox" crates/*/Cargo.toml

# Resultado esperado: solo el propio crate y chronos-e2e (tests)
# chronos-e2e/tests/ pueden sobrevivir como tests de integración
```

**Acción:** Eliminar `crates/chronos-sandbox/` completo. Si `chronos-e2e` tiene tests que dependían del sandbox, Those tests deben reescribirse como tests de integración directos o marcarse como `#[ignore]`.

---

### T0.2 — Eliminar `breakpoint.rs` y `watchpoint.rs` (764 LOC)

**Solo `chronos-native`** depende de ellos.

```bash
# Archivos a eliminar:
rm crates/chronos-native/src/breakpoint.rs
rm crates/chronos-native/src/watchpoint.rs

# En capture_runner.rs, eliminar:
# - use crate::breakpoint::*;
# - use crate::watchpoint::*;
# - BreakpointManager, HardwareWatchpointManager exports
# - Las llamadas a BreakpointManager en run_capture_loop
```

**Verificar** que `ptrace_tracer.rs` no reference `BreakpointManager`.

---

### T0.3 — Eliminar `chronos-format` (540 LOC)

Solo `chronos-native` y `chronos-e2e` usan `TraceFileWriter`.

```bash
rm -rf crates/chronos-format/
# En chronos-native/src/capture_runner.rs:
# Reemplazar TraceFileWriter por acumulador en memoria (Vec<Vec<u8>>)
# Eliminar: use chronos_format::*;
```

**Nueva dependencia:** `chronos-native` ya no depende de `chronos-format`.

---

### T0.4 — Eliminar `eval_dispatcher.rs` (376 LOC)

Solo `chronos-mcp/src/server.rs` referencia `SessionEvalDispatcher`.

```bash
rm crates/chronos-query/src/eval_dispatcher.rs
# En server.rs: eliminar use y todas las llamadas a SessionEvalDispatcher
# Los language adapters (python, java, go, js) ya no registran eval backends
```

---

## FASE 1 — Refactorización de Trait y Arquitectura (Semana 1-2, ~3 días)

### T1.1 — Colapsar `TraceAdapter` en `ProbeBackend`

**Dos traits existentes:**
- `chronos-domain::TraceAdapter` — 3 métodos: `is_available()`, `name()`, `drain_events()` — *modelo pull,是对的*
- `chronos-capture::TraceAdapter` — 8 métodos: `start_capture`, `stop_capture`, `attach_to_process`, `get_threads`, `get_stack_trace`, `get_variables`, `get_runtime_info`, `evaluate_expression` — *modelo start/stop, demasiado acoplado*

**Decisión de diseño:** Mantener la separación actual pero:
1. `chronos-domain::TraceAdapter` se renombra a `ProbeBackend` (pull, streaming)
2. `chronos-capture` se convierte en un **registry** de adapters (sin definir el trait)

```rust
// Nuevo: chronos-domain/src/adapter.rs
pub trait ProbeBackend: Send {
    fn is_available(&self) -> bool;
    fn name(&self) -> &str;
    fn probe_handle(&self) -> &ProbeHandle;
    fn drain_events(&mut self) -> Result<Vec<SemanticEvent>, ProbeError>;
}

pub struct ProbeHandle { /* opaque, cada backend decide */ }
pub enum ProbeError { ... }
```

**Todos los adaptadores de lenguaje** (`chronos-native`, `chronos-ebpf`, `chronos-python`, etc.) implementan `ProbeBackend`.

---

### T1.2 — Reemplazar `CaptureRunner` por alimentador de `EventBus`

`CaptureRunner` en `chronos-native` actualmente:
1. Hace `spawn()` del proceso
2. Loop de ptrace hasta exit
3. Acumula `Vec<TraceEvent>`
4. Lo devuelve todo al final

**Nuevo flujo:**
```rust
// NativeProbeBackend
pub struct NativeProbeBackend {
    event_bus: Arc<EventBus>,
    tracer: PtraceTracer,
    symbol_resolver: SymbolResolver,
}

impl NativeProbeBackend {
    pub fn attach(target: &Path, pid: Option<i32>) -> Result<ProbeHandle, ProbeError> {
        // 1. Spawn/attach ptrace
        // 2. Pre-cargar símbolos DWARF
        // 3. Instalar uprobes (INT3 en entry) si capture_function_exit=true
        // 4. Arrancar thread de polling que feeds EventBus
    }

    fn polling_loop(&self) {
        loop {
            let event = self.tracer.wait_stop();
            let trace_event = self.resolve_raw_event(event);
            self.event_bus.push(trace_event);  // NON-blocking push
        }
    }
}
```

**Todos los adaptadores** (Python, Java, Go, JS, eBPF) siguen el mismo patrón: polling loop → `EventBus::push()`.

---

### T1.3 — Construir `EventBus` (ring buffer in-memory)

**Nuevo archivo:** `chronos-domain/src/bus.rs`

```rust
pub struct EventBus {
    ring: Arc<RwLock<VecDeque<SemanticEvent>>>,
    capacity: usize,
    metrics: BusMetrics,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&self, event: SemanticEvent);  // blocking si full → evict oldest
    pub fn snapshot(&self) -> Vec<SemanticEvent>;  // para LLM query
    pub fn stream(&self) -> impl Stream<Item = SemanticEvent>;  // para consumo async
}

#[derive(Clone)]
pub struct BusMetrics {
    pub total_events: Arc<AtomicUsize>,
    pub watermark: Arc<AtomicUsize>,  // high-water mark
    pub overflow_count: Arc<AtomicUsize>,
}
```

**Política de evict:** FIFO simple (el más antiguo se descarta cuando se llena). Para el caso LLM "dame los últimos 5 segundos", el capacity se calcula como `5seg * eventos_por_segundo_estimado`.

**Conexión con CAS:** `EventBus::snapshot()` → `SessionStore::save_session()` (hash BLAKE3 + dedup).

---

## FASE 2 — Sistema de Tripwires (Semana 2, ~2 días)

### T2.1 — TripwireSpec y TripwireManager

**Nuevo archivo:** `chronos-domain/src/tripwire.rs`

```rust
pub struct Tripwire {
    pub id: TripwireId,
    pub condition: TripwireCondition,
    pub action: TripwireAction,
}

pub enum TripwireCondition {
    EventType(Vec<EventType>),
    FunctionName(GlobPattern),
    ExceptionType(String),
    MemoryAddress { addr: u64, access: AccessKind },
    Custom(Box<dyn Fn(&SemanticEvent) -> bool + Send + Sync>),
}

pub enum TripwireAction {
    Notify(tokio::sync::mpsc::Sender<TripwireFired>),
    Log,
    PauseSession,
}

// En EventBus::push():
pub fn push(&self, event: SemanticEvent) {
    // 1. Push al ring buffer
    // 2. Check todos los tripwires activos
    // 3. Si alguno fired → ejecutar su action
}
```

**MCP tool asociado:**
```rust
#[tool(name = "tripwire_create")]
async fn tripwire_create(params: TripwireParams) -> TripwireId;

#[tool(name = "tripwire_list")]
async fn tripwire_list() -> Vec<Tripwire>;

#[tool(name = "tripwire_delete")]
async fn tripwire_delete(id: TripwireId);
```

---

## FASE 3 — Adaptadores de Lenguaje como Semantic Resolvers (Semana 2-3, ~3 días)

### T3.1 — Nuevo patrón: `SemanticResolver` trait

```rust
// chronos-domain/src/semantic/resolver.rs
pub trait SemanticResolver: Send + Sync {
    fn language(&self) -> Language;
    fn resolve(&self, raw: &RawKernelEvent, ctx: &ResolverContext) -> Option<SemanticEvent>;
}

pub struct ResolverContext<'a> {
    pub symbol_table: &'a SymbolTable,
    pub source_map: &'a SourceMap,
    pub pid: u32,
}

// Ejemplo: RawKernelEvent (syscall nr=2) → FileOpened { path: "/tmp/x", mode: Read }
```

### T3.2 — Python: `PythonSemanticResolver`

```rust
// chronos-python/src/semantic_resolver.rs
pub struct PythonSemanticResolver { /* usa debugpy para resolve variable names */ }

impl SemanticResolver for PythonSemanticResolver {
    fn language(&self) -> Language { Language::Python }

    fn resolve(&self, raw: &RawKernelEvent, ctx: &ResolverContext) -> Option<SemanticEvent> {
        match raw {
            RawKernelEvent::FunctionEntry { frame } => {
                // Consultar PyFrameObject via /proc/pid/mem
                // Decodificar nombre de función Python (no mangled), argumentos
                Some(SemanticEvent::PythonFunctionCalled { name, args, locals })
            }
            RawKernelEvent::ExceptionThrown => {
                Some(SemanticEvent::PythonException { exc_type, exc_value, traceback })
            }
            _ => None,
        }
    }
}
```

### T3.3 — Java: `JavaSemanticResolver`

```rust
// chronos-java/src/semantic_resolver.rs
// Usa JDWP para obtener class names, method names, object IDs
// Convierte: raw uprobe at 0x... → "UserService.login(UserDTO)"
```

### T3.4 — Go: `GoSemanticResolver`

```rust
// chronos-go/src/semantic_resolver.rs
// Usa Delve para resolve goroutine info, interface values
// Convierte: runtime.goexit → "goroutine 12 exited"
```

---

## FASE 4 — API de Inyección de Probes (eBPF) (Semana 3, ~2 días)

### T4.1 — `probe_inject` MCP Tool

```rust
#[tool(name = "probe_inject")]
async fn probe_inject(
    session_id: String,
    target: ProbeTarget,
    event_mask: EventMask,
) -> Result<ProbeHandle, Error>;

pub enum ProbeTarget {
    Function { binary: String, name_pattern: String },
    Syscall { number: u32 },
    Address { binary: String, offset: u64 },
}

pub struct EventMask {
    pub entry: bool,
    pub exit: bool,
    pub syscalls: bool,
    pub memory: bool,
}
```

**Flujo interno:**
1. LLM llama `probe_inject(session_id, Function { binary: "/bin/app", name_pattern: "process_*" }, entry=true)`
2. `EbpfAdapter` hace `attach_uprobe()` para cada símbolo que matchea
3. Los eventos van al `EventBus`
4. El `SemanticResolver` correspondiente traduce a JSON legible

---

## FASE 5 — Herramientas MCP Faltantes (Semana 3, ~1 día)

### T5.1 — Implementar `compare_sessions`

**Código existente:** `chronos-store/src/diff.rs` tiene `TraceDiff::compare()` listo.

```rust
#[tool(name = "compare_sessions")]
async fn compare_sessions(
    session_a: String,
    session_b: String,
) -> Result<SessionDiff, Error>;
```

### T5.2 — Implementar `performance_regression_audit`

```rust
#[tool(name = "performance_regression_audit")]
async fn performance_regression_audit(
    baseline_session_id: String,
    target_session_id: String,
    top_n: Option<usize>,
) -> Result<PerformanceReport, Error>;
```

### T5.3 — Reescribir `debug_run` como 3 tools

**Antes** (548 LOC, blocking):
```rust
debug_run(program, args, trace_syscalls, capture_registers, cwd, auto_save, ...)
```

**Después** (3 tools, non-blocking):
```rust
// 1. Start probe (non-blocking)
#[tool(name = "probe_start")]
async fn probe_start(config: ProbeConfig) -> SessionId;

// 2. Stop probe
#[tool(name = "probe_stop")]
async fn probe_stop(session_id: SessionId) -> CaptureResult;

// 3. Freeze + build indices (solo cuando LLM quiere analizar)
#[tool(name = "session_snapshot")]
async fn session_snapshot(session_id: SessionId) -> IndexesBuilt;
```

---

## FASE 6 — Limpieza Final (Semana 4, ~1 día)

### T6.1 — Eliminar `EventType::BreakpointHit` y `WatchTrigger`

Estos solo existían por el `BreakpointManager` interactivo. Verificar que ningún código los emite después de T0.2.

```rust
// En chronos-domain/src/trace/event.rs:
// Eliminar: BreakpointHit, WatchTrigger de EventType
//替代为: ProbeHit (para eBPF uprobe triggers)
```

### T6.2 — Verificar que todo compila

```bash
cargo check --all 2>&1 | grep -v "warning:"
# Debe compilar limpio con 0 errores
```

### T6.3 — Runs tests

```bash
cargo test --all -- --test-threads=4
# Objetivo: >95% passing
```

---

## Orden de Ejecución Recomendado

```
Semana 1:
  LUN → T0.1 (delete sandbox) → T0.2 (delete breakpoints)
  MAR → T0.3 (delete format) → T0.4 (delete eval_dispatcher)
  MIÉ → T1.1 (ProbeBackend trait) → T1.2 (EventBus)
  JUE → T1.3 (NativeProbeBackend)
  VIE → T2.1 (Tripwire system)

Semana 2:
  LUN → T3.1 (SemanticResolver trait) → T3.2 (Python resolver)
  MAR → T3.3 (Java resolver) → T3.4 (Go resolver)
  MIÉ → T4.1 (probe_inject API) → T5.1 (compare_sessions)
  JUE → T5.2 (perf regression) → T5.3 (debug_run split)
  VIE → T6.1 (cleanup EventType) → T6.2 (cargo check)

Semana 3:
  Testing, edge cases, e2e verification
```

---

## Métricas Objetivo

| Métrica | Antes | Después |
|---|---|---|
| Total LOC | 37,412 | ~27,000 (-28%) |
| Crates | 17 | 14 (-3 eliminados) |
| MCP tools | 27 (con 3 huérfanos) | ~22 (más enfocados) |
| `TraceAdapter` traits | 2 (duplicated) | 1 (`ProbeBackend`) |
| Dependencias circulares | alguna (verificar) | 0 |
| `debug_run` LOC | 548 | ~150 (split en 3) |
| eBPF ring buffer | disconnected | conectado a EventBus |

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Mitigación |
|---|---|---|
| Romper `chronos-e2e` tests al borrar sandbox | Media | Revisar tests antes de borrar; mantenerlos como `#[ignore]` si son solo integración CI |
| `ProbeBackend` trait change rompe todos los adaptadores | Alta | Hacer el cambio incremental: 1) definir nuevo trait, 2) implementar en cada adapter uno por uno, 3) eliminar old trait |
| `EventBus` se llena muy rápido para high-throughput | Baja | Configurable capacity + watermark alerts via `Tripwire` |
| LLM no sabe qué probes injectar | Media | Crear `probe_suggest(target_binary)` que usa ELF/DWARF para sugerir funciones interesante |