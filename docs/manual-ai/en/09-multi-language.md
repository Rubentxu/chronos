# 09 — Multi-Language Support

Chronos supports 6 language families, each with a different capture mechanism. The `program_language` parameter in `debug_run` selects the appropriate adapter.

## Language Comparison

| Language | Mechanism | Adapter | Variables | Expression eval |
|----------|-----------|---------|-----------|-----------------|
| Native (C/C++/Rust) | ptrace | chronos-native | ✅ | ✅ Native evaluator |
| Java | JDWP | chronos-java | ✅ | ✅ JDWP Evaluate |
| Python | DAP / debugpy | chronos-python | ✅ | ✅ via DAP |
| JavaScript/Node.js | CDP | chronos-js | ✅ | ✅ via CDP |
| Go | Delve DAP | chronos-go | ✅ | ✅ via DAP |
| eBPF | aya uprobes | chronos-ebpf | ❌ | ❌ |

## Native (C, C++, Rust)

**Mechanism:** ptrace system call tracing

**Setup:** None — just point to the binary. Chronos auto-detects ELF binaries.

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

**Capabilities:**
- Full function entry/exit tracing
- System call enter/exit tracing
- Register capture at each stop
- Memory access events
- Hardware watchpoints
- Expression evaluation with local variables

**Gotchas:**
- Requires `CAP_SYS_PTRACE` capability or same user ID
- Very high event counts on I/O-heavy programs — consider `max_events` limit
- Syscall tracing adds overhead — disable with `trace_syscalls: false` if not needed

---

## Python

**Mechanism:** DAP (Debug Adapter Protocol) via debugpy

**Setup:** The target Python script must be running with debugpy listening:

```bash
python -m debugpy --listen 127.0.0.1:5678 --wait-for-client my_script.py
```

**Then capture with Chronos:**

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

**Key parameters:**
- `debug_port` — required for Python (no auto-discovery)
- `wait_for_connection: true` — polls until debugpy is ready (up to 30s)
- `debug_host` — defaults to `127.0.0.1`

**Retry behavior:** When `wait_for_connection` is false, Chronos retries 3x with exponential backoff (200ms → 400ms → 800ms).

**Capabilities:**
- Function entry/exit
- Variable inspection via DAP scopes
- Expression evaluation via DAP evaluate request
- Thread listing
- Call stack reconstruction

**Gotchas:**
- debugpy must be started BEFORE `debug_run` is called
- `--wait-for-client` blocks debugpy until Chronos connects — this is the recommended mode
- Without `--wait-for-client`, debugpy exits immediately
- Python's GIL means only one thread runs at a time — data race detection is less relevant

---

## JavaScript / Node.js

**Mechanism:** CDP (Chrome DevTools Protocol) via Node.js inspector

**Setup:** Start Node.js with the inspector API enabled:

```bash
node --inspect=127.0.0.1:9229 my_script.js
```

**Then capture with Chronos:**

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

**Key parameters:**
- `debug_port` — required for Node.js
- `wait_for_connection: true` — polls until inspector is ready
- `program_language` — accepts `"nodejs"`, `"javascript"`, `"js"`, `"node"`

**Capabilities:**
- Function entry/exit via CDP Debugger domain
- Variable inspection via CDP Runtime domain
- Expression evaluation via CDP Runtime.evaluate
- Call stack via CDP Debugger.getStackTrace
- Async call tree tracking

**Gotchas:**
- Node.js inspector must be running BEFORE Chronos connects
- `--inspect` enables the inspector on port 9229 by default
- The CDP WebSocket URL format: `ws://127.0.0.1:9229/...`
- Connection uses exponential backoff retry (same as Python): 200ms → 400ms → 800ms

---

## Java

**Mechanism:** JDWP (Java Debug Wire Protocol)

**Setup:** Start the JVM with debug arguments:

```bash
java -agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=*:5005 \
  -jar my-application.jar
```

**Then capture with Chronos:**

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

