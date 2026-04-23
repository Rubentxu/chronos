# 04 — Bulk Analysis Tools

Bulk analysis tools answer broad questions about the entire execution trace in a single call. They are designed to be called in parallel, after orientation, and before drilling down into specific events.

Each tool returns structured findings that guide the next level of investigation.

---

## `debug_find_crash`

**One-line description:** Where exactly did the program crash, and what was the call stack at that moment?

**When to call:** Whenever `get_execution_summary` shows a non-zero exit signal (SIGSEGV, SIGABRT, SIGBUS, etc.), or when `exit_code` indicates abnormal termination.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |

### Example call

```json
{
  "tool": "debug_find_crash",
  "params": {
    "session_id": "sess_a1b2c3d4"
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "crash_found": true,
  "signal": "SIGSEGV",
  "crash_event_id": 182940,
  "crash_timestamp_ns": 4230981234,
  "crash_function": "parse_record",
  "crash_address": "0x00007fff1a2b3c4d",
  "call_stack_at_crash": [
    { "frame": 0, "function": "parse_record",    "file": "src/parser.rs", "line": 142 },
    { "frame": 1, "function": "process_batch",   "file": "src/batch.rs",  "line": 89  },
    { "frame": 2, "function": "main",            "file": "src/main.rs",   "line": 23  }
  ],
  "last_events_before_crash": [
    { "event_id": 182937, "type": "function_entry", "function": "parse_record" },
    { "event_id": 182938, "type": "memory_write",   "address": "0x7fff1a2b3c4d", "size": 8 },
    { "event_id": 182939, "type": "function_entry", "function": "validate_field" },
    { "event_id": 182940, "type": "signal",         "signal": "SIGSEGV" }
  ]
}
```

### What to extract

- `crash_event_id` → use as anchor for `get_call_stack`, `debug_get_variables`, `state_diff`
- `crash_address` → pass to `forensic_memory_audit` to trace all writes to that address
- `call_stack_at_crash` → identifies the full chain of responsibility
- `last_events_before_crash` → look for memory_write events just before the crash as the likely corruption site

---

## `debug_detect_races`

**One-line description:** Are there any data races — two threads writing to the same memory address within a narrow time window?

**When to call:** Whenever `list_threads` shows more than one thread. Run in parallel with `debug_find_crash` if there's also a crash.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `threshold_ns` | int | `100` | Window in nanoseconds within which concurrent writes to the same address are flagged as a race |

### Example call

```json
{
  "tool": "debug_detect_races",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "threshold_ns": 200
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "races_detected": 2,
  "races": [
    {
      "address": "0x000000010a2b3c4d",
      "race_id": 1,
      "conflicting_accesses": [
        {
          "thread_id": 2,
          "event_id": 91204,
          "timestamp_ns": 2100000100,
          "access_type": "write",
          "function": "worker_update_counter",
          "value_written": "0x0000000000000005"
        },
        {
          "thread_id": 3,
          "event_id": 91389,
          "timestamp_ns": 2100000187,
          "access_type": "write",
          "function": "worker_update_counter",
          "value_written": "0x0000000000000006"
        }
      ],
      "delta_ns": 87,
      "variable_hint": "shared_counter"
    },
    {
      "address": "0x000000010a2b3f80",
      "race_id": 2,
      "conflicting_accesses": [
        {
          "thread_id": 1,
          "event_id": 44120,
          "timestamp_ns": 1050000010,
          "access_type": "write",
          "function": "push_queue"
        },
        {
          "thread_id": 2,
          "event_id": 44198,
          "timestamp_ns": 1050000095,
          "access_type": "read",
          "function": "pop_queue"
        }
      ],
      "delta_ns": 85,
      "variable_hint": "queue_head"
    }
  ]
}
```

### What to extract

- `address` for each race → pass to `inspect_causality` to trace the full write history
- `event_id` values → use with `get_call_stack` to see who was calling when the race occurred
- `variable_hint` → confirms which variable is unprotected
- `delta_ns` → smaller delta = more severe race (87ns is very tight, likely causing corruption)

### Tuning `threshold_ns`

- Default 100ns catches obvious races
- Use 500ns–1000ns for programs on slow machines or with high scheduler latency
- Use 50ns for high-frequency trading or real-time code where tighter guarantees are needed

---

## `debug_expand_hotspot`

**One-line description:** Which are the top-N functions by call count and CPU cycles — the actual hottest code paths?

**When to call:** When `debug_get_saliency_scores` shows functions with saliency > 0.5, or when investigating performance regressions. Run in parallel with other bulk tools.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `top_n` | int | `10` | Number of hottest functions to return |

### Example call

```json
{
  "tool": "debug_expand_hotspot",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "top_n": 15
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "hotspots": [
    {
      "rank": 1,
      "function": "serialize_blob",
      "call_count": 12,
      "total_cycles": 9420000,
      "avg_cycles_per_call": 785000,
      "max_cycles_single_call": 1200000,
      "callers": ["process_batch", "flush_output"],
      "file": "src/serialize.rs",
      "line": 78
    },
    {
      "rank": 2,
      "function": "parse_record",
      "call_count": 48201,
      "total_cycles": 7100000,
      "avg_cycles_per_call": 147,
      "max_cycles_single_call": 48200,
      "callers": ["process_batch"],
      "file": "src/parser.rs",
      "line": 30
    }
  ]
}
```

