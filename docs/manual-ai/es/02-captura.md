# 02 — Captura de Sesiones

## debug_run — Captura Principal

`debug_run` es la puerta de entrada a Chronos. Lanza el programa bajo un tracer, captura todos los eventos, y retorna un `session_id` que se usa para todas las consultas posteriores.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `program` | string | — | Ruta al binario o script. Requerido. |
| `args` | `Vec<String>` | `[]` | Argumentos de línea de comando. |
| `trace_syscalls` | bool | `true` | Si rastrear syscalls (entrada/salida). |
| `capture_registers` | bool | `true` | Si capturar registros en cada parada. |
| `cwd` | `Option<String>` | — | Directorio de trabajo. |
| `auto_save` | `Option<bool>` | `false` | Si persistir automáticamente al store. |
| `program_language` | `Option<String>` | auto-detect | Lenguaje: `"python"`, `"rust"`, `"java"`, `"go"`, `"nodejs"`, `"javascript"`, `"ebpf"`, `"native"`. |
| `max_events` | `Option<usize>` | `1_000_000` | Máximo de eventos antes de parar. |
| `timeout_secs` | `Option<u64>` | `60` | Timeout en segundos. |
| `background` | `Option<bool>` | `false` | Si ejecutar en background y retornar inmediatamente. |
| `debug_host` | `Option<String>` | `"127.0.0.1"` | Host para conexión DAP/CDP (Python/JS). |
| `debug_port` | `Option<u16>` | — | Puerto para conexión DAP/CDP. Requerido para Python y JS. |
| `wait_for_connection` | `Option<bool>` | `false` | Si esperar hasta 30s a que el debugger esté listo. |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/my-service",
    "args": ["--config", "prod.toml"],
    "trace_syscalls": true,
    "capture_registers": true,
    "auto_save": true
  }
}
```

**Respuesta (éxito):**
```json
{
  "session_id": "sess_a1b2c3d4",
  "status": "finalized",
  "total_events": 142857,
  "end_reason": "exited(0)",
  "message": "Program '/usr/bin/my-service' captured successfully",
  "hint": "Session is queryable now. Use query_events, get_call_stack, get_execution_summary, etc."
}
```

**Respuesta (error):**
```json
{
  "error": "Capture failed: program not found: /usr/bin/nonexistent"
}
```

---

## Auto-Detección de Lenguaje

Si `program_language` se omite, Chronos detecta desde la extensión del archivo:

| Extensión | Lenguaje |
|-----------|----------|
| `.py` | Python |
| `.js`, `.ts` | JavaScript |
| `.go` | Go |
| `.java`, `.class`, `.jar` | Java |
| Binario ELF (sin ext) | Native |
| `.o`, `.bc` (objeto eBPF) | eBPF |

---

## Modo Background

Para programas de larga duración, usa `background: true`. La llamada retorna inmediatamente con `status: "running"`:

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/long-running-daemon",
    "background": true
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_background_001",
  "status": "running",
  "background": true,
  "message": "Debug session for '/usr/bin/long-running-daemon' started in background"
}
```

**Nota:** La sesión queda disponible para consulta cuando el tracer la finaliza. El agente puede continuar con otro trabajo mientras la captura corre en background.

---

## debug_attach — Adjuntar a Proceso Existente

Para capturar un proceso que ya está corriendo, usa `debug_attach` con el PID del proceso.

**Parámetros:**

| Parámetro | Tipo | Default | Descripción |
|-----------|------|---------|-------------|
| `pid` | u32 | — | Process ID a adjuntar. Requerido. |
| `trace_syscalls` | bool | `true` | Si rastrear syscalls. |
| `capture_registers` | bool | `true` | Si capturar registros. |

**Ejemplo de llamada:**
```json
{
  "tool": "debug_attach",
  "params": {
    "pid": 12345,
    "trace_syscalls": true
  }
}
```

**Respuesta:**
```json
{
  "session_id": "sess_attach_xyz",
  "status": "finalized",
  "pid": 12345,
  "total_events": 8923,
  "end_reason": "exited(0)",
  "message": "Attached to PID 12345 and captured 8923 events"
}
```

**Errores comunes:**

| Error | Causa | Solución |
|-------|-------|----------|
| `"Process with PID N not found"` | Proceso no existe o no es rastreable | Verificar que el proceso existe |
| `"Permission denied"` | Sin CAP_SYS_PTRACE | Ejecutar como root o con capacidad |
| `"Operation not permitted"` | Mismo user ID requerido | Adjuntar a proceso del mismo usuario |

---

## Límites de Recursos

`max_events` y `timeout_secs` previenen agotamiento de recursos:

```json
{
  "tool": "debug_run",
  "params": {
    "program": "./io-heavy-service",
    "max_events": 500_000,
    "timeout_secs": 30
  }
}
```

Si se alcanza el límite, la captura para y la sesión queda queryable con los eventos capturados hasta ese momento.

---

## Captura Síncrona vs Asíncrona

| Modo | Uso | Latencia percibida |
|------|-----|--------------------|
| Síncrono (default) | Programas de corta duración (< 30s) | La llamada retorna cuando está listo |
| Background | Demonios, servicios largos | Retorna inmediatamente; polling si se necesita |

**Regla general:** Si el programa corre en menos de 30 segundos, usa síncrono. Si corre más, usa background y carga la sesión después.

---

## Parámetros Importantes según Lenguaje

### Python y JavaScript — Requieren `debug_port`

```json
{
  "tool": "debug_run",
  "params": {
    "program": "my_script.py",
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true
  }
}
```

Sin `debug_port`, Chronos no sabe dónde conectarse para lenguajes interpretados.

### Native / Rust / C / C++ — Sin parámetros extra

```json
{
  "tool": "debug_run",
  "params": {
    "program": "./my-rust-service",
    "trace_syscalls": false
  }
}
```

Solo se necesita `program`. Auto-detecta binarios ELF.
