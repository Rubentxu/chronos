# 04 — Advanced Analysis

This chapter documents all 15 advanced analysis tools: call graphs, variable origin tracing, crash detection, race detection, causality inspection, hotspot analysis, register inspection, memory forensics, performance regression auditing, and expression evaluation.

---

## `debug_call_graph` {#debug_call_graph}

Build the **complete function call graph** for the session up to a configurable depth. Returns callers and callees for every function observed in the trace.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `max_depth` | `number` | no | `10` | Maximum traversal depth |

### Example Call

```json
{
  "name": "debug_call_graph",
  "arguments": {
    "session_id": "sess_abc123",
    "max_depth": 5
  }
}
```

### Response Fields

```json
{
  "nodes": [
    { "function": "main", "call_count": 1 },
    { "function": "parse_config", "call_count": 1 },
    { "function": "start_server", "call_count": 1 }
  ],
  "edges": [
    { "caller": "main", "callee": "parse_config", "count": 1 },
    { "caller": "main", "callee": "start_server", "count": 1 },
    { "caller": "start_server", "callee": "handle_connection", "count": 48 }
  ],
  "max_depth": 5
}
```

### Natural Language Prompt

> "Show me the call graph for session `sess_abc123` up to depth 5."

---

## `debug_find_variable_origin` {#debug_find_variable_origin}

Trace the **origin and all mutations** of a named variable using the CausalityIndex. Returns every write observed during the trace, ordered chronologically.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `variable_name` | `string` | **yes** | — | Exact variable name to trace |
| `limit` | `number` | no | `100` | Maximum mutations to return |

### Example Call

```json
{
  "name": "debug_find_variable_origin",
  "arguments": {
    "session_id": "sess_abc123",
    "variable_name": "request_count",
    "limit": 50
  }
}
```

### Response Fields

```json
{
  "variable_name": "request_count",
  "mutations": [
    {
      "event_id": 102,
      "timestamp_ns": 1234510000000,
      "function": "init_counters",
      "old_value": null,
      "new_value": "0",
      "address": 140234567890
    },
    {
      "event_id": 850,
      "timestamp_ns": 1234580000000,
      "function": "handle_request",
      "old_value": "0",
      "new_value": "1",
      "address": 140234567890
    }
  ],
  "total_mutations": 2
}
```

### Natural Language Prompt

> "Where was the variable `request_count` first set, and how did it change over time?"

---

## `debug_find_crash` {#debug_find_crash}

Identify the **crash point** in a trace: finds the last event before a fatal signal (SIGSEGV, SIGABRT, etc.) and returns the call stack at that moment.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |

### Example Call

```json
{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_abc123" }
}
```

### Response Fields

```json
{
  "crash_found": true,
  "signal": "SIGSEGV",
  "crash_event_id": 84200,
  "timestamp_ns": 1234599001200,
  "faulting_address": "0x0000000000000008",
  "call_stack": [
    { "depth": 0, "function": "parse_packet", "file": "src/net.rs", "line": 201 },
    { "depth": 1, "function": "process_frame", "file": "src/protocol.rs", "line": 88 },
    { "depth": 2, "function": "main_loop", "file": "src/main.rs", "line": 45 }
  ]
}
```

### Natural Language Prompt

> "Did the program crash? If so, where and what was the stack?"

> "Find the crash in session `sess_abc123`."

---

## `debug_detect_races` {#debug_detect_races}

Detect **data races**: concurrent writes to the same memory address within a configurable time threshold from different threads.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `threshold_ns` | `number` | no | `100` | Race detection window in nanoseconds |

### Example Call

```json
{
  "name": "debug_detect_races",
  "arguments": {
    "session_id": "sess_abc123",
    "threshold_ns": 500
  }
}
```

### Response Fields

```json
{
  "races_found": 2,
  "races": [
    {
      "address": 140234567890,
      "thread_a": 2,
      "thread_b": 3,
      "event_id_a": 4210,
      "event_id_b": 4215,
      "delta_ns": 120,
      "function_a": "worker_thread",
      "function_b": "io_thread"
    }
  ],
  "threshold_ns": 500
}
```

