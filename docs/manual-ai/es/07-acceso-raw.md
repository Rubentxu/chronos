# 07 — Acceso Raw: Memoria y Registros

Las herramientas de acceso raw dan información a nivel de hardware: direcciones de memoria crudas, valores de registros CPU, y análisis de ventanas de memoria. Son las herramientas más primitivas del toolkit y rara vez son necesarias para debugging de applications de alto nivel.

**Regla general:** Si puedes resolver el problema con herramientas de orientación, análisis bulk, o forense, no uses herramientas raw.

## debug_get_memory — Leer Memoria Raw en un Timestamp

Lee el valor en una dirección de memoria específica en un timestamp dado.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `address` | u64 | — | Dirección de memoria a leer (decimal) |
| `timestamp_ns` | u64 | — | Timestamp en el cual leer |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_get_memory",
  "params": {
    "session_id": "sess_a1b2c3",
    "address": 140734193800032,
    "timestamp_ns": 5000000000
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "address": "0x7f8a2c3d4e5f",
  "timestamp_ns": 5000000000,
  "value": "0x00000001",
  "found": true
}
```

**Cuándo usarla:** Solo cuando conoces la dirección exacta de antemano (de `forensic_memory_audit`, `inspect_causality`, o `debug_find_crash`).

---

## debug_get_registers — Valores de Registros CPU

Obtiene los valores de todos los registros CPU en un event_id específico.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `event_id` | u64 | — | Evento en el cual obtener registros |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_get_registers",
  "params": { "session_id": "sess_a1b2c3", "event_id": 1234 }
}
```

**Respuesta (x86-64):**
```json
{
  "session_id": "sess_a1b2c3",
  "event_id": 1234,
  "registers": {
    "rax": "0x0000000000000001",
    "rbx": "0x00007f8a2c3d4e5f",
    "rcx": "0x0000000000000000",
    "rdx": "0x0000000000000000",
    "rsi": "0x00007ffd8a9b0",
    "rdi": "0x00007ffd8a9b8",
    "rbp": "0x00007ffd8a9c0",
    "rsp": "0x00007ffd8a9b0",
    "rip": "0x000055a3b2c1d0e0",
    "r8": "0x0000000000000000",
    "r9": "0x0000000000000000",
    "r10": "0x0000000000000000",
    "r11": "0x0000000000000000",
    "r12": "0x0000000000000000",
    "r13": "0x0000000000000000",
    "r14": "0x0000000000000000",
    "r15": "0x0000000000000000",
    "eflags": "0x00000246"
  }
}
```

**Cuándo usarla:** debugging de nivel muy bajo, crashes de segurijack, errores de memoria. Para la mayoría de lenguajes (Python, Java, Go), los registros tienen utilidad limitada.

**Parallel-safe:** ✅ Sí

---

## debug_analyze_memory — Análisis de Accesos en Ventana

Analiza todos los accesos a memoria en un rango de direcciones y ventana de tiempo.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `session_id` | string | — | Requerido |
| `start_address` | u64 | — | Dirección inicial (inclusive) |
| `end_address` | u64 | — | Dirección final (inclusive) |
| `start_ts` | u64 | — | Timestamp inicial (ns) |
| `end_ts` | u64 | — | Timestamp final (ns) |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_analyze_memory",
  "params": {
    "session_id": "sess_a1b2c3",
    "start_address": 140734193800000,
    "end_address": 140734193800100,
    "start_ts": 5000000000,
    "end_ts": 5100000000
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_a1b2c3",
  "start_address": "0x7f8a2c3d4e60",
  "end_address": "0x7f8a2c3d4ec8",
  "start_ts_ns": 5000000000,
  "end_ts_ns": 5100000000,
  "accesses": [
    {
      "event_id": 1234,
      "timestamp_ns": 5012345678,
      "thread_id": 2,
      "type": "write",
      "address": "0x7f8a2c3d4e60",
      "size_bytes": 8
    }
  ],
  "total_reads": 12,
  "total_writes": 3
}
```

**Cuándo usarla:** Análisis de buffer. ¿Cuántas veces se accedió a este rango? ¿Lecturas o escrituras? Útil para detectar accesos inesperados a regiones de memoria.

**Parallel-safe:** ✅ Sí

---

## ¿Cuándo Realmente Necesitas Acceso Raw?

**Casos válidos:**
- Bug de corrupcción de memoria a nivel C/C++
- Investigación de crash a nivel assembly
- Análisis de exploits
- Validación de valores de punteros en Rust unsafe

**Casos donde NO lo necesitas:**
- Python, Java, JavaScript, Go — tienen abstracciones de memoria que hacen el acceso raw innecesario
- Bugs lógicos en código de alto nivel
- Problemas de performance
- Condiciones de carrera

---

## Resumen

| Herramienta | Entrada | Caso de uso |
|------------|---------|-------------|
| `debug_get_memory` | dirección + timestamp | Leer valor específico |
| `debug_get_registers` | event_id | Estado de CPU en crash |
| `debug_analyze_memory` | rango direcciones + rango tiempo | Accesos a buffer |

**Regla de oro:** Las herramientas de acceso raw son para debugging de bajo nivel. Si una herramienta de nivel superior puede responder tu pregunta, úsala en su lugar.
