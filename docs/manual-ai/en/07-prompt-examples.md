# 07 — Prompt Examples

This chapter contains 25 complete workflow examples demonstrating realistic debugging scenarios. Each example shows the user's natural language prompt followed by the sequence of MCP tool calls an AI agent would make to fulfill the request.

---

## 1. Crash Investigation — Rust SIGSEGV

**User prompt:** _"My Rust server crashes with a segfault when I send a malformed JSON payload. Trace it and find the root cause."_

```json
// Step 1: Capture the crash
{
  "name": "debug_run",
  "arguments": {
    "program": "./target/debug/server",
    "args": ["--input", "malformed.json"],
    "timeout_secs": 15
  }
}
// → { "session_id": "sess_crash01", "event_count": 4821 }

// Step 2: Find the crash point
{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_crash01" }
}
// → { "crash_found": true, "signal": "SIGSEGV", "crash_event_id": 4820, "faulting_address": "0x8" }

// Step 3: Get the full backtrace at crash
{
  "name": "get_backtrace",
  "arguments": {
    "session_id": "sess_crash01",
    "event_id": 4820,
    "max_depth": 30
  }
}
// → frames: [parse_json_field, handle_request, main_loop, main]

// Step 4: Inspect what the parser was doing just before the crash
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_crash01",
    "event_id": 4819
  }
}

// Step 5: Check variables in scope at the crash event
{
  "name": "debug_get_variables",
  "arguments": {
    "session_id": "sess_crash01",
    "event_id": 4820
  }
}
// → { variables: [{ name: "field_ptr", value: "0x8", type: "*const u8" }] }

// Step 6: Save the session for later reference
{
  "name": "save_session",
  "arguments": {
    "session_id": "sess_crash01",
    "language": "rust",
    "target": "./target/debug/server"
  }
}
```

**Conclusion:** `field_ptr` is `0x8` — a near-null pointer dereference resulting from an unhandled `None` being cast to a raw pointer. Root cause identified at `parse_json_field` in `src/parser.rs`.

---

## 2. Data Race Detection — Multithreaded C++ Server

**User prompt:** _"I'm seeing random memory corruption in my C++ server. Check if there are any data races."_

```json
// Step 1: Capture a run
{
  "name": "debug_run",
  "arguments": {
    "program": "./server",
    "args": ["--workers", "8"],
    "timeout_secs": 20,
    "trace_syscalls": false
  }
}
// → { "session_id": "sess_race01", "event_count": 210000 }

// Step 2: Detect races (broad threshold)
{
  "name": "debug_detect_races",
  "arguments": {
    "session_id": "sess_race01",
    "threshold_ns": 1000
  }
}
// → { "races_found": 3, "races": [...] }

// Step 3: Inspect causality for the first racy address
{
  "name": "inspect_causality",
  "arguments": {
    "session_id": "sess_race01",
    "address": 140234567890,
    "limit": 20
  }
}
// → all reads and writes to that address, with thread IDs

// Step 4: Get call stacks for the two racing events
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_race01",
    "event_id": 45100
  }
}
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_race01",
    "event_id": 45105
  }
}
```

---

## 3. Performance Regression — Before/After Comparison

**User prompt:** _"The new release seems slower than v1.0. Compare the two traces and tell me which functions regressed."_

```json
// Step 1: Load both sessions (already saved)
{
  "name": "load_session",
  "arguments": { "session_id": "sess_v1_0_0" }
}
{
  "name": "load_session",
  "arguments": { "session_id": "sess_v1_1_0" }
}

// Step 2: Run the regression audit
{
  "name": "performance_regression_audit",
  "arguments": {
    "baseline_session_id": "sess_v1_0_0",
    "target_session_id": "sess_v1_1_0",
    "top_n": 25
  }
}
// → { "regression_score": 0.81, "severity": "critical", "critical_count": 2 }

// Step 3: Expand hotspots in the regressed session
{
  "name": "debug_expand_hotspot",
  "arguments": {
    "session_id": "sess_v1_1_0",
    "top_n": 10
  }
}

// Step 4: Get saliency scores to prioritize investigation
{
  "name": "debug_get_saliency_scores",
  "arguments": {
    "session_id": "sess_v1_1_0",
    "limit": 10
  }
}
```

---

## 4. Python Script Debugging

**User prompt:** _"My Python data pipeline crashes with an index error. Debug it."_

```bash
# First, start the script with debugpy:
python -m debugpy --listen 127.0.0.1:5678 --wait-for-client pipeline.py
```

