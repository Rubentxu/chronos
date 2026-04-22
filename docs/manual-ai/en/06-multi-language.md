# 06 — Multi-Language Support

Chronos supports six language families through different backend adapters. This chapter explains how to configure `debug_run` for each runtime, what level of introspection is available, and known limitations.

---

## Language Support Matrix

| Language | Backend | Transport | Eval Backend | Notes |
|----------|---------|-----------|--------------|-------|
| C / C++ | ptrace | in-process | Native | Full register capture |
| Rust | ptrace | in-process | Native | Full register capture |
| eBPF | aya uprobes | in-process | Native | Uprobe-based |
| Python | debugpy (DAP) | TCP | `PythonDapEvalBackend` | Retry 3×, `wait_for_connection` |
| JavaScript/Node.js | V8 Inspector (CDP) | WebSocket | `JsCdpEvalBackend` | `ws://host:port` |
| Java | JDWP | TCP | JDWP eval | `get_threads`, `get_stack_trace`, `get_variables` |
| Go | Delve DAP | TCP | DAP eval | Delve must be installed |

---

## Native Languages: C, C++, Rust {#native}

C, C++, and Rust programs are captured **natively via ptrace**. No external tooling is required beyond a debug build.

### Recommended Build Flags

```bash
# Rust
cargo build          # debug profile includes debug info by default

# C/C++
gcc -O0 -g -o myapp myapp.c
clang++ -O0 -g -o myapp myapp.cpp
```

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "./target/debug/myapp",
    "args": ["--arg1", "value"],
    "trace_syscalls": true,
    "capture_registers": true,
    "cwd": "/path/to/workdir",
    "timeout_secs": 30
  }
}
```

`program_language` can be omitted — Chronos infers native from the binary format.

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit | ✅ |
| Syscall tracing | ✅ |
| Register capture | ✅ |
| Memory reads/writes | ✅ |
| Variable inspection | ✅ (via DWARF) |
| Expression evaluation | ✅ (arithmetic, local vars) |
| Call graph | ✅ |
| Race detection | ✅ |
| Crash detection | ✅ (SIGSEGV/SIGABRT) |
| Watchpoints | ✅ |

---

## Python {#python}

Python sessions use **debugpy** (Microsoft's Debug Adapter Protocol implementation for Python). Chronos acts as a DAP client connecting to a running debugpy server.

### Setup

```bash
pip install debugpy

# Start your script with debugpy waiting for connection
python -m debugpy --listen 127.0.0.1:5678 --wait-for-client app.py
```

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "app.py",
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true,
    "timeout_secs": 60
  }
}
```

`wait_for_connection: true` causes Chronos to **poll every 500 ms for up to 30 seconds** before failing, accommodating slow startup times.

### Eval Backend

Expression evaluation uses `PythonDapEvalBackend` which:
- Sends `evaluate` requests over DAP
- Retries **3 times** on transient failures
- Supports arbitrary Python expressions (not just arithmetic)

### Example Expression

```json
{
  "name": "evaluate_expression",
  "arguments": {
    "session_id": "sess_py_abc",
    "event_id": 300,
    "expression": "len(request_queue) * timeout_factor"
  }
}
```

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit | ✅ (DAP step events) |
| Syscall tracing | ❌ (not exposed by debugpy) |
| Register capture | ❌ |
| Variable inspection | ✅ (DAP scopes/variables) |
| Expression evaluation | ✅ (Python expressions) |
| Call graph | ✅ (from DAP stack frames) |
| Race detection | ⚠️ Limited (no memory-level access) |
| Crash detection | ✅ (unhandled exception events) |
| Watchpoints | ❌ (no hardware watchpoints for managed runtimes) |

---

## JavaScript / Node.js {#javascript}

JavaScript sessions use the **V8 Inspector Protocol** (Chrome DevTools Protocol / CDP) over a WebSocket connection.

### Setup

```bash
# Start Node.js with inspector enabled
node --inspect=127.0.0.1:9229 app.js

# Or break on start:
node --inspect-brk=127.0.0.1:9229 app.js
```

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "app.js",
    "program_language": "javascript",
    "debug_host": "127.0.0.1",
    "debug_port": 9229,
    "wait_for_connection": true,
    "timeout_secs": 45
  }
}
```

Chronos connects to `ws://127.0.0.1:9229` using `JsCdpEvalBackend`.

### Eval Backend

