# rec-c3-hexagonal-closure — Etapa A.1 vault row

## Etapa

**A.1 — Define `NativeProbeController` trait in `chronos-domain`**

## Identidad

- **cycle_id**: rec-c3-hexagonal-closure
- **stage**: A.1
- **commit**: `638b8481`
- **scope**: `crates/chronos-domain/src/ports/probe.rs` (new trait + outcome types)
  + `crates/chronos-domain/src/ports/mod.rs` (re-exports)
  + `crates/chronos-domain/tests/probe_ports.rs` (4 new LSP tests)
- **diff_stat**: 3 files, +244 / -4

## Criterios de aceptación

| Criterio | Estado | Evidencia |
|---|---|---|
| Trait `NativeProbeController` declarado en `chronos-domain/src/ports/probe.rs` | ✅ | líneas 212-274 |
| Trait exportado en `chronos-domain/src/ports/mod.rs` | ✅ | línea 47-49 |
| Sin accessor `backend() -> &dyn ProbeBackend` (audit §4.5 S4) | ✅ | confirmado por inspección |
| Tipos `AdvanceOutcome` y `StepOutcome` declarados | ✅ | líneas 205, 210 |
| `Send + Sync + Debug` required | ✅ | línea 222 |
| Tests LSP pasan | ✅ | 4 nuevos + 8 pre-existentes = 12/12 |
| T0 (fmt + clippy) verde | ✅ | exit 0 |
| No `#[ignore]`, no `--skip`, no waiver | ✅ | confirmado |

## Comandos ejecutados

```
cargo fmt --all -- --check                              # exit 0
cargo clippy -p chronos-domain --all-targets -D warnings # exit 0
cargo test -p chronos-domain --test probe_ports --no-fail-fast
  → test result: ok. 12 passed; 0 failed; 0 ignored
```

## Decisiones de diseño tomadas

1. **Trait nuevo en lugar de ampliar `ProbeController`**: `ProbeController` es lifetime-focused (registry attach/detach); `NativeProbeController` es capability-focused (attach/advance/step/log/resolver). Mezclar ambos en un solo trait haría crecer la superficie y reintroduciría el smell audit §4.5 S4.
2. **Tuplas primitivas para advance/step en lugar de mover `AdvanceOutput`/`StepOutput`**: mantener los tipos de output en `chronos-services::output` donde viven hoy; la conversión tuple→struct ocurre en `ProbeService`. Esto evita mover 2 structs y reduce el scope de A.1 a sólo el trait.
3. **`execution_log()` y `resolver_pipeline()` devuelven `Arc<dyn ...>`**: la implementación actual de `NativeProbeBackend` ya usa `Arc<dyn ExecutionLogProvider>` internamente (`attach_execution_log` toma `Arc<dyn ExecutionLogProvider>`). Mantener el `Arc` evita cloning innecesario.
4. **`resolver_pipeline()` como `Arc<dyn SemanticResolver>` (no `Arc<dyn ResolverPipeline>`)**: `ResolverPipeline` es un struct concreto de `chronos-domain` con método `add_resolver`; el port sólo necesita "ejecutar resolución", no añadir resolvers. Exponer `SemanticResolver` (trait) cumple ISP.

## Próxima etapa

**A.2 — Implement `NativeProbeController` en `chronos-native`**

Estructura:
- `crates/chronos-native/src/native_probe_controller.rs` (NEW)
- Struct `NativeProbeControllerImpl { backend: NativeProbeBackend, session_id: SessionId }`
- `impl NativeProbeController for NativeProbeControllerImpl` reenviando a `NativeProbeBackend`:
  - `attach_to_pid(pid, config)` → `backend.attach_probe(...)` (returns CaptureSession)
  - `stop()` → `backend.stop_probe(&self.session)`
  - `advance()` → `backend.advance(&self.session).map(|_| (true, None, true))`
  - `step()` → `backend.step(&self.session).map(|_| (true, Some("single-step")))`
  - `execution_log()` → `backend.execution_log()`
  - `resolver_pipeline()` → extrae de `backend.clone_resolver_pipeline()` o devuelve `None`

Verificaciones:
- T0 + T1 (`cargo test -p chronos-native --lib`) GREEN
- LSP tests adicionales que comparan comportamiento del impl vs mock
