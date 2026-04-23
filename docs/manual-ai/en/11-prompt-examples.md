# 11 — AI Agent Prompt Examples

This chapter contains 20 complete workflow examples. Each shows a realistic user prompt to an AI agent, the sequence of Chronos tool calls the agent should make, and the key insights to extract.

The format for each example:
1. **User prompt** — what the user says to the AI agent
2. **Chronos sequence** — the tool calls in recommended order
3. **Key insights** — what the agent should extract and report

---

## Example 1: Native Rust Crash Investigation

**User:**
> "Our Rust service crashed with exit code 101. Can you find out what happened? Binary is at /usr/bin/chronos-service."

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/chronos-service", "trace_syscalls": true } }
]
```
// Wait for result with session_id

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_find_crash", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } }
]
```

**Key insights to extract:**
- Was it a panic? SIGABRT? SIGSEGV?
- Function at crash point
- Call stack leading to crash
- Which thread crashed (main? worker?)

---

## Example 2: Performance Regression Between Two Builds

**User:**
> "We shipped a new version and P99 latency went up 40ms. Can you compare the performance profile? Baseline binary: /usr/bin/service_v1, New binary: /usr/bin/service_v2."

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/service_v1", "trace_syscalls": false, "auto_save": true } },
  { "tool": "debug_run", "params": { "program": "/usr/bin/service_v2", "trace_syscalls": false, "auto_save": true } }
]
```
// Get two session_ids: baseline_id, current_id

```json
[
  { "tool": "performance_regression_audit", "params": { "baseline_session_id": "${BASELINE}", "target_session_id": "${CURRENT}", "top_n": 20 } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${BASELINE}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${CURRENT}" } }
]
```

**Key insights to extract:**
- `regression_score` (0.0–1.0) and severity
- Which specific functions degraded
- Call count changes between versions
- New functions introduced in the slow path

---

## Example 3: Data Race in Concurrent C Code

**User:**
> "We have a intermittent crash in our C++ multithreaded server. Suspect a data race. Can you look for it?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./c++-server", "trace_syscalls": false } }
]
```

```json
[
  { "tool": "debug_detect_races", "params": { "session_id": "${SESSION}", "threshold_ns": 100 } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```
// If races found:

```json
[
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "thread_id": "${RACE_THREAD_A}",
      "event_types": ["memory_write", "variable_write"],
      "limit": 50
    }
  },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "thread_id": "${RACE_THREAD_B}",
      "event_types": ["memory_write", "variable_write"],
      "limit": 50
    }
  }
]
```

**Key insights to extract:**
- All races found (not just the first one)
- Addresses involved in races
- Timestamp ordering of conflicting writes
- The variable/address name if resolvable

---

## Example 4: Memory Corruption / Buffer Overflow

**User:**
> "Our service starts fine but degrades over 10 minutes and eventually crashes. Likely memory corruption. Can you trace memory writes?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false } }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_find_crash", "params": { "session_id": "${SESSION}" } }
]
```
// If crash found, get forensic audit of the crash address:

```json
{
  "tool": "forensic_memory_audit",
  "params": {
    "session_id": "${SESSION}",
    "address": "${CRASH_ADDRESS}",
    "limit": 100
  }
}
```

**Key insights to extract:**
- All writes to the corrupted address over time
- Which function performed each write
- Timeline of corruption (was it gradual or sudden?)
- The value written at each step

---

## Example 5: Python Service Debugging

**User:**
> "Our Python API server is hanging on /api/users endpoint. Process is running with debugpy on port 5678. Can you find the bottleneck?"

**Prerequisites:** User started `python -m debugpy --listen 127.0.0.1:5678 --wait-for-client api_server.py`

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "api_server.py",
      "program_language": "python",
      "debug_host": "127.0.0.1",
      "debug_port": 5678,
      "wait_for_connection": true,
      "args": ["--endpoint", "/api/users"]
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 10 } }
]
```