`JsCdpEvalBackend` sends CDP `Runtime.evaluate` calls over the WebSocket. Full JavaScript expressions are supported, including:
- Property access: `obj.field.subfield`
- Method calls: `arr.length`
- Ternary expressions: `x > 0 ? x : -x`

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit | ✅ (CDP `Debugger` domain) |
| Syscall tracing | ❌ |
| Register capture | ❌ |
| Variable inspection | ✅ (CDP `Debugger.evaluateOnCallFrame`) |
| Expression evaluation | ✅ (JavaScript expressions) |
| Call graph | ✅ |
| Race detection | ❌ (single-threaded event loop) |
| Crash detection | ✅ (uncaught exceptions) |
| Watchpoints | ❌ |

---

## Java {#java}

Java sessions use the **Java Debug Wire Protocol (JDWP)** for remote debugging.

### Setup

```bash
# Launch JVM with JDWP agent
java -agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=*:5005 -jar app.jar
```

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "app.jar",
    "program_language": "java",
    "debug_host": "127.0.0.1",
    "debug_port": 5005,
    "wait_for_connection": true
  }
}
```

### JDWP-Specific Operations

The Java backend exposes additional capabilities via the standard tool surface:

| Operation | MCP Tool | Notes |
|-----------|----------|-------|
| List threads | `list_threads` | Maps to JDWP `VirtualMachine.AllThreads` |
| Get stack trace | `get_call_stack` | Maps to JDWP `ThreadReference.Frames` |
| Inspect variables | `debug_get_variables` | Maps to JDWP `StackFrame.GetValues` |
| Evaluate static field | `evaluate_expression` | Maps to JDWP `ClassType.InvokeMethod` |

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit | ✅ (method entry/exit events) |
| Thread listing | ✅ |
| Variable inspection | ✅ |
| Expression evaluation | ✅ (JDWP eval) |
| Call stack | ✅ |
| Crash detection | ✅ (uncaught exceptions) |
| Race detection | ⚠️ Limited |
| Watchpoints | ❌ |

---

## Go {#go}

Go sessions use **Delve** (the standard Go debugger) via the Debug Adapter Protocol (DAP).

### Setup

```bash
# Install Delve
go install github.com/go-delve/delve/cmd/dlv@latest

# Start Delve DAP server
dlv dap --listen=127.0.0.1:38697 -- ./myapp --arg1 value
```

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "./myapp",
    "program_language": "go",
    "debug_host": "127.0.0.1",
    "debug_port": 38697,
    "wait_for_connection": true,
    "timeout_secs": 30
  }
}
```

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit | ✅ |
| Goroutine listing | ✅ (via `list_threads`) |
| Variable inspection | ✅ |
| Expression evaluation | ✅ (Go expressions via Delve) |
| Call stack | ✅ |
| Crash detection | ✅ (panic detection) |
| Race detection | ✅ (combined with `go -race`) |
| Watchpoints | ❌ |

---

## eBPF {#ebpf}

eBPF sessions use **aya uprobes** to attach to functions at the kernel boundary without modifying the target binary.

### Use Cases

- Tracing production binaries without recompilation
- Kernel-level event capture
- System call interception at the kernel/user boundary

### `debug_run` Parameters

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "/usr/sbin/nginx",
    "program_language": "ebpf",
    "trace_syscalls": true,
    "timeout_secs": 60
  }
}
```

### Requirements

- Linux kernel 5.8+ (BTF support recommended)
- `CAP_BPF` or `CAP_SYS_ADMIN` capability
- aya eBPF programs must be pre-compiled and loaded

### Available Features

| Feature | Available |
|---------|-----------|
| Function entry/exit (uprobes) | ✅ |
| Syscall tracing (kprobes) | ✅ |
| Variable inspection | ❌ (no DWARF at kernel level) |
| Expression evaluation | ✅ (arithmetic only) |
| Race detection | ⚠️ Limited |
| Crash detection | ✅ (OOM, kernel panic) |
| Watchpoints | ❌ |

---

## Language Selection Guide

```
Target language?
    │
    ├── C / C++ / Rust ──► debug_run (no extra params needed)
    │
    ├── Python ──► pip install debugpy
    │              python -m debugpy --listen 127.0.0.1:5678 --wait-for-client script.py
    │              debug_run(debug_port=5678, wait_for_connection=true)
    │
    ├── JavaScript ──► node --inspect=127.0.0.1:9229 app.js
    │                  debug_run(debug_port=9229, wait_for_connection=true)
    │
    ├── Java ──► java -agentlib:jdwp=transport=dt_socket,server=y,address=*:5005 -jar app.jar
    │            debug_run(program_language="java", debug_port=5005)
    │
    ├── Go ──► dlv dap --listen=127.0.0.1:38697 -- ./myapp
    │          debug_run(program_language="go", debug_port=38697)
    │
    └── eBPF ──► debug_run(program_language="ebpf")
                 (requires CAP_BPF)
```
