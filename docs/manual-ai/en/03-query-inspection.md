# 03 — Query & Inspection

This chapter covers all seven tools for querying and inspecting the event trace within a loaded session.

---

## Overview

All query tools require a `session_id` that refers to a session **currently in memory** (either just captured or loaded with `load_session`). They never mutate session data.

---

## `query_events` {#query_events}

The primary tool for **filtering the event stream**. Supports combining multiple filters for precise targeting.

### Description

Returns a paginated list of trace events matching the specified criteria. All filters are optional and composable — providing multiple filters performs an AND combination.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `event_types` | `string[]` | no | all | Filter by event type names (see table below) |
| `thread_id` | `number` | no | all | Filter to a single thread |
| `timestamp_start` | `number` | no | 0 | Start of time window (nanoseconds, inclusive) |
| `timestamp_end` | `number` | no | ∞ | End of time window (nanoseconds, exclusive) |
| `function_pattern` | `string` | no | — | Glob pattern matched against function names |
| `limit` | `number` | no | `100` | Maximum events to return |
| `offset` | `number` | no | `0` | Events to skip (for pagination) |

### Event Type Values

| Value | Meaning |
|-------|---------|
| `function_entry` | Function call start |
| `function_exit` | Function return |
| `syscall_enter` | System call invocation |
| `syscall_exit` | System call return |
| `memory_read` | Memory read access |
| `memory_write` | Memory write access |
| `variable_write` | Variable mutation captured |
| `signal` | OS signal received |

### Example: All syscalls in a time window

```json
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "event_types": ["syscall_enter", "syscall_exit"],
    "timestamp_start": 1000000000,
    "timestamp_end": 2000000000,
    "limit": 50
  }
}
```

### Example: Function calls matching a pattern

```json
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry"],
    "function_pattern": "handle_*",
    "limit": 200,
    "offset": 0
  }
}
```

### Response Fields

```json
{
  "events": [
    {
      "event_id": 1042,
      "timestamp_ns": 1234567890123,
      "thread_id": 1,
      "event_type": "function_entry",
      "function_name": "handle_request",
      "source_file": "src/server.rs",
      "source_line": 87
    }
  ],
  "total_matching": 342,
  "returned": 100,
  "offset": 0
}
```

### Natural Language Prompts

> "Show me all function entries in session `sess_abc123` matching `parse_*`."

> "List write events between timestamps 1000000000 and 2000000000."

> "Get the first 50 syscall events from thread 3."

---

## `get_event` {#get_event}

Retrieve **full details** of a single event by its ID.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id` | `number` | **yes** | Unique event identifier |

### Example Call

```json
{
  "name": "get_event",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 1042
  }
}
```

### Response Fields

```json
{
  "event_id": 1042,
  "timestamp_ns": 1234567890123,
  "thread_id": 1,
  "event_type": "function_entry",
  "function_name": "handle_request",
  "source_file": "src/server.rs",
  "source_line": 87,
  "registers": {
    "rip": "0x55a3b2c1d0e4",
    "rsp": "0x7ffe12345678",
    "rax": "0x0"
  },
  "data": {
    "args": [{ "name": "req", "value": "0x7f3a4b5c", "type": "Request*" }]
  }
}
```

### Natural Language Prompt

> "Get the full details of event 1042 in session `sess_abc123`."

---

## `get_call_stack` {#get_call_stack}

Reconstruct the **call stack** at the moment a specific event occurred.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id` | `number` | **yes** | Event at which to reconstruct the stack |

### Example Call

```json
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 3195
  }
}
```

### Response Fields

```json
{
  "event_id": 3195,
  "timestamp_ns": 1234599001200,
  "frames": [
    { "depth": 0, "function": "parse_json", "file": "src/parser.rs", "line": 142 },
    { "depth": 1, "function": "handle_request", "file": "src/server.rs", "line": 87 },
    { "depth": 2, "function": "main", "file": "src/main.rs", "line": 23 }
  ]
}
```

### Natural Language Prompt

> "What was the call stack when event 3195 happened?"

---