**Key insights to extract:**
- Which Python functions consumed most time
- Call counts vs time consumption (discrepancy = I/O bound?)
- The full call chain from entry to bottleneck

---

## Example 6: JavaScript/Node.js Debugging

**User:**
> "Our Node.js microservice throws an unhandled promise rejection every ~5 minutes. Can you trace the rejection source?"

**Prerequisites:** User started `node --inspect=127.0.0.1:9229 server.js`

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "server.js",
      "program_language": "nodejs",
      "debug_host": "127.0.0.1",
      "debug_port": 9229,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["exception_thrown"],
      "limit": 10
    }
  },
  { "tool": "debug_call_graph", "params": { "session_id": "${SESSION}", "max_depth": 8 } }
]
```

**Key insights to extract:**
- Exception type and message
- Stack trace at rejection point
- Promise chain leading to the unhandled rejection

---

## Example 7: Java Application Debugging (JDWP)

**User:**
> "Our Java Spring Boot app hangs on startup. JVM args: -agentlib:jdwp=transport=dt_socket,server=y,suspend=n,address=*:5005. Can you find where it's stuck?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "/usr/bin/java",
      "args": ["-jar", "app.jar"],
      "program_language": "java",
      "debug_host": "127.0.0.1",
      "debug_port": 5005,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } }
]
```
// Then get stacks of all blocked/waiting threads:

```json
[
  { "tool": "get_call_stack", "params": { "session_id": "${SESSION}", "event_id": "${STUCK_EVENT_ID}" } }
]
```

**Key insights to extract:**
- Which thread is stuck (main? a pool thread?)
- Monitor/lock contention
- Network or database call blocking

---

## Example 8: Go Service Debugging (Delve)

**User:**
> "Our Go HTTP server has goroutine leaks after handling 1000 requests. Can you find what's not being cleaned up? Delve running on port 38657."

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "./http-server",
      "program_language": "go",
      "debug_host": "127.0.0.1",
      "debug_port": 38657,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 20 } }
]
```

**Key insights to extract:**
- Total goroutine count at start vs end
- Which goroutines are in wait state (chan receive, select on closed channel, etc.)
- Functions creating goroutines that aren't terminating

---

## Example 9: Python Calling Rust via FFI — Boundary Tracing

**User:**
> "Our Python service calls a Rust library via ctypes and crashes when we pass large arrays. Can you trace what happens at the FFI boundary?"

**Prerequisites:** Both processes running with debugpy on Python side, Rust compiled with debug symbols.

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": {
      "program": "python3",
      "args": ["-m", "debugpy", "--listen", "127.0.0.1:5678", "--wait-for-client", "-c", "from rust_lib import process_array; process_array(large_data)"],
      "program_language": "python",
      "debug_host": "127.0.0.1",
      "debug_port": 5678,
      "wait_for_connection": true
    }
  }
]
```

```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["function_entry", "function_exit"],
      "function_pattern": "*rust*",
      "limit": 50
    }
  },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 15 } }
]
```

**Key insights to extract:**
- Functions at the FFI boundary (Rust functions called from Python)
- Parameters passed across the boundary
- The crash location relative to the FFI call

---

## Example 10: CI/CD Regression Gate

**User:**
> "As part of our CI pipeline, we need to fail the build if the new version has more than 10% regression in the hot path functions. Baseline is in store as 'baseline_sha_abc123'."

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false, "auto_save": true } }
]
// New build session: current_id

[
  { "tool": "load_session", "params": { "session_id": "baseline_sha_abc123" } },
  { "tool": "performance_regression_audit", "params": {
      "baseline_session_id": "baseline_sha_abc123",
      "target_session_id": "${CURRENT}",
      "top_n": 50
    }
  }
]
```

**Key insights to extract:**
- `regression_score` — if > 0.1 (10%), fail the build
- `total_regressions` count
- `critical_count` — any critical regressions fail immediately
- Specific functions that regressed and by how much

**AI agent decision:**
```python
if regression.regression_score > 0.1:
    fail_build(f"Regression score {regression.regression_score:.2f} exceeds threshold")
