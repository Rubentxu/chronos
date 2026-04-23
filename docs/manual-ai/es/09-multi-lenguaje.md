# 09 — Soporte Multi-Lenguaje

Chronos soporta 6 familias de lenguajes, cada uno con un mecanismo de captura diferente. El parámetro `program_language` en `debug_run` selecciona el adaptador apropiado.

## Comparación de Lenguajes

| Lenguaje | Mecanismo | Adaptador | Variables | Eval expresión |
|----------|-----------|---------|-----------|----------------|
| Native (C/C++/Rust) | ptrace | chronos-native | ✅ | ✅ Evaluador nativo |
| Java | JDWP | chronos-java | ✅ | ✅ JDWP Evaluate |
| Python | DAP / debugpy | chronos-python | ✅ | ✅ via DAP |
| JavaScript/Node.js | CDP | chronos-js | ✅ | ✅ via CDP |
| Go | Delve DAP | chronos-go | ✅ | ✅ via DAP |
| eBPF | aya uprobes | chronos-ebpf | ❌ | ❌ |

---

## Native (C, C++, Rust)

**Mecanismo:** ptrace system call tracing

**Setup:** Ninguno — solo apuntar al binario. Chronos auto-detecta binarios ELF.

```json
{
  "tool": "debug_run",
  "params": {
    "program": "./target/release/my-service",
    "args": ["--config", "prod.toml"],
    "trace_syscalls": true,
    "capture_registers": true
  }
}
```

**Capacidades:**
- Tracing completo de entrada/salida de función
- Tracing de system calls enter/exit
- Captura de registros en cada parada
- Eventos de acceso a memoria
- Hardware watchpoints
- Evaluación de expresiones con variables locales

**Gotchas:**
- Requiere capacidad `CAP_SYS_PTRACE` o mismo user ID
- Conteo de eventos muy alto en programas I/O-heavy — considerar límite `max_events`
- Syscall tracing agrega overhead — desactivar con `trace_syscalls: false` si no es necesario

---

## Python

**Mecanismo:** DAP (Debug Adapter Protocol) via debugpy

**Setup:** El script Python objetivo debe estar corriendo con debugpy escuchando:

```bash
python -m debugpy --listen 127.0.0.1:5678 --wait-for-client my_script.py
```

**Luego capturar con Chronos:**

```json
{
  "tool": "debug_run",
  "params": {
    "program": "my_script.py",
    "args": ["--data", "input.json"],
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true
  }
}
```

**Parámetros clave:**
- `debug_port` — requerido para Python (sin auto-discovery)
- `wait_for_connection: true` — hace polling hasta que debugpy esté listo (hasta 30s)
- `debug_host` — default `127.0.0.1`

**Retry behavior:** Cuando `wait_for_connection` es false, Chronos reintenta 3x con exponential backoff (200ms → 400ms → 800ms).

**Capacidades:**
- Entrada/salida de función
- Inspección de variables via DAP scopes
- Evaluación de expresiones via DAP evaluate request
- Listado de threads
- Reconstrucción de call stack

**Gotchas:**
- debugpy debe iniciarse ANTES de que `debug_run` sea llamado
- `--wait-for-client` bloquea debugpy hasta que Chronos conecta — este es el modo recomendado
- Sin `--wait-for-client`, debugpy termina inmediatamente
- El GIL de Python significa que solo un thread corre a la vez — data race detection es menos relevante

---

## JavaScript / Node.js

**Mecanismo:** CDP (Chrome DevTools Protocol) via Node.js inspector

**Setup:** Iniciar Node.js con el inspector API habilitado:

```bash
node --inspect=127.0.0.1:9229 my_script.js
```

**Luego capturar con Chronos:**

```json
{
  "tool": "debug_run",
  "params": {
    "program": "my_script.js",
    "program_language": "nodejs",
    "debug_host": "127.0.0.1",
    "debug_port": 9229,
    "wait_for_connection": true
  }
}
```

**Parámetros clave:**
- `debug_port` — requerido para Node.js
- `wait_for_connection: true` — hace polling hasta que inspector esté listo
- `program_language` — acepta `"nodejs"`, `"javascript"`, `"js"`, `"node"`

