# 11 — Ejemplos de Prompts para Agentes IA

Este capítulo contiene 20 ejemplos de workflows completos. Cada uno muestra un prompt de usuario realista a un agente IA, la secuencia de herramientas Chronos que el agente debe hacer, y los insights clave a extraer.

---

## Ejemplo 1: Investigación de Crash en Rust Native

**Usuario:**
> "Nuestro servicio Rust crasheó con exit code 101. ¿Puedes descobrir qué pasó? El binario está en /usr/bin/chronos-service."

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/chronos-service", "trace_syscalls": true } }
]
```
// Esperar resultado con session_id

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_find_crash", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } }
]
```

**Insights a extraer:**
- ¿Fue un panic? ¿SIGABRT? ¿SIGSEGV?
- Función en el punto de crash
- Call stack hacia el crash
- Qué thread crasheó (main? worker?)

---

## Ejemplo 2: Regresión de Performance Entre Dos Builds

**Usuario:**
> "Lanzamos una nueva versión y la latencia P99 subió 40ms. ¿Puedes comparar el perfil de performance? Baseline: /usr/bin/service_v1, Nuevo: /usr/bin/service_v2."

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/service_v1", "trace_syscalls": false, "auto_save": true } },
  { "tool": "debug_run", "params": { "program": "/usr/bin/service_v2", "trace_syscalls": false, "auto_save": true } }
]
```
// Dos session_ids: baseline_id, current_id

```json
[
  { "tool": "performance_regression_audit", "params": { "baseline_session_id": "${BASELINE}", "target_session_id": "${CURRENT}", "top_n": 20 } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${BASELINE}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${CURRENT}" } }
]
```

**Insights a extraer:**
- `regression_score` (0.0–1.0) y severity
- Qué funciones específicas degradaron
- Cambios en call counts entre versiones
- Nuevas funciones en el hot path

---

## Ejemplo 3: Condición de Carrera en Código C Concurrente

**Usuario:**
> "Tenemos un crash intermitente en nuestro servidor C++ multithread. Sospechamos una data race. ¿Puedes buscarla?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./c++-server", "trace_syscalls": false } }
]
```

```json
[
  { "tool": "debug_detect_races", "params": { "session_id": "${SESSION}", "threshold_ns": 100 } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```

**Insights a extraer:**
- TODAS las races encontradas (no solo la primera)
- Direcciones involucradas en las races
- Orden temporal de escrituras en conflicto

---

## Ejemplo 4: Corrupción de Memoria / Buffer Overflow

**Usuario:**
> "Nuestro servicio inicia bien pero degrada en 10 minutos y eventualmente crashea. Probablemente corrupción de memoria. ¿Puedes trazar las escrituras a memoria?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false } }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_find_crash", "params": { "session_id": "${SESSION}" } }
]
```
// Si se encuentra crash:

```json
{
  "tool": "forensic_memory_audit",
  "params": { "session_id": "${SESSION}", "address": "${CRASH_ADDRESS}", "limit": 100 }
}
```

**Insights a extraer:**
- TODAS las escrituras a la dirección corrupta a lo largo del tiempo
- Qué función realizó cada escritura
- Timeline de corrupción (¿gradual o súbita?)

---

## Ejemplo 5: Debugging de Servicio Python

**Usuario:**
> "Nuestro API server Python está colgado en el endpoint /api/users. El proceso está corriendo con debugpy en el puerto 5678. ¿Puedes encontrar el bottleneck?"

**Prerrequisito:** Usuario inició `python -m debugpy --listen 127.0.0.1:5678 --wait-for-client api_server.py`

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "api_server.py",
      "program_language": "python",
      "debug_host": "127.0.0.1",
      "debug_port": 5678,
      "wait_for_connection": true,
      "args": ["--endpoint", "/api/users"]
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 10 } }
]
```

**Insights a extraer:**
- Qué funciones Python consumieron más tiempo
- Call counts vs consumo de tiempo (discrepancia = I/O bound?)

---

## Ejemplo 6: Debugging JavaScript/Node.js

**Usuario:**
> "Nuestro microservicio Node.js lanza un unhandled promise rejection cada ~5 minutos. ¿Puedes trazar la fuente del rechazo?"

**Prerrequisito:** Usuario inició `node --inspect=127.0.0.1:9229 server.js`

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "server.js",
      "program_language": "nodejs",
      "debug_host": "127.0.0.1",
      "debug_port": 9229,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["exception_thrown"],
      "limit": 10
    }
  },
  { "tool": "debug_call_graph", "params": { "session_id": "${SESSION}", "max_depth": 8 } }
]
```

**Insights a extraer:**
- Tipo de excepción y mensaje
- Stack trace en el punto de rechazo
- Cadena de Promises hacia el rejection no manejado

---

## Ejemplo 7: Debugging de Aplicación Java (JDWP)

**Usuario:**
> "Nuestra app Spring Boot Java cuelga al iniciar. JVM args: -agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=*:5005. ¿Puedes encontrar dónde está colgada?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "/usr/bin/java",
      "args": ["-jar", "app.jar"],
      "program_language": "java",
      "debug_host": "127.0.0.1",
      "debug_port": 5005,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } }
]
```