### Natural Language Prompt

> "Are there any data races in session `sess_abc123`?"

> "Check for concurrent writes within a 1-microsecond window."

---

## `inspect_causality` {#inspect_causality}

Inspect the **full causal history** of a memory address: all reads and writes, their timestamps, values, and originating functions. Uses the CausalityIndex.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `address` | `number` | **yes** | — | Memory address (decimal) |
| `limit` | `number` | no | `100` | Maximum history entries to return |

### Example Call

```json
{
  "name": "inspect_causality",
  "arguments": {
    "session_id": "sess_abc123",
    "address": 140234567890,
    "limit": 20
  }
}
```

### Response Fields

```json
{
  "address": 140234567890,
  "history": [
    {
      "event_id": 102,
      "timestamp_ns": 1234510000000,
      "access_type": "write",
      "value": "0x0000000000000000",
      "function": "init_counters",
      "thread_id": 1
    },
    {
      "event_id": 850,
      "timestamp_ns": 1234580000000,
      "access_type": "write",
      "value": "0x0000000000000001",
      "function": "handle_request",
      "thread_id": 2
    },
    {
      "event_id": 910,
      "timestamp_ns": 1234585000000,
      "access_type": "read",
      "value": "0x0000000000000001",
      "function": "read_counter",
      "thread_id": 1
    }
  ],
  "total_accesses": 3
}
```

### Natural Language Prompt

> "Show me the complete read/write history for memory address 140234567890."

---

## `debug_expand_hotspot` {#debug_expand_hotspot}

**Semantic compression Level 1** — return the top-N hottest functions by call count and CPU cycles. Use `get_execution_summary` first (Level 0) to identify if hotspot analysis is warranted.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `top_n` | `number` | no | `10` | Number of top functions to return |

### Example Call

```json
{
  "name": "debug_expand_hotspot",
  "arguments": {
    "session_id": "sess_abc123",
    "top_n": 15
  }
}
```

### Response Fields

```json
{
  "hotspots": [
    {
      "rank": 1,
      "function": "parse_field",
      "call_count": 12400,
      "cpu_cycles": 4820000,
      "avg_cycles_per_call": 389
    },
    {
      "rank": 2,
      "function": "alloc_buffer",
      "call_count": 8300,
      "cpu_cycles": 3110000,
      "avg_cycles_per_call": 375
    }
  ],
  "top_n": 15
}
```

### Natural Language Prompt

> "What are the top 15 hottest functions in session `sess_abc123`?"

---

## `debug_get_saliency_scores` {#debug_get_saliency_scores}

Compute **saliency scores** [0.0–1.0] for all functions: a high score means this function consumed a disproportionate share of CPU cycles relative to other functions. Use to prioritize investigation.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `limit` | `number` | no | `20` | Maximum functions to score |

### Example Call

```json
{
  "name": "debug_get_saliency_scores",
  "arguments": {
    "session_id": "sess_abc123",
    "limit": 10
  }
}
```

### Response Fields

```json
{
  "scores": [
    { "function": "parse_field",   "saliency": 0.94, "cpu_cycles": 4820000 },
    { "function": "alloc_buffer",  "saliency": 0.71, "cpu_cycles": 3110000 },
    { "function": "validate_utf8", "saliency": 0.45, "cpu_cycles": 1980000 }
  ],
  "limit": 10
}
```

### Natural Language Prompt

> "Which functions have the highest saliency scores in this session?"

---

## `debug_diff` {#debug_diff}

