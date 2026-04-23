# 03 — Orientación: Primera Pasada Obligatoria

Después de `debug_run`, SIEMPRE ejecuta estas tres herramientas en paralelo. Nunca saltes esta etapa.

## Por Qué Orientación Es Obligatoria

Sin orientación, el agente está ciego. No sabe:
- Si el programa crasheó o completó normalmente
- Cuántos threads tuvo
- Cuántos eventos se capturaron
- Cuáles funciones fueron más calientes

Orientación le da al agente un mapa del territorio antes de intentar cualquier drill-down.

## get_execution_summary

Resumen ejecutivo de la ejecución. Responde: "¿Qué pasó en términos generales?"

**Parámetros:** `session_id` (string, requerido)

**¿Qué responde?**
- Total de eventos capturados
- Número de threads
- Eventos por tipo (function_entry, syscall, etc.)
- Issues detectados (crashes, signals, excepciones)
- Top funciones por call count

**Ejemplo de llamada:**
```json
{
  "tool": "get_execution_summary",
  "params": { "session_id": "sess_a1b2c3" }
}
```

**Campos de respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "total_events": 142857,
  "thread_count": 8,
  "event_breakdown": {
    "function_entry": 45000,
    "function_exit": 44800,
    "syscall_enter": 22000,
    "syscall_exit": 21800,
    "memory_write": 8200,
    "variable_write": 1057
  },
  "issues": [
    { "type": "signal", "signal_name": "SIGABRT", "event_id": 142001 }
  ],
  "top_functions": [
    { "function": "process_request", "call_count": 14200 },
    { "function": "db_query", "call_count": 8900 }
  ]
}
```

**Parallel-safe:** ✅ Sí

---

## debug_get_saliency_scores

Scores de saliencia [0.0–1.0] por función. Responde: "¿Dónde está el cuello de botella?"

Las funciones con score alto consumieron una proporción desproporcionada de ciclos de CPU relativo a otras funciones.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `limit` | usize | `20` | Máximo de funciones a scorear |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_get_saliency_scores",
  "params": { "session_id": "sess_a1b2c3", "limit": 10 }
}
```

**Campos de respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "scored_functions": [
    { "function": "compress_image", "saliency_score": 0.94, "cpu_cycles": 4500000, "call_count": 1 },
    { "function": "db_query", "saliency_score": 0.78, "cpu_cycles": 1200000, "call_count": 8900 },
    { "function": "serialize_json", "saliency_score": 0.45, "cpu_cycles": 340000, "call_count": 4200 }
  ]
}
```

**Interpretación:**
- `0.9+` — Función crítica, casi todo el tiempo es aquí
- `0.5–0.9` — Función significativa
- `0.1–0.5` — Función moderada
- `< 0.1` — Función menor

**Parallel-safe:** ✅ Sí

---

## list_threads

Lista todos los thread IDs en el trace. Esencial para filtrar eventos por thread después.

**Parámetros:** `session_id` (string, requerido)

**Ejemplo de llamada:**
```json
{
  "tool": "list_threads",
  "params": { "session_id": "sess_a1b2c3" }
}
```

**Campos de respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "thread_count": 8,
  "thread_ids": [1, 2, 3, 4, 5, 6, 7, 8]
}
```

**Parallel-safe:** ✅ Sí

**¿Por qué importa?** Muchos bugs son específicos de un thread. Después de list_threads, puedes filtrar `query_events` por `thread_id` para ver solo la actividad de un thread específico.

---

## Patrón de Uso: Orientación Completa en Paralelo

```json
[
  { "tool": "get_execution_summary",      "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_get_saliency_scores",  "params": { "session_id": "sess_a1b2c3", "limit": 10 } },
  { "tool": "list_threads",               "params": { "session_id": "sess_a1b2c3" } }
]
```

Estas tres llamadas son independientes y retornan en ~50ms cada una. En paralelo, la orientación completa toma ~50ms, no ~150ms.

---

## Qué Hacer Después de Orientación

| Orientación revela... | Siguiente paso |
|----------------------|----------------|
| `issues.signal_name` presente | `debug_find_crash` |
| Threads > 1 | `debug_detect_races` |
| `saliency_score` > 0.8 en función | `debug_expand_hotspot` con esa función |
| `total_events` > 1M sin issues | Considerar `trace_syscalls: false` y re-capturar |
| Thread count muy alto | `query_events` filtrado por thread específico |

---

## Errores Comunes

### Error: "Session not found or not finalized"

La sesión de `debug_run` aún no está lista. Si usaste `background: true`, espera o intenta de nuevo.

### Error: "No events captured"

El programa terminó demasiado rápido o el tracer no pudo attaches. Verifica que el binary existe y tiene permisos de ejecución.

---

## Resumen

| Herramienta | Pregunta que responde | Parallel-safe |
|------------|----------------------|----------------|
| `get_execution_summary` | ¿Qué pasó en general? | ✅ |
| `debug_get_saliency_scores` | ¿Dónde está el cuello de botella? | ✅ |
| `list_threads` | ¿Cuántos threads y cuáles IDs? | ✅ |

**Regla de oro:** Siempre ejecuta las tres en paralelo después de `debug_run`, sin excepción.