elif regression.critical_count > 0:
    fail_build(f"{regression.critical_count} critical regressions found")
```

---

## Example 11: Production Incident Replay

**User:**
> "We had an incident last Tuesday at 14:32 UTC. The on-call engineer saved a session as 'incident_0420_1432'. Can you load it and figure out what went wrong?"

**Chronos sequence:**
```json
[
  { "tool": "load_session", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "get_execution_summary", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "debug_find_crash", "params": { "session_id": "incident_0420_1432" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "incident_0420_1432", "limit": 20 } }
]
```

**Key insights to extract:**
- What was the end state of the session (exited? signaled?)
- Hot functions at time of incident
- Memory or CPU anomalies

---

## Example 12: Memory Leak Detection

**User:**
> "Our service's RSS grows from 200MB to 800MB over 1 hour. No OOM, but it keeps growing. Can you find alloc/free imbalances?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": false } }
]
// Run for representative workload

[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 20 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 10 } }
]
```
// Look for malloc/free imbalances in hotspot functions

```json
[
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["function_entry", "function_exit"],
      "function_pattern": "*alloc*",
      "limit": 100
    }
  }
]
```

**Key insights to extract:**
- Which allocation functions are most called
- Allocation counts vs free counts in hotspot functions
- Growth trend of allocations over time

---

## Example 13: Slow Function Identification

**User:**
> "Our API has a 500ms latency spike on the /orders endpoint. Can you identify which function is the bottleneck?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./api-server", "args": ["--endpoint", "/orders"] } }
]

[
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 10 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 5 } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```

**Key insights to extract:**
- The top scoring function (highest CPU time)
- Call count vs time — a function called once that takes 400ms is the bottleneck
- Nested hot functions — caller vs callee

---

## Example 14: System Call Analysis

**User:**
> "We suspect our service is making too many redundant file system calls. Can you analyze the syscalls?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service", "trace_syscalls": true } }
]

[
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "event_types": ["syscall_enter"],
      "limit": 100
    }
  },
  { "tool": "debug_call_graph", "params": { "session_id": "${SESSION}", "max_depth": 6 } }
]
```

**Key insights to extract:**
- Syscall frequency by type (open, read, write, stat)
- Redundant patterns (same file opened N times without closing)
- Functions making the most syscalls

---

## Example 15: Variable Value Tracing

**User:**
> "Somewhere in our code, a variable `total` becomes -1 when it shouldn't. Can you trace how it got there?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./service" } }
]

[
  { "tool": "debug_find_variable_origin", "params": { "session_id": "${SESSION}", "variable_name": "total", "limit": 50 } },
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } }
]
```

**Key insights to extract:**
- All mutations to `total` with timestamps
- The function that first set it to -1
- The call path leading to that function

---

## Example 16: Thread Interleaving Analysis

**User:**
> "We have a race condition in thread synchronization. Can you analyze how the threads interleaved around timestamp 5s into the trace?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "./mt-service" } }
]

[
  { "tool": "list_threads", "params": { "session_id": "${SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${SESSION}",
      "timestamp_start": 5000000000,
      "timestamp_end": 5100000000,
      "limit": 200
    }
  }
]
```

**Key insights to extract:**
- Event ordering across threads in the window
- Lock acquisition sequences
- Potential deadlock patterns

---

## Example 17: eBPF Kernel Probe Analysis

**User:**
> "Can you analyze system call latency using eBPF? Our trace is in a session captured with program_language=ebpf."