**Insights a extraer:**
- Qué thread está colgado (main? un pool thread?)
- Contención de monitor/lock
- Llamada de red o base de datos bloqueante

---

## Ejemplo 8: Debugging de Servicio Go (Delve)

**Usuario:**
> "Nuestro HTTP server Go tiene goroutine leaks después de manejar 1000 requests. ¿Puedes encontrar qué no se está limpiando? Delve corriendo en puerto 38657."

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "./http-server",
      "program_language": "go",
      "debug_host": "127.0.0.1",
      "debug_port": 38657,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 20 } }
]
```

**Insights a extraer:**
- Conteo total de goroutines al inicio vs al final
- Cuáles goroutines están en estado wait (chan receive, select on closed channel, etc.)
- Funciones creando goroutines que no terminan

---

## Ejemplo 9: Python Llamando Rust via FFI

**Usuario:**
> "Nuestro servicio Python llama una librería Rust via ctypes y crashea cuando pasamos arrays grandes. ¿Puedes trazar qué pasa en el boundary FFI?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "python3",
      "args": ["-m", "debugpy", "--listen", "127.0.0.1:5678", "--wait-for-client", "-c", "from rust_lib import process_array; process_array(large_data)"],
      "program_language": "python",
      "debug_host": "127.0.0.1",
      "debug_port": 5678,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["function_entry", "function_exit"],
      "function_pattern": "*rust*",
      "limit": 50
    }
  },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 15 } }
]
```

**Insights a extraer:**
- Funciones en el boundary FFI (funciones Rust llamadas desde Python)
- Parámetros pasados a través del boundary
- Ubicación del crash relativa a la llamada FFI

---

## Ejemplo 10: CI/CD Regression Gate

**Usuario:**
> "Como parte de nuestro pipeline CI, necesitamos fallar el build si la nueva versión tiene más de 10% de regresión en las funciones del hot path. El baseline está en el store como 'baseline_sha_abc123'."

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false, "auto_save": true } }
]
// Nueva sesión de build: current_id

[
  { "tool": "load_session", "params": { "session_id": "baseline_sha_abc123" } },
  { "tool": "performance_regression_audit", "params": {
      "baseline_session_id": "baseline_sha_abc123",
      "target_session_id": "${CURRENT}",
      "top_n": 50
    }
  }
]
```

**Decisión del agente:**
```python
if regression.regression_score > 0.1:
    fail_build(f"Regression score {regression.regression_score:.2f} exceeds threshold")
elif regression.critical_count > 0:
    fail_build(f"{regression.critical_count} critical regressions found")
```

---

## Ejemplo 11: Replay de Incidente de Producción

**Usuario:**
> "Tuvimos un incidente el martes pasado a las 14:32 UTC. El ingeniero on-call guardó una sesión como 'incident_0420_1432'. ¿Puedes cargarla y descobrir qué salió mal?"

**Secuencia Chronos:**
```json
[
  { "tool": "load_session", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "get_execution_summary", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "debug_find_crash", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "incident_0420_1432", "limit": 20 } }
]
```

---

## Ejemplo 12: Detección de Memory Leak

**Usuario:**
> "El RSS de nuestro servicio crece de 200MB a 800MB en 1 hora. No hay OOM, pero sigue creciendo. ¿Puedes encontrar desbalances de alloc/free?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false } }
]
// Ejecutar workload representativo

[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 20 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } }
]
```
// Buscar desbalances malloc/free en funciones hotspot

```json
[
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["function_entry", "function_exit"],
      "function_pattern": "*alloc*",
      "limit": 100
    }
  }
]
```

---

## Ejemplo 13: Identificación de Función Lenta

**Usuario:**
> "Nuestra API tiene un spike de latencia de 500ms en el endpoint /orders. ¿Puedes identificar cuál función es el bottleneck?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./api-server", "args": ["--endpoint", "/orders"] } }
]

[
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 10 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 5 } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```

**Insights a extraer:**
- La función con score más alto (mayor tiempo CPU)
- Call count vs tiempo — una función llamada una vez que toma 400ms es el bottleneck
- Funciones hot anidadas — caller vs callee

---

## Ejemplo 14: Análisis de System Calls

**Usuario:**
> "Sospechamos que nuestro servicio está haciendo demasiadas llamadas al sistema de archivos redundantes. ¿Puedes analizar los syscalls?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": true } }
]

[
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["syscall_enter"],
      "limit": 100
    }
  },
  { "tool": "debug_call_graph", "params": { "session_id": "${SESSION}", "max_depth": 6 } }
]
```

