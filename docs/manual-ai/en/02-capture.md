# 02 — Capture Tools

Capture tools launch or attach to a program and record a complete execution trace. This is always the first step — every other tool requires a `session_id` returned by a capture tool.

---

## `debug_run`

**One-line description:** Launch a program, capture its entire execution, and return a session_id for analysis.

**When to call:** Always first. No other tool can be called without a session_id from debug_run (or debug_attach).

**Parallel-safe?** N/A — must complete before any analysis tool.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `program` | string | required | Absolute path to the executable or interpreter |
| `args` | array of string | `[]` | Command-line arguments passed to the program |
| `trace_syscalls` | bool | `true` | Capture system call entries and exits |
| `capture_registers` | bool | `true` | Snapshot CPU registers at every function entry/exit |
| `cwd` | string | current dir | Working directory for the target process |
| `auto_save` | bool | `false` | Automatically persist session to store after capture |
| `program_language` | string | `"native"` | Hint for tracer selection: `native`, `python`, `javascript`, `java`, `go`, `ebpf` |
| `max_events` | int | `1000000` | Hard cap on captured events (prevents OOM on very long runs) |
| `timeout_secs` | int | `30` | Kill process after this many seconds if still running |
| `background` | bool | `false` | Return immediately; capture runs asynchronously |
| `debug_host` | string | `"localhost"` | Host for remote debugger (Java/Python/JS/Go) |
| `debug_port` | int | `null` | Port for remote debugger — required for Python/JS/Java/Go |
| `wait_for_connection` | bool | `false` | Block until the debug adapter connects (Python/JS/Java/Go) |

### Example call — Native/Rust binary

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/home/user/project/target/debug/myapp",
    "args": ["--input", "data.csv", "--verbose"],
    "trace_syscalls": true,
    "capture_registers": true,
    "timeout_secs": 60
  }
}
```

### Example call — Python service

Python requires launching the program with `debugpy` and specifying the debug port:

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/python3",
    "args": ["-m", "debugpy", "--listen", "5678", "--wait-for-client", "myservice.py"],
    "program_language": "python",
    "debug_host": "localhost",
    "debug_port": 5678,
    "wait_for_connection": true,
    "timeout_secs": 120
  }
}
```

### Example call — Node.js service

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/node",
    "args": ["--inspect=9229", "server.js"],
    "program_language": "javascript",
    "debug_host": "localhost",
    "debug_port": 9229,
    "wait_for_connection": true,
    "timeout_secs": 60
  }
}
```

### Example call — Go service (Delve DAP)

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/local/go/bin/dlv",
    "args": ["dap", "--listen=:56268", "exec", "/home/user/project/myservice"],
    "program_language": "go",
    "debug_host": "localhost",
    "debug_port": 56268,
    "wait_for_connection": true,
    "timeout_secs": 90
  }
}
```

### Example call — Java application (JDWP)

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/java",
    "args": [
      "-agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=*:5005",
      "-jar", "myapp.jar"
    ],
    "program_language": "java",
    "debug_host": "localhost",
    "debug_port": 5005,
    "wait_for_connection": true,
    "timeout_secs": 120
  }
}
```

### Example call — Background mode (long-running service)

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/home/user/myserver",
    "args": ["--port", "8080"],
    "background": true,
    "max_events": 500000,
    "timeout_secs": 300
  }
}
```

Background mode returns immediately with a `session_id`. The trace is being collected asynchronously. Use `get_execution_summary` periodically to check if capture is complete before running analysis tools.

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "status": "complete",
  "exit_code": 1,
  "duration_ms": 4231,
  "events_captured": 182943,
  "threads_observed": 4,
  "warnings": []
}
```

### Background mode response

```json
{
  "session_id": "sess_e5f6g7h8",
  "status": "capturing",
  "message": "Capture running in background. Poll get_execution_summary to check completion."
}
```

### Notes

- **`max_events` is critical** for long-running programs. The default 1,000,000 events can overflow memory for services that run for minutes. Set a lower limit and use `timeout_secs` together.
- **Python, JavaScript, Go, and Java require `debug_port`** and typically `wait_for_connection: true`. The program must be launched with its debug adapter enabled (debugpy, `--inspect`, Delve DAP, or JDWP).
- **`capture_registers: false`** reduces memory usage by ~40% if you don't need register-level analysis. Use this for long captures where you only care about function-level profiling.
- **`auto_save: true`** is recommended for CI/CD pipelines where you want to persist the session for later comparison.

---

## `debug_attach`

**One-line description:** Attach to an already-running process by PID and begin capturing its execution.

**When to call:** When the target process is already running and cannot be relaunched (e.g., a production service, a daemon, or a process that requires specific startup conditions).

**Parallel-safe?** N/A — must complete before any analysis tool.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `pid` | int | required | Process ID of the running target |
| `trace_syscalls` | bool | `true` | Capture system call entries and exits |
| `capture_registers` | bool | `true` | Snapshot CPU registers at function entries |

### Example call

```json
{
  "tool": "debug_attach",
  "params": {
    "pid": 18342,
    "trace_syscalls": true,
    "capture_registers": false
  }
}
```

### Example response

```json
{
  "session_id": "sess_attach_9z8y7x",
  "status": "capturing",
  "pid": 18342,
  "message": "Attached successfully. Trace is ongoing. Send SIGTERM or wait for process exit to finalize."
}
```

### Notes

- **Prefer `debug_run` over `debug_attach` whenever possible.** `debug_run` captures from the very first instruction. `debug_attach` misses everything that happened before attachment.
- Attaching to a process requires appropriate OS permissions (typically the same user, or root). On Linux, `ptrace_scope` may need to be set to 0.
- The attach session finishes when the process exits or when you stop the trace. Only then can you run analysis tools.
- `debug_attach` does not support `program_language`, `debug_port`, or `wait_for_connection` — it uses ptrace directly regardless of language.

---

## Choosing Between debug_run and debug_attach

| Situation | Use |
|-----------|-----|
| You control the program's launch | `debug_run` (always preferred) |
| Program requires specific environment setup you've already done | `debug_run` with `cwd` |
| Program is already running, cannot be restarted | `debug_attach` |
| Production service you want to briefly instrument | `debug_attach` |
| CI/CD pipeline — reproducible capture | `debug_run` |
| Python/JS/Java/Go — needs debug adapter | `debug_run` with `debug_port` + `wait_for_connection` |

---

## Language-Specific Capture Quick Reference

| Language | `program_language` | `debug_port` needed? | `wait_for_connection` | Extra args |
|----------|--------------------|----------------------|-----------------------|------------|
| C / C++ / Rust | `native` or omit | No | No | — |
| Python | `python` | Yes (e.g. 5678) | Yes | `-m debugpy --listen <port> --wait-for-client` |
| JavaScript / Node.js | `javascript` | Yes (e.g. 9229) | Yes | `--inspect=<port>` |
| Java | `java` | Yes (e.g. 5005) | Yes | `-agentlib:jdwp=...` |
| Go | `go` | Yes (e.g. 56268) | Yes | `dlv dap --listen=:<port> exec <binary>` |
| eBPF | `ebpf` | No | No | — |

See [09-multi-language.md](09-multi-language.md) for full language-specific setup instructions.