**Capabilities:**
- All threads listing via JDWP `AllThreads`
- Stack trace via `ThreadReference frames` + `StackFrame`
- Variable inspection via `GetValues` (including static fields)
- Expression evaluation via `InvokeMethod` / `GetValues`
- Exception events via `ExceptionRequest`

**Architecture note:** Java evaluation uses the adapter's `evaluate_expression` directly, not the dispatcher — it calls `JdwpAdapter::evaluate()` which uses `InvokeMethod` for instance methods and `GetValues` for static fields.

**Gotchas:**
- `suspend=n` is critical — otherwise the JVM pauses at startup waiting for a debugger
- `address=*:5005` binds to all interfaces for remote debugging
- JDWP is a stateful protocol — sessions must stay connected
- Large Java applications generate very high event counts

---

## Go

**Mechanism:** Delve DAP (Debug Adapter Protocol)

**Setup:** Start the Go program with Delve:

```bash
dlv debug ./cmd/my-service --accept-multiclient --listen=127.0.0.1:38657
```

**Then capture with Chronos:**

```json
{
  "tool": "debug_run",
  "params": {
    "program": "my-service",
    "program_language": "go",
    "debug_host": "127.0.0.1",
    "debug_port": 38657,
    "wait_for_connection": true
  }
}
```

**Capabilities:**
- Full goroutine tracking
- Stack traces with Go-specific frames (goroutine, goroadmap)
- Variable inspection via Delve's expression evaluator
- Thread awareness (Go has thousands of goroutines)
- Concurrent access detection

**Gotchas:**
- `--accept-multiclient` is required — allows Chronos to connect while a Delve client is also connected
- Go's goroutine model means `list_threads` returns thousands of entries by default — filter aggressively
- Data race detection is native to Go — `debug_detect_races` is especially valuable for Go

---

## eBPF

**Mechanism:** aya-rs uprobes attached to kernel/user-space functions

**Setup:** eBPF programs must be pre-compiled and loaded. Chronos attaches uprobes to specified function addresses.

**Capture:**
```json
{
  "tool": "debug_run",
  "params": {
    "program": "/path/to/elf-binary",
    "program_language": "ebpf",
    "trace_syscalls": false
  }
}
```

**Capabilities:**
- Kernel function entry/exit (kprobes)
- User-space function entry/exit (uprobes)
- System call tracing at the kernel level
- Minimal overhead — runs in kernel space

**Gotchas:**
- Requires kernel headers and a compatible kernel version
- Cannot inspect variables — eBPF only captures addresses and timestamps
- No expression evaluation
- Debugger is the kernel itself

---

## Auto-Detection

If `program_language` is omitted, Chronos auto-detects from file extension:

| Extension | Language |
|-----------|----------|
| `.py` | Python |
| `.js` | JavaScript |
| `.ts` | JavaScript |
| `.go` | Go |
| `.java` | Java |
| `.class`, `.jar` | Java |
| ELF binary (no ext) | Native |
| eBPF object | eBPF |

---

## Language-Specific Workflow Summary

| Language | Start debugger | Chronos params |
|----------|---------------|----------------|
| Native | None needed | `debug_run({ program: "./binary" })` |
| Python | `python -m debugpy --listen HOST:PORT --wait-for-client` | `debug_run({ program: "app.py", program_language: "python", debug_port: PORT })` |
| Node.js | `node --inspect=HOST:PORT` | `debug_run({ program: "app.js", program_language: "nodejs", debug_port: PORT })` |
| Java | `java -agentlib:jdwp=...,address=*:PORT` | `debug_run({ program: "java", args: ["-jar", "app.jar"], program_language: "java", debug_port: PORT })` |
| Go | `dlv debug --accept-multiclient --listen=HOST:PORT` | `debug_run({ program: "app", program_language: "go", debug_port: PORT })` |
| eBPF | Pre-compiled BPF program | `debug_run({ program: "app", program_language: "ebpf" })` |
