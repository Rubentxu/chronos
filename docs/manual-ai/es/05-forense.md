# 05 — Forense: Investigación Causal

Las herramientas de forense responden a "¿Cómo llegó este valor aquí?" o "¿Quién tocó esta dirección y cuándo?". Se usan DESPUÉS de que el análisis bulk identifica una dirección o variable específica de interés.

## forensic_memory_audit — Auditoría Completa de una Dirección

Devuelve TODAS las escrituras a una dirección de memoria específica, con timestamps, valores escritos, y el contexto de cada escritura.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `address` | u64 | — | Dirección de memoria a auditar (decimal) |
| `limit` | usize | `100` | Máximo de escrituras a devolver |

**Ejemplo de llamada:**
```json
{
  "tool": "forensic_memory_audit",
  "params": {
    "session_id": "sess_a1b2c3",
    "address": 140734193800032,
    "limit": 50
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "address": "0x7f8a2c3d4e5f",
  "writes_count": 23,
  "writes": [
    {
      "event_id": 45001,
      "timestamp_ns": 1234567890,
      "thread_id": 2,
      "function": "init_buffer",
      "value_written": "0x00000001",
      "operation": "write"
    },
    {
      "event_id": 89002,
      "timestamp_ns": 2345678901,
      "thread_id": 5,
      "function": "update_state",
      "value_written": "0x00000000",
      "operation": "write"
    }
  ]
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** Memoria corrupta, buffer overflow, use-after-free. Cuando `debug_find_crash` o `debug_detect_races` identifica una dirección sospechosa.

---

## inspect_causality — Historial Causal Completo

Reconstruye el historial causal completo de una dirección de memoria: todas las lecturas y escrituras, sus timestamps, y la cadena de dependencias.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `address` | u64 | — | Dirección a inspeccionar |
| `limit` | usize | `100` | Máximo de entradas a devolver |

**Ejemplo de llamada:**
```json
{
  "tool": "inspect_causality",
  "params": {
    "session_id": "sess_a1b2c3",
    "address": 140734193800032,
    "limit": 100
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "address": "0x7f8a2c3d4e5f",
  "causality_entries": [
    {
      "event_id": 45001,
      "timestamp_ns": 1234567890,
      "thread_id": 2,
      "type": "write",
      "function": "init_buffer",
      "value": "0x00000001",
      "derived_from": []
    },
    {
      "event_id": 67001,
      "timestamp_ns": 1800000000,
      "thread_id": 2,
      "type": "read",
      "function": "check_state",
      "value": "0x00000001",
      "derived_from": [45001]
    },
    {
      "event_id": 89002,
      "timestamp_ns": 2345678901,
      "thread_id": 5,
      "type": "write",
      "function": "update_state",
      "value": "0x00000000",
      "derived_from": [67001]
    }
  ]
}
```

**El campo `derived_from`:** Muestra qué eventos anteriores influenciaron el valor. Esto permite reconstruir cómo un valor "fluyó" a través del programa.

**Parallel-safe:** ✅ Sí

**Caso de uso:** Investigación de linaje de datos. ¿De dónde vino este valor? ¿Qué operaciones lo transformaron?

---

## debug_find_variable_origin — Linaje de Variable

Trazа la historia completa de mutaciones de una variable específica por nombre.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `variable_name` | string | — | Nombre exacto de la variable |
| `limit` | usize | `100` | Máximo de mutaciones a devolver |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_find_variable_origin",
  "params": {
    "session_id": "sess_a1b2c3",
    "variable_name": "total",
    "limit": 50
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "variable_name": "total",
  "mutations": [
    {
      "event_id": 1234,
      "timestamp_ns": 1000000000,
      "function": "init",
      "action": "declared",
      "value": "0",
      "scope": "local"
    },
    {
      "event_id": 5678,
      "timestamp_ns": 2000000000,
      "function": "process_item",
      "action": "write",
      "value": "1",
      "scope": "local"
    },
    {
      "event_id": 9012,
      "timestamp_ns": 3000000000,
      "function": "process_item",
      "action": "write",
      "value": "-1",
      "scope": "local"
    }
  ]
}
```

**Parallel-safe:** ✅ Sí

**Caso de uso:** "La variable `total` se volvió -1 en algún punto. ¿Cómo llegó ahí?" El agente busca la mutación que estableció el valor incorrecto y la cadena de llamadas que llevó a ella.

---

## Cuándo Usar Herramientas de Forense

```
Orientación
    ↓
Análisis bulk (debug_find_crash, debug_detect_races, etc.)
    ↓
¿Identificaste una dirección o variable de interés?
    ↓
Sí → forensic_memory_audit / inspect_causality / debug_find_variable_origin
No → Drill-down con query_events
```

**Ejemplo de flujo completo:**

1. `get_execution_summary` → encuentra signal SIGSEGV
2. `debug_find_crash` → crash en dirección 0x7f8a2c3d4e5f, thread 3
3. `forensic_memory_audit` de esa dirección → encuentra 15 escrituras, la última de thread 5
4. `inspect_causality` de esa dirección → reconstruye la cadena completa
5. `debug_find_variable_origin` del buffer asociado → encuentra el write corrupto

---

## Diferencias Entre las Tres Herramientas

| Herramienta | Entrada | Salida | Caso de uso |
|------------|---------|--------|-------------|
| `forensic_memory_audit` | Dirección de memoria | Solo escrituras | Buffer overflow, use-after-free |
| `inspect_causality` | Dirección de memoria | Lecturas + escrituras + dependencias | Linaje de datos, data flow |
| `debug_find_variable_origin` | Nombre de variable | Solo mutaciones de esa variable | Variable con valor incorrecto |

---

## Resumen

| Herramienta | Pregunta que responde | Parallel-safe |
|------------|----------------------|----------------|
| `forensic_memory_audit` | ¿Quién escribió en esta dirección? | ✅ |
| `inspect_causality` | ¿Cuál es el historial completo de esta dirección? | ✅ |
| `debug_find_variable_origin` | ¿Cómo mutate esta variable a lo largo del tiempo? | ✅ |

**Regla de oro:** Las herramientas de forense vienen DESPUÉS de que el análisis bulk identifica algo específico. No las llames al azar — tienen parámetros concretos (addresses, variable names) que deben venir de hallazgos previos.