### What to extract

- **High `avg_cycles_per_call` + low `call_count`** → single expensive operation (e.g., allocating a large buffer each call)
- **Low `avg_cycles_per_call` + very high `call_count`** → hot loop — consider batching or caching
- **`max_cycles_single_call` >> `avg_cycles_per_call`** → inconsistent performance, possible pathological input case
- `callers` → who is responsible for triggering the expensive function

---

## `performance_regression_audit`

**One-line description:** Compare two sessions (e.g., main branch vs PR) and identify functions where performance regressed, with severity scoring.

**When to call:** In CI/CD pipelines after capturing a baseline session and a target session. Run after orientation on both sessions.

**Parallel-safe?** Yes (within a session), but requires two separate session_ids from two separate `debug_run` captures.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `baseline_session_id` | string | required | Session ID of the known-good baseline (e.g., main branch) |
| `target_session_id` | string | required | Session ID of the build being evaluated (e.g., PR branch) |
| `top_n` | int | `20` | Maximum number of regressions to report |

### Example call

```json
{
  "tool": "performance_regression_audit",
  "params": {
    "baseline_session_id": "sess_baseline_main",
    "target_session_id": "sess_pr_42",
    "top_n": 20
  }
}
```

### Example response

```json
{
  "baseline_session_id": "sess_baseline_main",
  "target_session_id": "sess_pr_42",
  "overall_regression_score": 0.34,
  "verdict": "degraded",
  "regressions": [
    {
      "function": "serialize_blob",
      "severity": "critical",
      "regression_score": 0.89,
      "baseline_avg_cycles": 120000,
      "target_avg_cycles": 785000,
      "change_pct": "+554%",
      "baseline_call_count": 12,
      "target_call_count": 12
    },
    {
      "function": "parse_record",
      "severity": "medium",
      "regression_score": 0.31,
      "baseline_avg_cycles": 112,
      "target_avg_cycles": 147,
      "change_pct": "+31%",
      "baseline_call_count": 48201,
      "target_call_count": 48201
    }
  ],
  "improvements": [
    {
      "function": "hash_string",
      "baseline_avg_cycles": 2400,
      "target_avg_cycles": 890,
      "change_pct": "-63%"
    }
  ]
}
```

### What to extract

- `overall_regression_score` → use as CI gate threshold (e.g., fail if > 0.3)
- `severity: "critical"` regressions → always investigate before merging
- `regression_score` per function → prioritizes which function to fix first
- `improvements` → confirm that intended optimizations landed

### Severity levels

| Severity | Regression score | Typical meaning |
|----------|-----------------|-----------------|
| critical | > 0.7 | > 3× slowdown |
| high | 0.5–0.7 | 2–3× slowdown |
| medium | 0.3–0.5 | 1.3–2× slowdown |
| low | < 0.3 | < 30% regression |

---

## `debug_call_graph`

**One-line description:** What is the complete call graph of the execution — who called what, at what depth?

**When to call:** When you need to understand program structure, find unexpected call chains, or identify which functions are entry points to a problematic subsystem.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `max_depth` | int | `10` | Maximum call depth to include in the graph |

### Example call

```json
{
  "tool": "debug_call_graph",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "max_depth": 5
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "node_count": 47,
  "edge_count": 112,
  "max_depth_observed": 14,
  "nodes": [
    {
      "id": "main",
      "call_count": 1,
      "callees": ["process_batch", "init_config", "cleanup"]
    },
    {
      "id": "process_batch",
      "call_count": 4,
      "callees": ["parse_record", "serialize_blob", "validate_field"]
    },
    {
      "id": "parse_record",
      "call_count": 48201,
      "callees": ["validate_field", "hash_string", "alloc_record"]
    }
  ],
  "entry_points": ["main"],
  "leaf_functions": ["hash_string", "alloc_record", "free", "memcpy"]
}
```

### What to extract

- `entry_points` → confirm expected program structure
- `leaf_functions` → functions that call nothing — usually utility functions or system calls
- Unexpected edges → a function calling something it shouldn't (indicates architectural violation or injection)
- High fan-out nodes (many callees) → complex functions worth splitting
- `max_depth_observed` → very deep call graphs (> 50) risk stack overflow

---

## Bulk Analysis Decision Guide

After orientation, choose bulk tools based on findings:

```
exit_signal present?           → debug_find_crash
thread_count > 1?              → debug_detect_races
saliency score > 0.5?          → debug_expand_hotspot
have a baseline session?       → performance_regression_audit
unknown program structure?     → debug_call_graph
memory error in issues_detected? → forensic_memory_audit (forensic level)
```

These are not mutually exclusive. Run all that apply simultaneously.