Compare **program state** (registers and variables) between two specific event IDs within a session.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id_a` | `number` | **yes** | First event ID |
| `event_id_b` | `number` | **yes** | Second event ID |

### Example Call

```json
{
  "name": "debug_diff",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id_a": 1000,
    "event_id_b": 2000
  }
}
```

### Response Fields

```json
{
  "event_id_a": 1000,
  "event_id_b": 2000,
  "register_diff": {
    "rax": { "before": "0x0", "after": "0x2a" },
    "rip": { "before": "0x55a3b2c10010", "after": "0x55a3b2c14320" }
  },
  "variable_diff": [
    { "name": "count", "before": "0", "after": "42" }
  ]
}
```

### Natural Language Prompt

> "What changed between event 1000 and event 2000?"

---

## `debug_analyze_memory` {#debug_analyze_memory}

Analyze **all memory accesses** (reads and writes) within a specified address range and time window.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `start_address` | `number` | **yes** | Start of address range (inclusive) |
| `end_address` | `number` | **yes** | End of address range (inclusive) |
| `start_ts` | `number` | **yes** | Start timestamp (nanoseconds) |
| `end_ts` | `number` | **yes** | End timestamp (nanoseconds) |

### Example Call

```json
{
  "name": "debug_analyze_memory",
  "arguments": {
    "session_id": "sess_abc123",
    "start_address": 140234567890,
    "end_address": 140234568000,
    "start_ts": 1234510000000,
    "end_ts": 1234599001200
  }
}
```

### Response Fields

```json
{
  "address_range": "0x20a6dba892..0x20a6dba8c0",
  "time_range_ns": [1234510000000, 1234599001200],
  "total_accesses": 12,
  "reads": 8,
  "writes": 4,
  "accesses": [
    {
      "event_id": 510,
      "timestamp_ns": 1234515000000,
      "access_type": "read",
      "address": 140234567892,
      "function": "read_header"
    }
  ]
}
```

### Natural Language Prompt

> "Show me all memory accesses to addresses 140234567890–140234568000 during the trace."

---

## `debug_get_variables` {#debug_get_variables}

Retrieve **all variables in scope** at a specific event.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id` | `number` | **yes** | Event at which to inspect scope |

### Example Call

```json
{
  "name": "debug_get_variables",
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
  "variables": [
    { "name": "req",   "value": "0x7f3a4b5c", "type": "Request*", "scope": "local" },
    { "name": "count", "value": "42",          "type": "i32",       "scope": "local" },
    { "name": "limit", "value": "100",         "type": "i32",       "scope": "global" }
  ]
}
```

### Natural Language Prompt

> "What variables were in scope at event 1042?"

---

## `debug_get_memory` {#debug_get_memory}

Read the **raw memory value** at a given address at (or before) a specific timestamp.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `address` | `number` | **yes** | Memory address to read |
| `timestamp_ns` | `number` | **yes** | Returns most-recent write at or before this timestamp |

### Example Call

```json
{
  "name": "debug_get_memory",
  "arguments": {
    "session_id": "sess_abc123",
    "address": 140234567890,
    "timestamp_ns": 1234580000000
  }
}
```

### Response Fields

```json
{
  "address": 140234567890,
  "timestamp_ns": 1234580000000,
  "value": "0x0000000000000001",
  "last_written_at": 1234580000000,
  "written_by_function": "handle_request"
}
```

### Natural Language Prompt

> "What was the value at address 140234567890 at timestamp 1234580000000?"

---

## `debug_get_registers` {#debug_get_registers}

