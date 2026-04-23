# 06 — Drill-Down Tools

Drill-down tools provide targeted, event-specific inspection. They answer precise questions about a specific moment in the execution trace — "what was the call stack at event 4721?" or "what were the variable values when this function was called?"

**Critical rule: Never call drill-down tools without first knowing which event_id, timestamp, or function to target.** These tools are designed for precision, not exploration. Use orientation and bulk analysis to identify the relevant scope, then use drill-down tools to get the exact details.

All drill-down tools are parallel-safe.

---

## `query_events`

**One-line description:** Retrieve a filtered list of events from the trace — by type, thread, time window, or function name pattern.

**When to call:** After orientation and bulk analysis have narrowed the scope to a specific thread, time range, or function. **Always use filters.** Unfiltered queries can return millions of events.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_types` | array of string | `null` (all) | Filter by event type: `function_entry`, `function_exit`, `syscall_enter`, `syscall_exit`, `memory_write`, `thread_switch`, `signal` |
| `thread_id` | int | `null` (all) | Filter to a specific thread |
| `timestamp_start` | int | `null` | Start of time window (nanoseconds) |
| `timestamp_end` | int | `null` | End of time window (nanoseconds) |
| `function_pattern` | string | `null` | Glob pattern to match function names (e.g., `"parse_*"`) |
| `limit` | int | `100` | Maximum events to return |
| `offset` | int | `0` | Skip this many matching events (for pagination) |

### Example call — Find all syscalls on thread 1

```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_types": ["syscall_enter"],
    "thread_id": 1,
    "limit": 50
  }
}
```

### Example call — Find all parse_* function entries in a time window

```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_types": ["function_entry"],
    "function_pattern": "parse_*",
    "timestamp_start": 1000000000,
    "timestamp_end": 2000000000,
    "limit": 100
  }
}
```

### Example call — Find events immediately before crash (using crash timestamp from debug_find_crash)

```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "timestamp_start": 4230000000,
    "timestamp_end": 4230981234,
    "limit": 50
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "total_matching": 412,
  "returned": 50,
  "offset": 0,
  "events": [
    {
      "event_id": 91201,
      "type": "function_entry",
      "timestamp_ns": 2100000000,
      "thread_id": 1,
      "function": "parse_record",
      "file": "src/parser.rs",
      "line": 30
    },
    {
      "event_id": 91202,
      "type": "syscall_enter",
      "timestamp_ns": 2100000050,
      "thread_id": 1,
      "syscall_name": "read",
      "syscall_args": { "fd": 3, "count": 4096 }
    }
  ]
}
```

### Filter pattern examples

| Goal | Filters to use |
|------|----------------|
| Events on the crashing thread only | `thread_id: <crash_thread>` |
| Find all file I/O | `event_types: ["syscall_enter"], function_pattern: null` + check `syscall_name` in results |
| Parse function calls only | `event_types: ["function_entry"], function_pattern: "parse_*"` |
| Events just before crash | `timestamp_end: <crash_timestamp_ns>`, `timestamp_start: <crash_ts - 1_000_000>` |
| Memory writes in hot path | `event_types: ["memory_write"], timestamp_start: X, timestamp_end: Y` |

---

## `get_call_stack`

**One-line description:** What is the full call stack at a specific event?

**When to call:** After identifying a specific event_id (from `debug_find_crash`, `query_events`, `forensic_memory_audit`, etc.) that you want to understand in context.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id` | int | required | Event ID at which to reconstruct the stack |

### Example call

```json
{
  "tool": "get_call_stack",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id": 91204
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "event_id": 91204,
  "timestamp_ns": 2100000100,
  "thread_id": 2,
  "call_stack": [
    { "frame": 0, "function": "worker_update_counter", "file": "src/worker.rs",  "line": 77, "is_inlined": false },
    { "frame": 1, "function": "worker_run",            "file": "src/worker.rs",  "line": 45, "is_inlined": false },
    { "frame": 2, "function": "thread_main",           "file": "src/thread.rs",  "line": 12, "is_inlined": false }
  ]
}
```

---

## `evaluate_expression`

**One-line description:** Evaluate an arithmetic expression using local variable values at a specific event.

**When to call:** After `debug_get_variables` returns variable values and you need to compute a derived value (e.g., pointer arithmetic, offset calculation, index validation).

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id` | int | required | Event ID providing the variable scope |
| `expression` | string | required | Arithmetic expression referencing local variables |

### Example call

```json
{
  "tool": "evaluate_expression",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id": 91204,
    "expression": "base_ptr + offset * element_size"
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "event_id": 91204,
  "expression": "base_ptr + offset * element_size",
  "result": "0x7fff1a2b3c4d",
  "result_decimal": 140734799995981,
  "variables_used": {
    "base_ptr": "0x7fff1a2b0000",
    "offset": 1420,
    "element_size": 8
  }
}
```

---

## `debug_get_variables`

**One-line description:** What are the values of all local variables in scope at a specific event?

**When to call:** After `get_call_stack` identifies the relevant frame, or directly after `debug_find_crash` to inspect state at crash time.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id` | int | required | Event ID at which to inspect variables |