## `get_execution_summary` {#get_execution_summary}

Get a **high-level summary** of the session: total event counts by type, top called functions, and detected issues.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |

### Example Call

```json
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Response Fields

```json
{
  "session_id": "sess_abc123",
  "total_events": 84201,
  "event_counts": {
    "function_entry": 41000,
    "function_exit": 40800,
    "syscall_enter": 1200,
    "syscall_exit": 1200,
    "signal": 1
  },
  "top_functions": [
    { "name": "parse_field", "call_count": 12400 },
    { "name": "alloc_buffer", "call_count": 8300 },
    { "name": "validate_utf8", "call_count": 6100 }
  ],
  "thread_count": 4,
  "duration_ns": 1423000000,
  "issues": ["SIGSEGV detected at event 84200"]
}
```

### Natural Language Prompt

> "Give me an overview of what happened in session `sess_abc123`."

> "What are the top functions called during the trace?"

---

## `get_backtrace` {#get_backtrace}

Retrieve a **full backtrace** at a specific event, up to a configurable depth.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `event_id` | `number` | **yes** | — | Event at which to capture the backtrace |
| `max_depth` | `number` | no | `50` | Maximum stack frame depth |

### Example Call

```json
{
  "name": "get_backtrace",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 84200,
    "max_depth": 20
  }
}
```

### Response Fields

```json
{
  "event_id": 84200,
  "frames": [
    { "depth": 0, "address": "0x55a3b2c1d0e4", "function": "memcpy", "file": null, "line": null },
    { "depth": 1, "address": "0x55a3b2c14320", "function": "copy_buffer", "file": "src/buf.rs", "line": 55 },
    { "depth": 2, "address": "0x55a3b2c10100", "function": "parse_packet", "file": "src/net.rs", "line": 201 }
  ],
  "truncated": false,
  "max_depth": 20
}
```

### Natural Language Prompt

> "Show me the full backtrace at event 84200 with depth 20."

---

## `list_threads` {#list_threads}

List **all thread IDs** observed in the trace.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |

### Example Call

```json
{
  "name": "list_threads",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Response Fields

```json
{
  "session_id": "sess_abc123",
  "threads": [
    { "thread_id": 1, "event_count": 72000, "is_main": true },
    { "thread_id": 2, "event_count": 8400, "is_main": false },
    { "thread_id": 3, "event_count": 3801, "is_main": false }
  ],
  "count": 3
}
```

### Natural Language Prompt

> "How many threads were active during session `sess_abc123`?"

---

## `state_diff` {#state_diff}

Compare **CPU register state** between two timestamps in the same session. Useful for understanding what changed between two points in time.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `timestamp_a` | `number` | **yes** | First timestamp (nanoseconds) |
| `timestamp_b` | `number` | **yes** | Second timestamp (nanoseconds) |

### Example Call

```json
{
  "name": "state_diff",
  "arguments": {
    "session_id": "sess_abc123",
    "timestamp_a": 1234567000000,
    "timestamp_b": 1234599000000
  }
}
```

### Response Fields

```json
{
  "timestamp_a": 1234567000000,
  "timestamp_b": 1234599000000,
  "changed_registers": {
    "rip": { "before": "0x55a3b2c10010", "after": "0x55a3b2c1d0e4" },
    "rsp": { "before": "0x7ffe12345900", "after": "0x7ffe12345678" },
    "rax": { "before": "0x0",            "after": "0x1" }
  },
  "unchanged_registers": ["rbx", "rcx", "rdx", "rbp"]
}
```

### Natural Language Prompt

> "What register values changed between timestamps 1234567000000 and 1234599000000?"

---

## Pagination Pattern

`query_events` supports cursor-style pagination via `limit` and `offset`:

```
# Page 1
query_events({ limit: 100, offset: 0 })  → events[0..99],  total_matching: 342

# Page 2
query_events({ limit: 100, offset: 100 }) → events[100..199]

# Page 4 (last)
query_events({ limit: 100, offset: 300 }) → events[300..341]
```

Always check `total_matching` to know the total result set size.