**Chronos sequence:**
```json
[
  { "tool": "load_session", "params": { "session_id": "${EBPF_SESSION}" } },
  { "tool": "get_execution_summary", "params": { "session_id": "${EBPF_SESSION}" } },
  { "tool": "query_events", "params": {
      "session_id": "${EBPF_SESSION}",
      "event_types": ["syscall_enter", "syscall_exit"],
      "limit": 100
    }
  }
]
```

**Key insights to extract:**
- Syscall enter/exit pairs and latency per call
- High-latency syscalls (e.g., recvfrom, write taking unusually long)
- Frequency distribution of syscall types

---

## Example 18: Staging vs Production Comparison

**User:**
> "The service works in staging but fails in production. Can you compare the production trace against our staging baseline?"

**Chronos sequence:**
```json
[
  { "tool": "load_session", "params": { "session_id": "staging_baseline" } },
  { "tool": "load_session", "params": { "session_id": "prod_incident_trace" } }
]

[
  { "tool": "compare_sessions", "params": { "session_a": "staging_baseline", "session_b": "prod_incident_trace" } },
  { "tool": "performance_regression_audit", "params": {
      "baseline_session_id": "staging_baseline",
      "target_session_id": "prod_incident_trace",
      "top_n": 30
    }
  }
]
```

**Key insights to extract:**
- Functions present in production but not staging
- Functions with different call counts
- Performance degradation specific to production environment

---

## Example 19: Test Failure Forensics

**User:**
> "Our integration test 'test_user_creation' fails in CI but passes locally. Can you compare the passing local trace against the failing CI trace?"

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "pytest", "args": ["-v", "tests/test_user_creation.py"] } }
]
// local_session and ci_session captured separately

[
  { "tool": "compare_sessions", "params": { "session_a": "local_session", "session_b": "ci_session" } },
  { "tool": "debug_detect_races", "params": { "session_id": "ci_session" } },
  { "tool": "get_execution_summary", "params": { "session_id": "ci_session" } }
]
```

**Key insights to extract:**
- Any additional events in the failing trace
- Race conditions specific to CI environment
- Environment differences (different config files loaded, different code paths)

---

## Example 20: Long-Running Service Profiling

**User:**
> "We have a daemon that runs for 24 hours. We need a CPU profile after 1 hour of operation. Can you capture and analyze it?"

**Note:** For truly long-running services, use background mode and load the session after.

**Chronos sequence:**
```json
[
  { "tool": "debug_run", "params": { "program": "/usr/bin/daemon", "background": true } }
]
// Wait for session to be available (polling or just retry load after delay)

[
  { "tool": "get_session_status", "params": { "session_id": "${SESSION}" } }
]
// If status is "running", wait and retry
// Once "finalized":
[
  { "tool": "get_execution_summary", "params": { "session_id": "${SESSION}" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "${SESSION}", "limit": 30 } },
  { "tool": "debug_expand_hotspot", "params": { "session_id": "${SESSION}", "top_n": 15 } }
]
```

**Key insights to extract:**
- Top 30 hot functions over the full run
- Whether CPU usage is concentrated or spread
- Functions whose hot path evolved over time

---

## Summary: When to Use Each Tool

| Scenario | Primary tools |
|----------|---------------|
| Crash investigation | `debug_find_crash` → `get_call_stack` → `debug_get_variables` |
| Performance regression | `performance_regression_audit` → `debug_get_saliency_scores` |
| Data race detection | `debug_detect_races` → `query_events` (filtered by thread) |
| Memory corruption | `forensic_memory_audit` → `inspect_causality` |
| Slow function | `debug_get_saliency_scores` → `debug_expand_hotspot` |
| Variable tracing | `debug_find_variable_origin` → `query_events` (time range) |
| Multi-build comparison | `compare_sessions` → `performance_regression_audit` |
| Production incident | `load_session` → orientation batch → targeted drill-down |
| CI/CD gate | `debug_run` → `save_session` → regression audit |
| Python/JS debugging | `debug_run` with `debug_port` → orientation batch |
