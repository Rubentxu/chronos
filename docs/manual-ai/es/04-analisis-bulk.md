# 04 — Análisis Bulk: Respuestas Completas de Una Sola Llamada

Estas son las herramientas más valiosas del toolkit Chronos. Cada una responde una pregunta compleja con UNA llamada — sin loops, sin iteraciones, sin sequencias de llamadas.

## debug_find_crash — ¿Dónde Crasheó?

Encuentra el último evento antes de una señal fatal (SIGSEGV, SIGABRT, etc.). Devuelve el punto de crash y el call stack completo.

**Parámetros:** `session_id` (string, requerido)

**Ejemplo de llamada:**
```json
{
  "tool": "debug_find_crash",
  "params": { "session_id": "sess_a1b2c3" }
}
```

**Respuesta (cuando hay crash):**
```json
{
  "session_id": "sess_a1b2c3",
  "crash_found": true,
  "signal_name": "SIGABRT",
  "crash_event_id": 142001,
  "crash_timestamp_ns": 5892341002,
  "crash_function": "process_request",
  "crash_address": "0x7f8a2c3d4e5f",
  "thread_id": 1,
  "backtrace": [
    { "depth": 0, "function": "process_request", "address": "0x55a3b2c1d0e0" },
    { "depth": 1, "function": "handle_connection", "address": "0x55a3b2c1d100" },
    { "depth": 2, "function": "main_loop", "address": "0x55a3b2c1d200" }
  ],
  "registers_at_crash": {
    "rip": "0x7f8a2c3d4e5f",
    "rsp": "0x7ffd8a9b0",
    "rax": 0
  }
}
```

**Respuesta (cuando no hay crash):**
```json
{
  "session_id": "sess_a1b2c3",
  "crash_found": false,
  "end_reason": "exited(0)"
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** El primer análisis después de orientación cuando el `get_execution_summary` muestra un issue de tipo signal.

---

## debug_detect_races — ¿Hay Condiciones de Carrera?

Detecta TODAS las condiciones de carrera en la sesión. Una condición de carrera existe cuando dos threads escriben a la misma dirección de memoria dentro de un threshold de nanosegundos.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `threshold_ns` | u64 | `100` | Ventana de detección en nanosegundos |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_detect_races",
  "params": { "session_id": "sess_a1b2c3", "threshold_ns": 100 }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "races_detected": 3,
  "threshold_ns": 100,
  "races": [
    {
      "address": "0x7f8a2c3d4e5f",
      "thread_a": 2,
      "thread_b": 5,
      "timestamp_a_ns": 1234567890,
      "timestamp_b_ns": 1234567945,
      "delta_ns": 55,
      "write_value_a": "0x00000001",
      "write_value_b": "0x00000000"
    }
  ]
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** Programas multithread donde orientación mostró múltiples threads activos. El threshold de 100ns es agresivo — para sistemas más lentos, considera 1000ns.

---

## debug_expand_hotspot — ¿Cuáles Son las Funciones Más Calientes?

Devuelve las N funciones con mayor tiempo de CPU acumulado, desglosado por call count y tiempo total.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `top_n` | usize | `10` | Número de funciones a devolver |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_expand_hotspot",
  "params": { "session_id": "sess_a1b2c3", "top_n": 5 }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "hotspots": [
    {
      "rank": 1,
      "function": "compress_image",
      "call_count": 1,
      "total_cpu_cycles": 4500000,
      "avg_cpu_cycles_per_call": 4500000,
      "is_leaf": true
    },
    {
      "rank": 2,
      "function": "db_query",
      "call_count": 8900,
      "total_cpu_cycles": 1200000,
      "avg_cpu_cycles_per_call": 135,
      "is_leaf": false
    },
    {
      "rank": 3,
      "function": "handle_request",
      "call_count": 14200,
      "total_cpu_cycles": 800000,
      "avg_cpu_cycles_per_call": 56,
      "is_leaf": false
    }
  ]
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** Latencia alta o CPU alto. Identifica exactamente qué función es el bottleneck.

---

## performance_regression_audit — ¿Qué Cambió Entre Versiones?

Compara dos sesiones (baseline vs actual) y devuelve un reporte de regresión con severity scoring.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `baseline_session_id` | string | — | Sesión de referencia |
| `target_session_id` | string | — | Sesión a comparar |
| `top_n` | `Option<usize>` | `20` | Funciones top a comparar |

**Ejemplo de llamada:**
```json
{
  "tool": "performance_regression_audit",
  "params": {
    "baseline_session_id": "sess_baseline_v2",
    "target_session_id": "sess_current_pr",
    "top_n": 20
  }
}
```

**Respuesta:**
```json
{
  "baseline_session_id": "sess_baseline_v2",
  "target_session_id": "sess_current_pr",
  "regression_score": 0.23,
  "severity": "high",
  "total_regressions": 5,
  "total_improvements": 2,
  "critical_count": 1,
  "top_regressions": [
    {
      "function": "process_request",
      "baseline_cycles": 500000,
      "target_cycles": 890000,
      "regression_pct": 78,
      "severity": "critical"
    }
  ],
  "top_improvements": [
    {
      "function": "serialize_json",
      "baseline_cycles": 340000,
      "target_cycles": 210000,
      "improvement_pct": 38
    }
  ]
}
```

**Interpretación de severity:**

| severity | regression_score | Acción |
|----------|-----------------|--------|
| `critical` | > 0.5 | Bloquear merge inmediatamente |
| `high` | 0.2–0.5 | Requiere review obligatorio |
| `medium` | 0.1–0.2 | Warning, revisar si es inesperado |
| `low` | < 0.1 | Aceptable |

**Parallel-safe:** ✅ Sí

**Caso de uso:** CI/CD regression gates, comparaciones de staging vs production, análisis pre/post deploy.

---

## debug_call_graph — ¿Cuál Es la Estructura de Llamadas?

Construye el grafo completo de llamadas de la sesión, mostrando quién llama a quién y cuántos veces.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `max_depth` | usize | `10` | Profundidad máxima del grafo |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_call_graph",
  "params": { "session_id": "sess_a1b2c3", "max_depth": 6 }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "max_depth": 6,
  "nodes": [
    {
      "function": "main",
      "call_count": 1,
      "callees": ["process_request", "init"],
      "callers": []
    },
    {
      "function": "process_request",
      "call_count": 14200,
      "callees": ["db_query", "serialize_json"],
      "callers": ["main"]
    }
  ],
  "edges": [
    { "caller": "main", "callee": "process_request", "count": 14200 },
    { "caller": "process_request", "callee": "db_query", "count": 8900 }
  ]
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** Entender la arquitectura de llamadas. Identificar funciones de alto nivel que llaman a muchas funciones hoja.

---

## Resumen de Análisis Bulk

| Herramienta | Pregunta que responde | Parallel-safe |
|------------|----------------------|----------------|
| `debug_find_crash` | ¿Dónde y por qué crasheó? | ✅ |
| `debug_detect_races` | ¿Hay condiciones de carrera? | ✅ |
| `debug_expand_hotspot` | ¿Cuáles funciones consumen más CPU? | ✅ |
| `performance_regression_audit` | ¿Qué cambió entre baseline y actual? | ✅ |
| `debug_call_graph` | ¿Cuál es la estructura de llamadas? | ✅ |

**Regla de oro:** Después de orientación, ejecuta TODOS los análisis bulk en paralelo — son independientes entre sí y la respuesta combinada da una imagen completa.