**Insights a extraer:**
- Frecuencia de syscalls por tipo (open, read, write, stat)
- Patrones redundantes (mismo archivo abierto N veces sin cerrar)
- Funciones haciendo más syscalls

---

## Ejemplo 15: Trazado de Valor de Variable

**Usuario:**
> "En algún lugar de nuestro código, una variable `total` se vuelve -1 cuando no debería. ¿Puedes trazar cómo llegó ahí?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service" } }
]

[
  { "tool": "debug_find_variable_origin", "params": { "session_id": "${SESSION}", "variable_name": "total", "limit": 50 } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```

**Insights a extraer:**
- TODAS las mutaciones a `total` con timestamps
- La función que primero la estableció a -1
- El call path hacia esa función

---

## Ejemplo 16: Análisis de Interleaving de Threads

**Usuario:**
> "Tenemos una condición de carrera en la sincronización de threads. ¿Puedes analizar cómo los threads se entrelazaron alrededor del timestamp 5s del trace?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "./mt-service" } }
]

[
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "timestamp_start": 5000000000,
      "timestamp_end": 5100000000,
      "limit": 200
    }
  }
]
```

**Insights a extraer:**
- Orden de eventos entre threads en la ventana
- Secuencias de adquisición de locks
- Patrones potenciales de deadlock

---

## Ejemplo 17: Análisis de eBPF Kernel Probe

**Usuario:**
> "¿Puedes analizar la latencia de system calls usando eBPF? Nuestro trace está en una sesión capturada con program_language=ebpf."

**Secuencia Chronos:**
```json
[
  { "tool": "load_session", "params": { "session_id": "${EBPF_SESSION}" } },
  { "tool": "get_execution_summary", "params": { "session_id": "${EBPF_SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${EBPF_SESSION}",
      "event_types": ["syscall_enter", "syscall_exit"],
      "limit": 100
    }
  }
]
```

**Insights a extraer:**
- Pares syscall enter/exit y latencia por llamada
- Syscalls de alta latencia
- Distribución de frecuencia de tipos de syscall

---

## Ejemplo 18: Comparación Staging vs Production

**Usuario:**
> "El servicio funciona en staging pero falla en production. ¿Puedes comparar el trace de production contra nuestro baseline de staging?"

**Secuencia Chronos:**
```json
[
  { "tool": "load_session", "params": { "session_id": "staging_baseline" } },
  { "tool": "load_session", "params": { "session_id": "prod_incident_trace" } }
]

[
  { "tool": "compare_sessions", "params": { "session_a": "staging_baseline", "session_b": "prod_incident_trace" } },
  { "tool": "performance_regression_audit", "params": {
      "baseline_session_id": "staging_baseline",
      "target_session_id": "prod_incident_trace",
      "top_n": 30
    }
  }
]
```

**Insights a extraer:**
- Funciones presentes en production pero no en staging
- Funciones con diferentes call counts
- Degradación de performance específica del entorno de production

---

## Ejemplo 19: Forensics de Test Fallido

**Usuario:**
> "Nuestro test de integración 'test_user_creation' falla en CI pero pasa localmente. ¿Puedes comparar el trace passing local contra el trace failing de CI?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "pytest", "args": ["-v", "tests/test_user_creation.py"] } }
]
// local_session y ci_session capturadas separadamente

[
  { "tool": "compare_sessions", "params": { "session_a": "local_session", "session_b": "ci_session" } },
  { "tool": "debug_detect_races", "params": { "session_id": "ci_session" } },
  { "tool": "get_execution_summary", "params": { "session_id": "ci_session" } }
]
```

---

## Ejemplo 20: Perfilado de Servicio de Larga Duración

**Usuario:**
> "Tenemos un daemon que corre por 24 horas. Necesitamos un perfil de CPU después de 1 hora de operación. ¿Puedes capturar y analizar?"

**Secuencia Chronos:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/daemon", "background": true } }
]
// Esperar a que la sesión esté disponible

[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 30 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 15 } }
]
```

---

## Resumen: Cuándo Usar Cada Herramienta

| Escenario | Herramientas principales |
|-----------|------------------------|
| Investigación de crash | `debug_find_crash` → `get_call_stack` → `debug_get_variables` |
| Regresión de performance | `performance_regression_audit` → `debug_get_saliency_scores` |
| Detección de data race | `debug_detect_races` → `query_events` (filtrado por thread) |
| Corrupción de memoria | `forensic_memory_audit` → `inspect_causality` |
| Función lenta | `debug_get_saliency_scores` → `debug_expand_hotspot` |
| Trazado de variable | `debug_find_variable_origin` → `query_events` (rango de tiempo) |
| Comparación multi-build | `compare_sessions` → `performance_regression_audit` |
| Incidente de producción | `load_session` → batch de orientación → drill-down dirigido |
| CI/CD gate | `debug_run` → `save_session` → regression audit |
| Debugging Python/JS | `debug_run` con `debug_port` → batch de orientación |
