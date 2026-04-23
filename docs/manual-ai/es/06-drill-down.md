# 06 — Drill-Down: Inspección Dirigida

Las herramientas de drill-down se usan para inspeccionar ventanas específicas de eventos o tiempo. Vienen DESPUÉS de que orientación y análisis bulk identifican algo concreto. No son herramientas de descubrimiento — son herramientas de confirmación.

## query_events — Consulta Filtrada de Eventos

La herramienta más versátil. Consulta eventos con filtros específicos.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_types` | `Vec<String>` | — | Filtrar por tipo: `"function_entry"`, `"syscall_enter"`, etc. |
| `thread_id` | `Option<u64>` | — | Filtrar por thread específico |
| `timestamp_start` | `Option<u64>` | — | Inicio del rango (ns, inclusivo) |
| `timestamp_end` | `Option<u64>` | — | Fin del rango (ns, exclusivo) |
| `function_pattern` | `Option<String>` | — | Patrón glob para nombre de función |
| `limit` | usize | `100` | Máximo de eventos a devolver |
| `offset` | usize | `0` | Número de eventos a saltar (paginado) |

**Tipos de eventos disponibles:**
- `function_entry` / `function_exit`
- `syscall_enter` / `syscall_exit`
- `variable_write` / `memory_write`
- `signal_delivered`
- `breakpoint_hit`
- `thread_create` / `thread_exit`
- `exception_thrown`

**Ejemplo: Todos los eventos de función "process_" en thread 3**
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_a1b2c3",
    "event_types": ["function_entry", "function_exit"],
    "thread_id": 3,
    "function_pattern": "process_*",
    "limit": 50
  }
}
```

**Ejemplo: Syscalls en una ventana de tiempo específica**
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_a1b2c3",
    "event_types": ["syscall_enter", "syscall_exit"],
    "timestamp_start": 5000000000,
    "timestamp_end": 5100000000,
    "limit": 100
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "total_matching": 142,
  "returned_count": 50,
  "next_offset": 50,
  "events": [
    {
      "event_id": 1234,
      "timestamp_ns": 5012345678,
      "thread_id": 3,
      "type": "function_entry",
      "function": "process_request",
      "address": "0x55a3b2c1d0e0"
    }
  ]
}
```

**⚠️ Regla CRÍTICA:** SIEMPRE usa al menos un filtro. Sin filtros, devuelve solo 100 eventos arbitrarios de los millones capturados.

**Parallel-safe:** ✅ Sí

---

## get_call_stack — Reconstruir Stack en un Evento

Devuelve el call stack completo en un event_id específico.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id` | u64 | — | Evento en el cual reconstruir el stack |

**Ejemplo de llamada:**
```json
{
  "tool": "get_call_stack",
  "params": { "session_id": "sess_a1b2c3", "event_id": 1234 }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "at_event_id": 1234,
  "depth": 12,
  "frames": [
    { "depth": 0, "function": "crash_function", "file": "src/main.rs", "line": 42, "address": "0x55a3b2c1d0e0" },
    { "depth": 1, "function": "handle_connection", "file": "src/server.rs", "line": 100, "address": "0x55a3b2c1d100" },
    { "depth": 2, "function": "main_loop", "file": "src/main.rs", "line": 200, "address": "0x55a3b2c1d200" }
  ]
}
```

**Parallel-safe:** ✅ Sí

---

## evaluate_expression — Evaluar Expresión Aritmética

Evalúa una expresión aritmética usando variables locales disponibles en el contexto de un event_id.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id` | u64 | — | Evento en cuyo contexto evaluar |
| `expression` | string | — | Expresión aritmética (ej. `"x + y * 2"`) |

**Ejemplo de llamada:**
```json
{
  "tool": "evaluate_expression",
  "params": {
    "session_id": "sess_a1b2c3",
    "event_id": 1234,
    "expression": "offset + length * 2"
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "event_id": 1234,
  "expression": "offset + length * 2",
  "result": 42,
  "variables": {
    "offset": 10,
    "length": 16
  }
}
```

**Parallel-safe:** ✅ Sí

---

## debug_get_variables — Variables en Scope

Devuelve todas las variables visibles en el scope de un event_id.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id` | u64 | — | Evento en el cual obtener variables |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_get_variables",
  "params": { "session_id": "sess_a1b2c3", "event_id": 1234 }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "event_id": 1234,
  "variables": [
    { "name": "request_id", "type": "u64", "value": "0x55a3b2c1" },
    { "name": "total", "type": "i32", "value": "-1" },
    { "name": "buffer", "type": "*const u8", "value": "0x7f8a2c3d4e5f" }
  ]
}
```

**Parallel-safe:** ✅ Sí

---

## state_diff — Comparar Registros Entre Timestamps

Compara los valores de registros de CPU entre dos timestamps.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `timestamp_a` | u64 | — | Primer timestamp (ns) |
| `timestamp_b` | u64 | — | Segundo timestamp (ns) |

**Ejemplo de llamada:**
```json
{
  "tool": "state_diff",
  "params": {
    "session_id": "sess_a1b2c3",
    "timestamp_a": 5000000000,
    "timestamp_b": 5100000000
  }
}
```

**Parallel-safe:** ✅ Sí

---

## debug_diff — Comparar Estado Entre Dos Eventos

Compara las variables y estado entre dos event_ids específicos.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id_a` | u64 | — | Primer evento |
| `event_id_b` | u64 | — | Segundo evento |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_diff",
  "params": {
    "session_id": "sess_a1b2c3",
    "event_id_a": 1234,
    "event_id_b": 5678
  }
}
```

**Parallel-safe:** ✅ Sí

---

## get_event — Detalle de un Evento

Obtiene el detalle completo de un evento específico por ID.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id` | u64 | — | ID del evento a obtener |

**Ejemplo de llamada:**
```json
{
  "tool": "get_event",
  "params": { "session_id": "sess_a1b2c3", "event_id": 1234 }
}
```

**Parallel-safe:** ✅ Sí

---

## Resumen

| Herramienta | Cuándo usarla | Parallel-safe |
|------------|--------------|----------------|
| `query_events` | Siempre con filtros después de orientación | ✅ |
| `get_call_stack` | Después de identificar un event_id de interés | ✅ |
| `evaluate_expression` | Después de conocer variables en scope | ✅ |
| `debug_get_variables` | Después de identificar un event_id de interés | ✅ |
| `state_diff` | Después de identificar dos timestamps a comparar | ✅ |
| `debug_diff` | Después de identificar dos eventos a comparar | ✅ |
| `get_event` | Lookup de un evento específico por ID | ✅ |

**Regla de oro:** Las herramientas de drill-down requieren parámetros específicos (event_id, timestamps, direcciones) que deben venir de hallazgos previos. No las llames al azar.