```json
// Step 1: Connect and capture
{
  "name": "debug_run",
  "arguments": {
    "program": "pipeline.py",
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true,
    "timeout_secs": 60
  }
}
// → { "session_id": "sess_py01" }

// Step 2: Find the crash
{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_py01" }
}
// → { "signal": "IndexError", "crash_event_id": 1204 }

// Step 3: Get variables at crash point
{
  "name": "debug_get_variables",
  "arguments": {
    "session_id": "sess_py01",
    "event_id": 1204
  }
}
// → { variables: [{ name: "idx", value: "100" }, { name: "data", value: "list[len=50]" }] }

// Step 4: Evaluate expression to confirm
{
  "name": "evaluate_expression",
  "arguments": {
    "session_id": "sess_py01",
    "event_id": 1204,
    "expression": "idx - len(data)"
  }
}
// → { "result": "50" } — confirms out-of-bounds access
```

---

## 5. Node.js Memory Leak Investigation

**User prompt:** _"My Node.js API server's memory grows unbounded. Trace it and find the hot paths."_

```bash
node --inspect=127.0.0.1:9229 api_server.js
```

```json
// Step 1: Connect
{
  "name": "debug_run",
  "arguments": {
    "program": "api_server.js",
    "program_language": "javascript",
    "debug_host": "127.0.0.1",
    "debug_port": 9229,
    "wait_for_connection": true,
    "timeout_secs": 30
  }
}

// Step 2: Get execution summary
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_node01" }
}

// Step 3: Expand hotspots
{
  "name": "debug_expand_hotspot",
  "arguments": {
    "session_id": "sess_node01",
    "top_n": 20
  }
}
// → Top function: "addToCache" called 50,000 times

// Step 4: Trace origin of the cache variable
{
  "name": "debug_find_variable_origin",
  "arguments": {
    "session_id": "sess_node01",
    "variable_name": "requestCache",
    "limit": 100
  }
}
// → Never cleared — grows monotonically
```

---

## 6. Security Audit — Unauthorized Memory Write

**User prompt:** _"I want to verify that the `auth_token` variable is only ever written by the authentication module."_

```json
// Step 1: Capture
{
  "name": "debug_run",
  "arguments": {
    "program": "./myapp",
    "timeout_secs": 30
  }
}
// → { "session_id": "sess_sec01" }

// Step 2: Forensic audit of auth_token's address
{
  "name": "forensic_memory_audit",
  "arguments": {
    "session_id": "sess_sec01",
    "address": 140234567890,
    "limit": 100
  }
}
// → Shows write from "inject_payload" at event 2210 — unexpected!

// Step 3: Inspect the call stack at the suspicious write
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_sec01",
    "event_id": 2210
  }
}
// → Stack: [inject_payload, process_user_input, main]
// Confirms unauthorized write path
```

---

## 7. Real-Time Watchpoint Monitoring

**User prompt:** _"Watch the `global_error_count` variable while the server processes requests. Alert me when it changes."_

```json
// Step 1: Start background session
{
  "name": "debug_run",
  "arguments": {
    "program": "./server",
    "background": true
  }
}
// → { "session_id": "sess_bg01" }

// Step 2: Subscribe to the variable
{
  "name": "subscribe_to_symbol",
  "arguments": {
    "session_id": "sess_bg01",
    "symbol": "global_error_count",
    "watch_type": "write"
  }
}
// → { "subscription_id": "sub_err01" }

// Step 3: Poll for changes every 2 seconds
{
  "name": "get_subscription_events",
  "arguments": {
    "subscription_id": "sub_err01",
    "limit": 10,
    "timeout_ms": 2000
  }
}
// → { events: [{ timestamp_ns: ..., value: "0x3" }] }

// Step 4: Clean up
{
  "name": "unsubscribe_from_symbol",
  "arguments": { "subscription_id": "sub_err01" }
}
```

---

## 8. Call Graph Visualization

**User prompt:** _"Show me the complete call graph for this session so I can understand the architecture."_

```json
{
  "name": "debug_call_graph",
  "arguments": {
    "session_id": "sess_abc123",
    "max_depth": 8
  }
}
// → nodes[], edges[] — can be rendered as a Mermaid/DOT graph
```

---

## 9. Java Spring Boot Debugging

**User prompt:** _"My Spring Boot service throws a NullPointerException during startup. Debug it."_

```bash
java -agentlib:jdwp=transport=dt_socket,server=y,suspend=y,address=*:5005 -jar app.jar
```

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "app.jar",
    "program_language": "java",
    "debug_host": "127.0.0.1",
    "debug_port": 5005,
    "wait_for_connection": true
  }
}

