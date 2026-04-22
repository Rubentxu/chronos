# 02 — Session Lifecycle

This chapter covers all tools for managing session state: attaching to processes, stopping captures, querying background status, and persisting/loading sessions.

---

## Session State Model

A session can exist in three states:

```
[capture running] ──debug_stop──► [in-memory]
                                       │
                   ┌───────────────────┼───────────────────┐
                   │                   │                   │
              drop_session        save_session       delete_session
                   │                   │                   │
              [removed]          [persisted]          [removed from
             (no storage                             both memory and
              impact)                                   storage)]
```

Persisted sessions can be brought back with `load_session`.

---

## `debug_attach` {#debug_attach}

Attach to a **running** process by PID. Chronos uses ptrace to hook into the target without restarting it.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `pid` | `number` | **yes** | — | OS process ID to attach to |
| `trace_syscalls` | `bool` | no | `true` | Capture syscall events |
| `capture_registers` | `bool` | no | `true` | Snapshot registers on each stop |

### Example Call

```json
{
  "name": "debug_attach",
  "arguments": {
    "pid": 42817,
    "trace_syscalls": true,
    "capture_registers": true
  }
}
```

### Response Fields

```json
{
  "session_id": "sess_d9e1f2a3",
  "pid": 42817,
  "status": "attached"
}
```

### Natural Language Prompt

> "Attach to process 42817 and start capturing its execution."

---

## `debug_stop` {#debug_stop}

Stop an **active** capture session and finalize its query indices.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Session to stop |

### Example Call

```json
{
  "name": "debug_stop",
  "arguments": { "session_id": "sess_d9e1f2a3" }
}
```

### Response Fields

```json
{
  "session_id": "sess_d9e1f2a3",
  "event_count": 15200,
  "status": "stopped"
}
```

### Natural Language Prompt

> "Stop the capture for session `sess_d9e1f2a3`."

---

## `get_session_status` {#get_session_status}

Query the status of a **background** session (one launched with `background: true`).

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Background session ID to query |

### Example Call

```json
{
  "name": "get_session_status",
  "arguments": { "session_id": "sess_bg_7f3a" }
}
```

### Response Fields

```json
{
  "session_id": "sess_bg_7f3a",
  "status": "running",
  "event_count": 12400,
  "elapsed_ms": 4200
}
```

| Field | Values | Description |
|-------|--------|-------------|
| `status` | `"running"` \| `"completed"` \| `"error"` | Current state |
| `event_count` | `number` | Events collected so far |
| `elapsed_ms` | `number` | Time since session start |

### Natural Language Prompt

> "What is the current status of background session `sess_bg_7f3a`?"

---

## `drop_session` {#drop_session}

Remove a session from **memory only**. Does not touch the persistent store. Safe to call even if the session is not loaded — it is idempotent.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Session to drop from memory |

### Example Call

```json
{
  "name": "drop_session",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Response Fields

```json
{
  "session_id": "sess_abc123",
  "dropped": true
}
```

### Natural Language Prompt

> "Free the memory for session `sess_abc123` but keep it on disk."

---

## `delete_session` {#delete_session}

Permanently remove a session from **both the persistent store and memory**. This action is irreversible.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Session to delete permanently |

### Example Call

```json
{
  "name": "delete_session",
  "arguments": { "session_id": "sess_old_trace" }
}
```

### Response Fields

```json
{
  "session_id": "sess_old_trace",
  "deleted": true
}
```

### Natural Language Prompt

> "Permanently delete session `sess_old_trace` from disk and memory."

---

## `save_session` {#save_session}

Persist an **in-memory** session to the redb store so it survives server restarts.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | In-memory session to persist |
| `language` | `string` | **yes** | Runtime/language tag (e.g., `"rust"`, `"python"`) |
| `target` | `string` | **yes** | Program path or name associated with this session |

### Example Call

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

### Response Fields

```json
{
  "session_id": "sess_abc123",
  "persisted": true,
  "bytes_written": 2048576
}
```

### Natural Language Prompt

> "Save session `sess_abc123` to disk as a Rust trace of `./myapp`."

---

## `load_session` {#load_session}

Load a previously persisted session **from disk into memory** so it can be queried.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Session ID to load from store |

### Example Call

```json
{
  "name": "load_session",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Response Fields

```json
{
  "session_id": "sess_abc123",
  "loaded": true,
  "event_count": 84201,
  "language": "rust",
  "target": "./target/debug/myapp"
}
```

### Natural Language Prompt

> "Load session `sess_abc123` from disk so I can query it."

---

## `list_sessions` {#list_sessions}

List **all persisted sessions** in the store. Takes no parameters.

### Parameters

_None._

### Example Call

```json
{
  "name": "list_sessions",
  "arguments": {}
}
```

### Response Fields

```json
{
  "sessions": [
    {
      "session_id": "sess_abc123",
      "language": "rust",
      "target": "./myapp",
      "event_count": 84201,
      "created_at": "2025-01-15T14:30:00Z"
    },
    {
      "session_id": "sess_def456",
      "language": "python",
      "target": "app.py",
      "event_count": 12000,
      "created_at": "2025-01-16T09:00:00Z"
    }
  ],
  "count": 2
}
```

### Natural Language Prompt

> "List all saved debug sessions."

> "Show me every persisted session in the store."

---

## `compare_sessions` {#compare_sessions}

Perform a **hash-based structural diff** between two sessions. Useful for spotting regressions or changes between runs.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_a` | `string` | **yes** | First session ID |
| `session_b` | `string` | **yes** | Second session ID |

### Example Call

```json
{
  "name": "compare_sessions",
  "arguments": {
    "session_a": "sess_baseline",
    "session_b": "sess_new_build"
  }
}
```

### Response Fields

```json
{
  "are_identical": false,
  "event_count_a": 84201,
  "event_count_b": 84850,
  "delta_events": 649,
  "new_functions": ["async_handler::process_batch"],
  "removed_functions": [],
  "hash_a": "a3f8b2c1d4e5f6a7",
  "hash_b": "b4e9c3d2e6f7a8b9",
  "diff_summary": "649 additional events in session_b; 1 new function observed"
}
```

### Natural Language Prompt

> "Compare session `sess_baseline` against `sess_new_build` to see what changed."

---

## Lifecycle Workflow Cheat Sheet

```
# Capture a new trace
debug_run → session_id

# Attach to a running process
debug_attach(pid) → session_id

# Background capture: launch and poll
debug_run(background=true) → session_id
get_session_status(session_id) → {status, event_count}
  ... wait until status = "completed" ...

# Persist and free memory
save_session(session_id, language, target)
drop_session(session_id)

# Reload later
load_session(session_id)

# Clean up permanently
delete_session(session_id)
```