### Example call

```json
{
  "tool": "debug_get_variables",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id": 182940
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "event_id": 182940,
  "function": "parse_record",
  "variables": [
    { "name": "record",     "type": "Record*",  "value": "0x7fff1a2b3c4d", "is_null": false },
    { "name": "field_idx",  "type": "usize",    "value": "48201" },
    { "name": "buf",        "type": "*mut u8",  "value": "0x7fff1a2b0000" },
    { "name": "buf_len",    "type": "usize",    "value": "256" },
    { "name": "write_pos",  "type": "usize",    "value": "512",             "note": "exceeds buf_len — buffer overflow" }
  ]
}
```

---

## `state_diff`

**One-line description:** What changed in CPU register state between two timestamps?

**When to call:** When you want to understand register-level changes across a time span — e.g., how did the stack pointer change? Did the return address get overwritten? Used to detect stack corruption.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `timestamp_a` | int | required | Earlier timestamp (nanoseconds) |
| `timestamp_b` | int | required | Later timestamp (nanoseconds) |

### Example call

```json
{
  "tool": "state_diff",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "timestamp_a": 4229000000,
    "timestamp_b": 4230981234
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "timestamp_a": 4229000000,
  "timestamp_b": 4230981234,
  "changed_registers": [
    { "register": "rsp", "value_a": "0x7fffffffd490", "value_b": "0x7fffffffd3a0", "delta": "-240 bytes (stack grew)" },
    { "register": "rip", "value_a": "0x000000010101ab", "value_b": "0x00007fff1a2b3c4d", "note": "RIP points into stack — stack smashing detected" },
    { "register": "rbp", "value_a": "0x7fffffffd4b0", "value_b": "0x4141414141414141", "note": "base pointer overwritten with 0x41 pattern" }
  ],
  "unchanged_register_count": 12
}
```

---

## `debug_diff`

**One-line description:** Compare full program state (registers + variables) between two specific event IDs.

**When to call:** When `state_diff` (timestamp-based) is too coarse, and you need to compare state at two specific known events (e.g., "before calling function X" vs "after returning from function X").

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id_a` | int | required | First event ID |
| `event_id_b` | int | required | Second event ID |

### Example call

```json
{
  "tool": "debug_diff",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id_a": 91200,
    "event_id_b": 91250
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "event_id_a": 91200,
  "event_id_b": 91250,
  "register_changes": [
    { "register": "rax", "value_a": "0x0", "value_b": "0x5" }
  ],
  "variable_changes": [
    { "name": "count", "value_a": "0", "value_b": "5", "changed": true },
    { "name": "buf",   "value_a": "0x7fff1000", "value_b": "0x7fff1000", "changed": false }
  ]
}
```

---

## `get_event`

**One-line description:** Get the full details of a single specific event by ID.

**When to call:** Only when you have a specific event_id and need its complete raw details. Do not call in a loop — use `query_events` with filters instead.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id` | int | required | Event ID to retrieve |

### Example call

```json
{
  "tool": "get_event",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id": 182940
  }
}
```

### Example response

```json
{
  "event_id": 182940,
  "type": "signal",
  "timestamp_ns": 4230981234,
  "thread_id": 1,
  "signal": "SIGSEGV",
  "faulting_address": "0x7fff1a2b3c4d",
  "instruction_pointer": "0x000000010a2b3c50",
  "function": "parse_record",
  "file": "src/parser.rs",
  "line": 142
}
```

---

## Drill-Down Workflow Template

After `debug_find_crash` returns `crash_event_id: 182940` and `crash_address: "0x7fff1a2b3c4d"`:

```json
[
  {
    "tool": "get_call_stack",
    "params": { "session_id": "sess_a1b2c3d4", "event_id": 182940 }
  },
  {
    "tool": "debug_get_variables",
    "params": { "session_id": "sess_a1b2c3d4", "event_id": 182940 }
  },
  {
    "tool": "query_events",
    "params": {
      "session_id": "sess_a1b2c3d4",
      "timestamp_start": 4229000000,
      "timestamp_end": 4230981234,
      "event_types": ["memory_write"],
      "limit": 20
    }
  }
]
```

All three run simultaneously. The call stack shows context. Variable values show the corrupted state. The recent memory writes show what caused the corruption.