{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_java01" }
}
// → NullPointerException in BeanFactory.createBean

{
  "name": "debug_get_variables",
  "arguments": {
    "session_id": "sess_java01",
    "event_id": 820
  }
}
// → { variables: [{ name: "dependency", value: "null" }] }
```

---

## 10. Go Goroutine Race Condition

**User prompt:** _"Find the race condition in my Go service. I already have a trace from `go -race`."_

```json
{
  "name": "load_session",
  "arguments": { "session_id": "sess_go_race" }
}

{
  "name": "debug_detect_races",
  "arguments": {
    "session_id": "sess_go_race",
    "threshold_ns": 200
  }
}
// → { "races_found": 1, "races": [{ "thread_a": 5, "thread_b": 7 }] }

{
  "name": "inspect_causality",
  "arguments": {
    "session_id": "sess_go_race",
    "address": 140234567890,
    "limit": 10
  }
}
```

---

## 11. Variable Mutation Timeline

**User prompt:** _"I need to see every time `connection_count` was modified during the trace."_

```json
{
  "name": "debug_find_variable_origin",
  "arguments": {
    "session_id": "sess_abc123",
    "variable_name": "connection_count",
    "limit": 200
  }
}
// → Complete mutation timeline with values, timestamps, functions
```

---

## 12. Session Persistence and Reload

**User prompt:** _"Save this session and reload it tomorrow for deeper analysis."_

```json
// Save
{
  "name": "save_session",
  "arguments": {
    "session_id": "sess_abc123",
    "language": "rust",
    "target": "./target/release/myapp"
  }
}

// Free memory
{
  "name": "drop_session",
  "arguments": { "session_id": "sess_abc123" }
}

// --- Next day ---
// Reload from disk
{
  "name": "load_session",
  "arguments": { "session_id": "sess_abc123" }
}
```

---

## 13. Compare Two Builds

**User prompt:** _"Did my refactoring change the execution behavior? Compare before and after."_

```json
{
  "name": "compare_sessions",
  "arguments": {
    "session_a": "sess_before_refactor",
    "session_b": "sess_after_refactor"
  }
}
// → { "are_identical": false, "new_functions": ["new_helper"], "delta_events": 1240 }
```

---

## 14. Syscall Analysis

**User prompt:** _"What system calls does this binary make? Are there any unexpected ones?"_

```json
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "event_types": ["syscall_enter"],
    "limit": 500
  }
}

{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_abc123" }
}
// Cross-reference syscall events with expected behavior
```

---

## 15. Time-Travel to a Specific Moment

**User prompt:** _"Go back to exactly when the HTTP request was received and show me the state."_

```json
// Find the event
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry"],
    "function_pattern": "handle_http_request",
    "limit": 1
  }
}
// → { events: [{ event_id: 1500, timestamp_ns: 1234567890000 }] }

// Inspect state at that moment
{
  "name": "debug_get_variables",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 1500
  }
}

{
  "name": "debug_get_registers",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 1500
  }
}
```

---

## 16. Memory Access Pattern Analysis

**User prompt:** _"Analyze all memory accesses to the buffer pool between timestamps T1 and T2."_

```json
{
  "name": "debug_analyze_memory",
  "arguments": {
    "session_id": "sess_abc123",
    "start_address": 140234500000,
    "end_address": 140234600000,
    "start_ts": 1234510000000,
    "end_ts": 1234599000000
  }
}
// → 240 total accesses, 180 reads, 60 writes
```

---

## 17. CPU Register Snapshot

**User prompt:** _"What were the CPU registers when the overflow occurred?"_

```json
// Find the overflow event
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "event_types": ["signal"],
    "limit": 5
  }
}
// → event_id: 8200

{
  "name": "debug_get_registers",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 8200
  }
}
// → rax: 0xffffffffffffffff (overflow indicator)
```

---

## 18. Expression Evaluation for Root Cause

**User prompt:** _"At the point of failure, what was the value of `buffer_size - bytes_written`?"_

```json
{
  "name": "evaluate_expression",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 3050,
    "expression": "buffer_size - bytes_written"
  }
}
// → { "result": "-12" } — buffer overrun confirmed
```

---

## 19. Thread-Specific Trace Analysis

**User prompt:** _"Show me only what thread 3 was doing during the trace."_

```json
// List threads first
{
  "name": "list_threads",
  "arguments": { "session_id": "sess_abc123" }
}

