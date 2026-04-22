# 01 — Getting Started

This chapter covers installation prerequisites, the `debug_run` tool in full detail, and a minimal end-to-end example that takes a Rust binary from source to analyzed trace.

---

## Prerequisites

| Requirement | Notes |
|-------------|-------|
| Linux x86-64 | ptrace is Linux-only for native languages |
| Rust toolchain (1.75+) | Required to build Chronos |
| `redb` storage | Auto-created at `~/.local/share/chronos/sessions.redb` |
| Python 3.9+ + `debugpy` | Only for Python sessions |
| Node.js 18+ | Only for JavaScript/Node.js sessions |
| Java 11+ | Only for Java sessions (JDWP) |
| Go 1.21+ + Delve | Only for Go sessions |

## Starting the MCP Server

```bash
# From project root
cargo run --release -p chronos-mcp

# With a custom session store path
CHRONOS_DB_PATH=/tmp/my-sessions.redb cargo run --release -p chronos-mcp
```

The server speaks JSON-RPC 2.0 over stdio (MCP transport). Connect your MCP client (Claude, a custom agent, etc.) to it.

---

## `debug_run` {#debug_run}

**The primary entry point.** Launches a target program under time-travel capture, records all events, and returns a `session_id` for subsequent queries.

### Description

`debug_run` forks the target process under ptrace (or connects to a DAP/CDP debug server for managed runtimes), captures every function entry/exit, syscall, memory access, and signal, then builds in-memory query indices. For background sessions, it returns immediately and the capture continues asynchronously.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `program` | `string` | **yes** | — | Path to the target binary or script |
| `args` | `string[]` | no | `[]` | Command-line arguments passed to the target |
| `trace_syscalls` | `bool` | no | `true` | Capture syscall enter/exit events |
| `capture_registers` | `bool` | no | `true` | Snapshot CPU registers on each stop |
| `cwd` | `string \| null` | no | `null` | Working directory for the target process |
| `auto_save` | `bool \| null` | no | `null` | Automatically persist session to disk after capture |
| `program_language` | `string \| null` | no | auto-detect | Language hint: `"c"`, `"cpp"`, `"rust"`, `"python"`, `"javascript"`, `"java"`, `"go"`, `"ebpf"` |
| `max_events` | `number \| null` | no | `1_000_000` | Cap on events collected |
| `timeout_secs` | `number \| null` | no | `60` | Wall-clock capture timeout |
| `background` | `bool \| null` | no | `null` | Return immediately; capture continues in background |
| `debug_host` | `string \| null` | no | `"127.0.0.1"` | Host for DAP/CDP connection (Python/JS) |
| `debug_port` | `number \| null` | no | `null` | Port for DAP/CDP connection |
| `wait_for_connection` | `bool \| null` | no | `null` | Poll for connection every 500 ms for up to 30 s |

### Language Detection

If `program_language` is omitted, Chronos infers it from the file extension:

| Extension | Language |
|-----------|----------|
| `.py` | `python` |
| `.js`, `.mjs` | `javascript` |
| `.java`, `.jar` | `java` |
| `.go` | `go` |
| (binary / `.rs` compiled) | `rust` / `c` / `cpp` |

### Example: Native Rust binary

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "debug_run",
    "arguments": {
      "program": "/usr/local/bin/my-server",
      "args": ["--config", "/etc/server.toml"],
      "trace_syscalls": true,
      "capture_registers": true,
      "cwd": "/var/app",
      "timeout_secs": 30,
      "auto_save": true
    }
  }
}
```

### Example: Python script with debugpy

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

### Example: Background capture

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "/opt/services/worker",
    "background": true,
    "max_events": 500000
  }
}
```

### Response Fields

```json
{
  "session_id": "sess_a3f8b2c1",
  "status": "completed",
  "event_count": 84201,
  "duration_ms": 1423,
  "end_reason": "program_exited",
  "language": "rust"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `session_id` | `string` | Unique identifier for this trace session |
| `status` | `string` | `"completed"` \| `"running"` (background) \| `"error"` |
| `event_count` | `number` | Total events captured |
| `duration_ms` | `number` | Capture wall-clock duration |
| `end_reason` | `string` | `"program_exited"` \| `"timeout"` \| `"max_events"` |
| `language` | `string` | Detected or provided language |

### Natural Language Prompts

> "Run `/usr/bin/myapp` under Chronos and give me the session ID."

> "Start capturing events from `server.py` using debugpy on port 5678."

> "Launch `./target/release/my-tool --verbose` in the background with a 120-second timeout."

> "Trace `app.js` with the Node.js inspector on port 9229."

---

## First End-to-End Example

The following sequence demonstrates a complete debugging workflow for a crashing Rust binary.

### Step 1: Capture

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "./target/debug/myapp",
    "args": ["--input", "bad_data.json"],
    "timeout_secs": 10
  }
}
```

Response:
```json
{ "session_id": "sess_abc123", "status": "completed", "event_count": 3200 }
```

### Step 2: Get execution summary

```json
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Step 3: Find the crash

```json
{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Step 4: Inspect the call stack at crash point

```json
{
  "name": "get_call_stack",
  "arguments": { "session_id": "sess_abc123", "event_id": 3195 }
}
```

### Step 5: Save the session for later analysis

```json
{
  "name": "save_session",
  "arguments": {
    "session_id": "sess_abc123",
    "language": "rust",
    "target": "./target/debug/myapp"
  }
}
```

---

## Resource Limits

Chronos enforces configurable resource limits to prevent runaway captures:

| Limit | Default | Override Parameter |
|-------|---------|-------------------|
| Max events | 1,000,000 | `max_events` |
| Capture timeout | 60 seconds | `timeout_secs` |

When a limit is reached, capture stops with `end_reason: "max_events"` or `end_reason: "timeout"`. The partial trace is still available for analysis.