**Gotchas:**
- Node.js inspector debe estar corriendo ANTES de que Chronos conecte
- `--inspect` habilita el inspector en puerto 9229 por defecto
- El formato de URL WebSocket CDP: `ws://127.0.0.1:9229/...`
- Conexión usa exponential backoff retry (igual que Python): 200ms → 400ms → 800ms

---

## Java

**Mecanismo:** JDWP (Java Debug Wire Protocol)

**Setup:** Iniciar la JVM con argumentos de debug:

```bash
java -agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=*:5005 \
  -jar my-application.jar
```

**Luego capturar con Chronos:**

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/java",
    "args": ["-jar", "my-application.jar"],
    "program_language": "java",
    "debug_host": "127.0.0.1",
    "debug_port": 5005,
    "wait_for_connection": true
  }
}
```

**Nota de arquitectura:** La evaluación Java usa el `evaluate_expression` del adaptador directamente, no el dispatcher — llama a `JdwpAdapter::evaluate()` que usa `InvokeMethod` para métodos de instancia y `GetValues` para campos estáticos.

**Gotchas:**
- `suspend=n` es crítico — de lo contrario la JVM pausa al inicio esperando un debugger
- `address=*:5005` bindea a todas las interfaces para debug remoto
- JDWP es un protocolo stateful — las sesiones deben mantenerse conectadas
- Aplicaciones Java grandes generan conteos de eventos muy altos

---

## Go

**Mecanismo:** Delve DAP (Debug Adapter Protocol)

**Setup:** Iniciar el programa Go con Delve:

```bash
dlv debug ./cmd/my-service --accept-multiclient --listen=127.0.0.1:38657
```

**Capacidades:**
- Tracking completo de goroutines
- Stack traces con frames específicos de Go (goroutine, goroadmap)
- Inspección de variables via Delve expression evaluator
- Awareness de threads (Go tiene miles de goroutines)
- Detección de acceso concurrente

**Gotchas:**
- `--accept-multiclient` es requerido — permite a Chronos conectar mientras un cliente Delve también está conectado
- El modelo de goroutines de Go significa que `list_threads` devuelve miles de entradas por defecto — filtrar agresivamente
- Data race detection es nativo en Go — `debug_detect_races` es especialmente valioso para Go

---

## eBPF

**Mecanismo:** aya-rs uprobes attachadas a funciones kernel/user-space

**Capacidades:**
- Entrada/salida de funciones kernel (kprobes)
- Entrada/salida de funciones user-space (uprobes)
- System call tracing a nivel kernel
- Overhead mínimo — corre en kernel space

**Gotchas:**
- Requiere headers del kernel y versión de kernel compatible
- No puede inspeccionar variables — eBPF solo captura direcciones y timestamps
- Sin evaluación de expresiones
- El debugger es el kernel mismo

---

## Auto-Detección

Si `program_language` se omite, Chronos auto-detecta desde la extensión del archivo:

| Extensión | Lenguaje |
|-----------|----------|
| `.py` | Python |
| `.js`, `.ts` | JavaScript |
| `.go` | Go |
| `.java`, `.class`, `.jar` | Java |
| Binario ELF (sin ext) | Native |
| Objeto eBPF | eBPF |

---

## Resumen por Lenguaje

| Lenguaje | Iniciar debugger | Parámetros Chronos |
|----------|-----------------|-------------------|
| Native | Ninguno necesario | `debug_run({ program: "./binary" })` |
| Python | `python -m debugpy --listen HOST:PORT --wait-for-client` | `program_language: "python"`, `debug_port: PORT` |
| Node.js | `node --inspect=HOST:PORT` | `program_language: "nodejs"`, `debug_port: PORT` |
| Java | `java -agentlib:jdwp=...,address=*:PORT` | `program_language: "java"`, `debug_port: PORT` |
| Go | `dlv debug --accept-multiclient --listen=HOST:PORT` | `program_language: "go"`, `debug_port: PORT` |
| eBPF | Programa BPF pre-compilado | `program_language: "ebpf"` |
