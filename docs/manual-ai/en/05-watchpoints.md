# 05 — Watchpoints & Subscriptions

Hardware watchpoints allow Chronos to **monitor a symbol or memory address in real time** while a program is running. Unlike trace queries (which analyze past data), watchpoints are live subscriptions that fire asynchronously as the target writes to the watched location.

---

## How Watchpoints Work

Chronos uses x86-64 **debug registers** (DR0–DR3) to set hardware watchpoints on a running process via ptrace. When the CPU detects an access matching the watchpoint condition, it raises SIGTRAP, which Chronos intercepts and converts into a `VariableWrite` `TraceEvent`.

```
subscribe_to_symbol(symbol, watch_type)
        │
        ▼
  HardwareWatchpointManager
  sets DR0–DR3 on target PID
        │
        ▼
  watch_task (async tokio task)
  waits on waitpid(SIGTRAP)
        │
        ▼
  TraceEvent pushed to ring buffer
  (max 1024 events per subscription)
        │
        ▼
get_subscription_events(subscription_id) → events[]
```

---

## `subscribe_to_symbol` {#subscribe_to_symbol}

Set a **hardware watchpoint** on a named symbol or raw memory address.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Debug session containing the target process |
| `symbol` | `string` | **yes** | — | Symbol name or `"0x<hex address>"` for a raw address |
| `watch_type` | `string` | no | `"write"` | `"write"` \| `"readwrite"` \| `"execute"` |
| `pid` | `number \| null` | no | from session | Override PID (if different from session PID) |

### Watch Type Semantics

| Value | Triggers on |
|-------|-------------|
| `"write"` | Any write to the watched address |
| `"readwrite"` | Any read or write to the watched address |
| `"execute"` | Execution of the instruction at the watched address |

### Example: Watch a symbol for writes

```json
{
  "name": "subscribe_to_symbol",
  "arguments": {
    "session_id": "sess_abc123",
    "symbol": "global_error_count",
    "watch_type": "write"
  }
}
```

### Example: Watch a raw address for reads and writes

```json
{
  "name": "subscribe_to_symbol",
  "arguments": {
    "session_id": "sess_abc123",
    "symbol": "0x7f3a4b5c6d00",
    "watch_type": "readwrite"
  }
}
```

### Response Fields

```json
{
  "subscription_id": "sub_f3a8b1c2",
  "session_id": "sess_abc123",
  "symbol": "global_error_count",
  "resolved_address": 140234567890,
  "watch_type": "write",
  "status": "active"
}
```

| Field | Description |
|-------|-------------|
| `subscription_id` | Unique ID; required for `get_subscription_events` and `unsubscribe_from_symbol` |
| `resolved_address` | The actual memory address the watchpoint was placed on |
| `status` | `"active"` if the watchpoint was set successfully |

### Natural Language Prompts

> "Watch the symbol `global_error_count` for any writes."

> "Set a watchpoint on address `0x7f3a4b5c6d00` to track reads and writes."

> "Monitor `connection_table` for execute access."

### Constraints

- x86-64 supports at most **4 simultaneous hardware watchpoints** (DR0–DR3). Attempting to add a fifth will return an error.
- The target process must still be running (attached via `debug_attach` or an active `debug_run` session).

---

## `get_subscription_events` {#get_subscription_events}

Poll the **event buffer** of a subscription to retrieve watchpoint hit events.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `subscription_id` | `string` | **yes** | — | Subscription ID from `subscribe_to_symbol` |
| `limit` | `number` | no | `100` | Maximum events to return |
| `timeout_ms` | `number \| null` | no | `0` | Wait up to N ms for events (0 = non-blocking) |

### Polling Modes

| `timeout_ms` | Behavior |
|-------------|----------|
| `0` (default) | Non-blocking: returns immediately with whatever is buffered |
| `> 0` | Waits up to `timeout_ms` milliseconds for at least one event |

### Example: Non-blocking poll

```json
{
  "name": "get_subscription_events",
  "arguments": {
    "subscription_id": "sub_f3a8b1c2",
    "limit": 50
  }
}
```

### Example: Wait up to 2 seconds for events

```json
{
  "name": "get_subscription_events",
  "arguments": {
    "subscription_id": "sub_f3a8b1c2",
    "limit": 10,
    "timeout_ms": 2000
  }
}
```

### Response Fields

```json
{
  "subscription_id": "sub_f3a8b1c2",
  "events": [
    {
      "event_id": 0,
      "timestamp_ns": 1234580000000,
      "thread_id": 2,
      "event_type": "variable_write",
      "address": 140234567890,
      "value": "0x0000000000000001"
    },
    {
      "event_id": 0,
      "timestamp_ns": 1234581500000,
      "thread_id": 3,
      "event_type": "variable_write",
      "address": 140234567890,
      "value": "0x0000000000000002"
    }
  ],
  "returned": 2,
  "buffer_size": 2
}
```

> **Note:** `event_id` is `0` for watchpoint events because they are appended to the subscription ring buffer with a placeholder ID, not to the main trace.

### Natural Language Prompts

> "Check for any new watchpoint hits on subscription `sub_f3a8b1c2`."

> "Wait up to 5 seconds for the next write to `global_error_count`."

---

## `unsubscribe_from_symbol` {#unsubscribe_from_symbol}

Remove a **hardware watchpoint** and free the debug register it occupied.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `subscription_id` | `string` | **yes** | Subscription to remove |

### Example Call

```json
{
  "name": "unsubscribe_from_symbol",
  "arguments": {
    "subscription_id": "sub_f3a8b1c2"
  }
}
```

### Response Fields

```json
{
  "subscription_id": "sub_f3a8b1c2",
  "removed": true,
  "events_captured": 18
}
```

### Natural Language Prompt

> "Remove the watchpoint for subscription `sub_f3a8b1c2`."

---

## Complete Watchpoint Workflow

The following example shows a full watchpoint lifecycle for detecting unauthorized writes to a critical global variable:

### Step 1: Run the target in background

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "./target/debug/server",
    "background": true
  }
}
```
→ `{ "session_id": "sess_bg_abc" }`

### Step 2: Subscribe to a critical variable

```json
{
  "name": "subscribe_to_symbol",
  "arguments": {
    "session_id": "sess_bg_abc",
    "symbol": "auth_bypass_flag",
    "watch_type": "write"
  }
}
```
→ `{ "subscription_id": "sub_sec_01" }`

### Step 3: Poll for events (with 3-second wait)

```json
{
  "name": "get_subscription_events",
  "arguments": {
    "subscription_id": "sub_sec_01",
    "limit": 10,
    "timeout_ms": 3000
  }
}
```

### Step 4: Analyze any hits

If events are returned, use `get_call_stack` or `inspect_causality` on the triggering event to determine the source.

### Step 5: Clean up

```json
{
  "name": "unsubscribe_from_symbol",
  "arguments": { "subscription_id": "sub_sec_01" }
}
```

---

## Buffer Overflow Behavior

The subscription ring buffer holds a maximum of **1024 events**. When full, the oldest event is evicted (`buf.remove(0)`). Always poll frequently to avoid losing events in high-frequency write scenarios.
