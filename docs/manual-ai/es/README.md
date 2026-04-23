# Chronos MCP — Manual de Debugging Time-Travel para Agentes IA

## ¿Qué es Chronos?

Chronos es un sistema de debugging time-travel expuesto como herramientas MCP (Model Context Protocol), diseñado desde cero para agentes IA — no para humanos. Captura un trace completo y congelado de la ejecución de un programa en una sola operación, y permite consultas paralelas ilimitadas sobre ese trace.

## El Paradigma Central

```
Debugging tradicional (humano):
  breakpoint → ejecutar → pausar → inspeccionar → step → repetir → repetir → repetir

Debugging AI-native (Chronos):
  debug_run() → UNA sesión congelada → consultar TODO en paralelo → hecho
```

Esta diferencia no es cosmética. Cambia completamente cómo piensas sobre debugging.

Un debugger humano trabaja interactivamente porque los humanos solo pueden mantener unas pocas cosas en mente. Un agente IA puede issuing dozens of analysis queries simultaneously y sintetizar todos los resultados en una pasada. Chronos está construido para este modelo.

## El Patrón "Una Captura, N Análisis"

```
                    ┌─────────────────┐
                    │   debug_run()   │
                    │  (una captura)  │
                    └────────┬────────┘
                             │ session_id
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
   get_execution_    debug_get_      list_threads()
      summary()    saliency_scores()
              │              │              │
              └──────────────┼──────────────┘
                             │ orientación completada
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
   debug_find_crash() debug_detect_  debug_expand_
                        races()       hotspot()
              │              │              │
              └──────────────┼──────────────┘
                             │ análisis bulk completado
                    (drill-in en hallazgos)
```

Todas las consultas en el mismo nivel se ejecutan en paralelo. La sesión es inmutable — ninguna consulta la modifica.

## Inicio Rápido

### 1. Capturar un trace

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/path/to/my-binary",
    "args": ["--config", "app.toml"],
    "trace_syscalls": true,
    "capture_registers": true
  }
}
```

La respuesta incluye `session_id` (ej. `"sess_a1b2c3"`).

### 2. Ejecutar herramientas de orientación EN PARALELO

```json
[
  { "tool": "get_execution_summary",     "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "list_threads",              "params": { "session_id": "sess_a1b2c3" } }
]
```

### 3. Ejecutar análisis bulk EN PARALELO según síntomas

```json
[
  { "tool": "debug_find_crash",   "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_detect_races", "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_call_graph",   "params": { "session_id": "sess_a1b2c3" } }
]
```

### 4. Drill-down en hallazgos específicos

Usar `query_events`, `get_call_stack`, `debug_get_variables`, etc. — solo después de saber qué buscar.

## Capas de Herramientas de un Vistazo

| Capa | Herramientas | Cuándo |
|------|-------------|--------|
| **Captura** | `debug_run`, `debug_attach` | Siempre primero |
| **Orientación** | `get_execution_summary`, `debug_get_saliency_scores`, `list_threads` | Inmediatamente después de captura, siempre en paralelo |
| **Análisis bulk** | `debug_find_crash`, `debug_detect_races`, `debug_expand_hotspot`, `performance_regression_audit`, `debug_call_graph` | Después de orientación, en paralelo según síntoma |
| **Forense** | `forensic_memory_audit`, `inspect_causality`, `debug_find_variable_origin` | Después de que bulk identifica dirección/variable sospechosa |
| **Drill-down** | `query_events`, `get_call_stack`, `evaluate_expression`, `debug_get_variables`, `state_diff`, `debug_diff`, `get_event` | Después de que forense/bulk estrecha el alcance |
| **Acceso raw** | `debug_get_memory`, `debug_get_registers`, `debug_analyze_memory` | Raramente — solo para investigación a nivel hardware |
| **Gestión de sesión** | `save_session`, `load_session`, `list_sessions`, `delete_session`, `drop_session`, `compare_sessions` | CI/CD, multi-agente, persistencia |

## Lenguajes Soportados

| Lenguaje | Mecanismo de captura |
|----------|---------------------|
| Native / C / C++ / Rust | ptrace |
| Java | JDWP |
| Python | DAP / debugpy |
| JavaScript / Node.js | CDP (Chrome DevTools Protocol) |
| Go | Delve DAP |
| eBPF | aya uprobes |

## Estructura del Manual

- **[01-patron-central.md](01-patron-central.md)** — Explicación profunda del paradigma AI-native
- **[02-captura.md](02-captura.md)** — debug_run y debug_attach en detalle completo
- **[03-orientacion.md](03-orientacion.md)** — Herramientas de primera pasada obligatorias
- **[04-analisis-bulk.md](04-analisis-bulk.md)** — Herramientas de respuesta bulk
- **[05-forense.md](05-forense.md)** — Herramientas de investigación causal
- **[06-drill-down.md](06-drill-down.md)** — Herramientas de inspección dirigida
- **[07-acceso-raw.md](07-acceso-raw.md)** — Acceso a memoria y registros a bajo nivel
- **[08-gestion-sesiones.md](08-gestion-sesiones.md)** — Persistencia, CI/CD, multi-agente
- **[09-multi-lenguaje.md](09-multi-lenguaje.md)** — Configuración específica por lenguaje
- **[10-anti-patrones.md](10-anti-patrones.md)** — Qué NO hacer
- **[11-ejemplos-prompts.md](11-ejemplos-prompts.md)** — 20+ ejemplos completos de workflows de agente