// Query events for thread 3 only
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "thread_id": 3,
    "event_types": ["function_entry", "function_exit"],
    "limit": 200
  }
}
```

---

## 20. Background Long-Running Service

**User prompt:** _"Start a background trace of my daemon, let it run for 2 minutes, then analyze it."_

```json
// Step 1: Start background
{
  "name": "debug_run",
  "arguments": {
    "program": "/usr/sbin/mydaemon",
    "background": true,
    "timeout_secs": 120,
    "max_events": 2000000
  }
}
// → { "session_id": "sess_bg_daemon" }

// Step 2: Poll status
{
  "name": "get_session_status",
  "arguments": { "session_id": "sess_bg_daemon" }
}
// → { "status": "running", "event_count": 450000, "elapsed_ms": 45000 }

// (wait until status = "completed")

// Step 3: Analyze
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_bg_daemon" }
}
{
  "name": "debug_expand_hotspot",
  "arguments": { "session_id": "sess_bg_daemon", "top_n": 10 }
}
```

---

## 21. State Change Between Two Moments

**User prompt:** _"What changed between the start of the request and when the error was raised?"_

```json
// Find request start and error events
{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "function_pattern": "begin_request",
    "limit": 1
  }
}
// → event_id: 1000, timestamp_ns: 1234567000000

{
  "name": "query_events",
  "arguments": {
    "session_id": "sess_abc123",
    "function_pattern": "raise_error",
    "limit": 1
  }
}
// → event_id: 2000, timestamp_ns: 1234599000000

// Compare state
{
  "name": "debug_diff",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id_a": 1000,
    "event_id_b": 2000
  }
}
// → register_diff: { rax: { before: "0x0", after: "0x1" } }
```

---

## 22. eBPF Production Tracing

**User prompt:** _"Trace nginx in production using eBPF without modifying it."_

```json
{
  "name": "debug_run",
  "arguments": {
    "program": "/usr/sbin/nginx",
    "program_language": "ebpf",
    "trace_syscalls": true,
    "timeout_secs": 60,
    "background": true
  }
}
// → { "session_id": "sess_ebpf01" }

{
  "name": "get_session_status",
  "arguments": { "session_id": "sess_ebpf01" }
}

// After completion:
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_ebpf01" }
}
```

---

## 23. List and Manage Sessions

**User prompt:** _"Show me all saved sessions and delete the ones older than last week."_

```json
// List all sessions
{
  "name": "list_sessions",
  "arguments": {}
}
// → [{ session_id: "sess_old1", created_at: "2025-01-08..." }, ...]

// Delete old sessions
{
  "name": "delete_session",
  "arguments": { "session_id": "sess_old1" }
}
{
  "name": "delete_session",
  "arguments": { "session_id": "sess_old2" }
}
```

---

## 24. Causality Chain for a Critical Address

**User prompt:** _"I need to understand who wrote to the config struct at address 0x20a6dba892 and in what order."_

```json
{
  "name": "inspect_causality",
  "arguments": {
    "session_id": "sess_abc123",
    "address": 140234567890,
    "limit": 50
  }
}
// → Ordered timeline of all reads/writes with functions and thread IDs

// For each suspicious write, get the full call stack
{
  "name": "get_call_stack",
  "arguments": {
    "session_id": "sess_abc123",
    "event_id": 850
  }
}
```

---

## 25. Full Diagnostic Workflow

**User prompt:** _"Run a complete diagnostic on this binary. I want crashes, races, hotspots, and saliency scores."_

```json
// Step 1: Capture
{
  "name": "debug_run",
  "arguments": {
    "program": "./target/debug/myapp",
    "timeout_secs": 30,
    "auto_save": true
  }
}
// → { "session_id": "sess_diag01" }

// Step 2: Summary overview
{
  "name": "get_execution_summary",
  "arguments": { "session_id": "sess_diag01" }
}

// Step 3: Crash analysis
{
  "name": "debug_find_crash",
  "arguments": { "session_id": "sess_diag01" }
}

// Step 4: Race detection
{
  "name": "debug_detect_races",
  "arguments": {
    "session_id": "sess_diag01",
    "threshold_ns": 500
  }
}

// Step 5: Hotspot analysis
{
  "name": "debug_expand_hotspot",
  "arguments": {
    "session_id": "sess_diag01",
    "top_n": 15
  }
}

// Step 6: Saliency scoring
{
  "name": "debug_get_saliency_scores",
  "arguments": {
    "session_id": "sess_diag01",
    "limit": 15
  }
}

// Step 7: Call graph for architecture overview
{
  "name": "debug_call_graph",
  "arguments": {
    "session_id": "sess_diag01",
    "max_depth": 6
  }
}
```

**Summary output:** The agent now has crashes, races, performance profile, and architecture overview — enough for a complete root-cause analysis report.
