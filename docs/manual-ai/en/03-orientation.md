# 03 — Orientation Tools

Orientation tools are the **mandatory first pass** after capture. Always call all three simultaneously, immediately after receiving a `session_id`. They answer the broadest questions about the execution and guide which bulk analysis tools to invoke next.

**Rule: Never call bulk, forensic, or drill-down tools before running orientation tools.**

---

## `get_execution_summary`

**One-line description:** What happened overall? How many events, any obvious issues, top functions?

**When to call:** Immediately after `debug_run`/`debug_attach`, always in parallel with `debug_get_saliency_scores` and `list_threads`.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID from `debug_run` or `debug_attach` |

### Example call

```json
{
  "tool": "get_execution_summary",
  "params": {
    "session_id": "sess_a1b2c3d4"
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "status": "complete",
  "exit_code": 139,
  "exit_signal": "SIGSEGV",
  "duration_ms": 4231,
  "total_events": 182943,
  "event_breakdown": {
    "function_entry": 91201,
    "function_exit": 90988,
    "syscall_enter": 412,
    "syscall_exit": 412,
    "memory_write": 213,
    "thread_switch": 117
  },
  "threads": 4,
  "issues_detected": [
    {
      "type": "crash",
      "signal": "SIGSEGV",
      "at_event": 182940,
      "function": "parse_record"
    }
  ],
  "top_functions_by_call_count": [
    { "name": "parse_record",    "calls": 48201 },
    { "name": "validate_field",  "calls": 48200 },
    { "name": "hash_string",     "calls": 24100 }
  ]
}
```

### What to extract

- `exit_signal` — if present, you have a crash: go to `debug_find_crash`
- `issues_detected` — any pre-detected anomalies; use these to select bulk tools
- `total_events` — if > 500k, set tight `timestamp_start`/`timestamp_end` filters on all `query_events` calls
- `threads` — if > 1, always run `debug_detect_races` in bulk analysis

---

## `debug_get_saliency_scores`

**One-line description:** Which functions consumed disproportionately more CPU cycles than expected?

**When to call:** Immediately after capture, in parallel with `get_execution_summary` and `list_threads`. Use results to decide whether to run `debug_expand_hotspot` or `performance_regression_audit`.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `limit` | int | `20` | Maximum number of functions to score |

### Example call

```json
{
  "tool": "debug_get_saliency_scores",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "limit": 20
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "scored_functions": [
    { "function": "serialize_blob",    "saliency": 0.94, "cycles": 9420000, "call_count": 12 },
    { "function": "parse_record",      "saliency": 0.71, "cycles": 7100000, "call_count": 48201 },
    { "function": "validate_field",    "saliency": 0.43, "cycles": 4300000, "call_count": 48200 },
    { "function": "hash_string",       "saliency": 0.12, "cycles": 1200000, "call_count": 24100 },
    { "function": "free",              "saliency": 0.03, "cycles": 300000,  "call_count": 98203 }
  ]
}
```

### What to extract

- **High saliency (> 0.7) + low call count** = a slow outlier function — worth investigating with `debug_expand_hotspot`
- **High saliency + high call count** = a hot loop — consider algorithmic optimization
- **Multiple functions with similar saliency** = diffuse performance problem, not a single bottleneck
- **All saliency < 0.3** = program likely spent most time in I/O or sleep (check syscalls)

Saliency is normalized to [0.0, 1.0]. A score of 0.94 means that function consumed 94% of observed CPU cycles relative to the most expensive function in the session.

---

## `list_threads`

**One-line description:** How many threads ran during execution, and what are their IDs?

**When to call:** Immediately after capture, in parallel with the other two orientation tools. Thread IDs are required as filters for `query_events`, `debug_detect_races`, and per-thread analysis.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |

### Example call

```json
{
  "tool": "list_threads",
  "params": {
    "session_id": "sess_a1b2c3d4"
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "thread_count": 4,
  "threads": [
    { "thread_id": 1, "name": "main",          "event_count": 91201, "first_event": 1,      "last_event": 182940 },
    { "thread_id": 2, "name": "worker-pool-0", "event_count": 48203, "first_event": 1021,   "last_event": 180112 },
    { "thread_id": 3, "name": "worker-pool-1", "event_count": 39211, "first_event": 1022,   "last_event": 179831 },
    { "thread_id": 4, "name": "gc-thread",     "event_count": 4328,  "first_event": 10421,  "last_event": 170031 }
  ]
}
```

### What to extract

- **Thread count > 1** → always run `debug_detect_races` in bulk analysis
- **Thread names** → identify which threads are relevant (e.g., filter `query_events` by `thread_id` for the crashing thread)
- **Event counts per thread** → a thread with very low event count but significant CPU (from saliency) may be doing expensive work in a tight loop that Chronos sampled sparsely
- **first_event / last_event** → use these as `timestamp_start`/`timestamp_end` bounds when querying thread-specific events

---

## Parallel Orientation Template

Always use this pattern immediately after receiving a `session_id`:

```json
[
  {
    "tool": "get_execution_summary",
    "params": { "session_id": "REPLACE_WITH_SESSION_ID" }
  },
  {
    "tool": "debug_get_saliency_scores",
    "params": { "session_id": "REPLACE_WITH_SESSION_ID", "limit": 20 }
  },
  {
    "tool": "list_threads",
    "params": { "session_id": "REPLACE_WITH_SESSION_ID" }
  }
]
```

## Decision Matrix After Orientation

| Observation | Next bulk tools to run |
|-------------|------------------------|
| `exit_signal` is SIGSEGV/SIGABRT | `debug_find_crash` |
| `thread_count` > 1 | `debug_detect_races` |
| Any function with saliency > 0.6 | `debug_expand_hotspot` |
| Comparing against known baseline | `performance_regression_audit` |
| Want full call structure | `debug_call_graph` |
| `issues_detected` contains memory errors | `forensic_memory_audit` + `inspect_causality` |