Get **CPU register values** at a specific event.

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id` | `number` | **yes** | Event at which to read registers |

### Example Call

```json
{
  "name": "debug_get_registers",
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
  "registers": {
    "rax": "0x0000000000000000",
    "rbx": "0x0000000000000001",
    "rcx": "0x00007f3a4b5c6d00",
    "rdx": "0x0000000000000064",
    "rsi": "0x00007ffe12345678",
    "rdi": "0x00007f3a4b5c0000",
    "rsp": "0x00007ffe12345678",
    "rbp": "0x00007ffe12345700",
    "rip": "0x000055a3b2c10010",
    "rflags": "0x0000000000000246"
  }
}
```

### Natural Language Prompt

> "What were the CPU register values at event 1042?"

---

## `forensic_memory_audit` {#forensic_memory_audit}

Generate a **complete audit trail** of all writes to a specific memory address. Useful for security analysis, forensic investigation, and understanding memory corruption.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `session_id` | `string` | **yes** | — | Target session |
| `address` | `number` | **yes** | — | Memory address to audit |
| `limit` | `number` | no | `100` | Maximum write records to return |

### Example Call

```json
{
  "name": "forensic_memory_audit",
  "arguments": {
    "session_id": "sess_abc123",
    "address": 140234567890,
    "limit": 50
  }
}
```

### Response Fields

```json
{
  "address": 140234567890,
  "total_writes": 4,
  "writes": [
    {
      "event_id": 102,
      "timestamp_ns": 1234510000000,
      "value_written": "0x0000000000000000",
      "function": "init_counters",
      "thread_id": 1,
      "call_stack_depth": 3
    },
    {
      "event_id": 850,
      "timestamp_ns": 1234580000000,
      "value_written": "0x0000000000000001",
      "function": "handle_request",
      "thread_id": 2,
      "call_stack_depth": 5
    }
  ]
}
```

### Natural Language Prompt

> "Show me every write to address 140234567890 during the session."

> "Produce a forensic audit of all modifications to address 0x20a6dba892."

---

## `performance_regression_audit` {#performance_regression_audit}

Compare two sessions and **score regressions** by CPU cycles and call counts. Returns severity-classified findings with a composite regression score.

### Parameters

| Parameter | Type | Required | Default | Description |
|-----------|------|----------|---------|-------------|
| `baseline_session_id` | `string` | **yes** | — | Session representing the baseline (e.g., previous release) |
| `target_session_id` | `string` | **yes** | — | Session to evaluate for regressions |
| `top_n` | `number` | no | `20` | Maximum functions to compare |

### Response Fields

```json
{
  "regression_score": 0.73,
  "severity": "high",
  "total_regressions": 5,
  "total_improvements": 2,
  "critical_count": 1,
  "regressions": [
    {
      "function": "parse_field",
      "severity": "critical",
      "cycles_baseline": 4820000,
      "cycles_target": 12400000,
      "cycle_increase_pct": 157.3,
      "call_count_baseline": 12400,
      "call_count_target": 12400
    },
    {
      "function": "alloc_buffer",
      "severity": "high",
      "cycles_baseline": 3110000,
      "cycles_target": 5200000,
      "cycle_increase_pct": 67.2
    }
  ],
  "improvements": [
    {
      "function": "validate_utf8",
      "cycles_baseline": 1980000,
      "cycles_target": 980000,
      "cycle_decrease_pct": 50.5
    }
  ]
}
```

### Severity Classification

| Severity | Regression Score Range | Meaning |
|----------|----------------------|---------|
| `critical` | > 0.8 | Severe degradation, likely a bug |
| `high` | 0.6–0.8 | Significant regression requiring immediate attention |
| `medium` | 0.4–0.6 | Moderate regression, should be investigated |
| `low` | < 0.4 | Minor regression, acceptable in most cases |

### Example Call

```json
{
  "name": "performance_regression_audit",
  "arguments": {
    "baseline_session_id": "sess_v1_0_0",
    "target_session_id": "sess_v1_1_0",
    "top_n": 30
  }
}
```

### Natural Language Prompt

> "Compare session `sess_v1_0_0` (baseline) against `sess_v1_1_0` for performance regressions."

> "Did the new build regress in CPU usage compared to last release?"

---

## `evaluate_expression` {#evaluate_expression}

**Evaluate an arithmetic expression** using the values of local variables at a specific event. Supports multi-language runtimes (native, Python, JavaScript, Java, Go).

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `session_id` | `string` | **yes** | Target session |
| `event_id` | `number` | **yes** | Event at which to evaluate |
| `expression` | `string` | **yes** | Arithmetic expression (e.g., `"x + y * 2"`) |

### Example Call

```json
{
  "name": "evaluate_expression",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 1042,
    "expression": "request_count * retry_delay + base_timeout"
  }
}
```

### Response Fields

```json
{
  "event_id": 1042,
  "expression": "request_count * retry_delay + base_timeout",
  "result": "342",
  "result_type": "i64",
  "variables_used": [
    { "name": "request_count", "value": "42" },
    { "name": "retry_delay",   "value": "8" },
    { "name": "base_timeout",  "value": "6" }
  ]
}
```

### Natural Language Prompt

> "Evaluate `request_count * retry_delay + base_timeout` at event 1042."

> "What is `x + y * 2` using the variables in scope at event 500?"
