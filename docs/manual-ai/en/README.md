# Chronos MCP — AI Agent Manual

**Chronos** is a time-travel debugging (TTD) platform exposed via the Model Context Protocol (MCP). It enables AI agents to capture, replay, and analyze program execution traces across multiple languages and runtimes.

---

## Architecture Overview

```
User / AI Agent
      │
      ▼  MCP (JSON-RPC 2.0)
┌─────────────────┐
│  ChronosServer  │  ← chronos-mcp crate
│  (35 tools)     │
└────────┬────────┘
         │
   ┌─────┴──────┐
   │            │
QueryEngine  SessionStore
(in-memory)  (redb on disk)
   │
   ├── ptrace  (C/C++/Rust/eBPF)
   ├── JDWP    (Java)
   ├── DAP     (Python / Go Delve)
   └── CDP     (JavaScript/Node.js)
```

### Session States

```
[capture] ──debug_run──► [in-memory] ──save_session──► [persisted]
                              │                              │
                         drop_session                  load_session
                              │                              │
                           (gone)                    [in-memory again]
```

---

## Tool Reference Index

### Capture & Session Lifecycle (10 tools)

| Tool | Description |
|------|-------------|
| [`debug_run`](01-getting-started.md#debug_run) | Launch a program under time-travel capture |
| [`debug_attach`](02-session-lifecycle.md#debug_attach) | Attach to a running process by PID |
| [`debug_stop`](02-session-lifecycle.md#debug_stop) | Stop an active capture session |
| [`get_session_status`](02-session-lifecycle.md#get_session_status) | Query status of a background session |
| [`drop_session`](02-session-lifecycle.md#drop_session) | Remove session from memory (idempotent, no storage impact) |
| [`delete_session`](02-session-lifecycle.md#delete_session) | Remove session from both storage and memory |
| [`save_session`](02-session-lifecycle.md#save_session) | Persist in-memory session to disk store |
| [`load_session`](02-session-lifecycle.md#load_session) | Load a persisted session into memory |
| [`list_sessions`](02-session-lifecycle.md#list_sessions) | List all persisted sessions |
| [`compare_sessions`](02-session-lifecycle.md#compare_sessions) | Hash-based diff between two sessions |

### Query & Inspection (7 tools)

| Tool | Description |
|------|-------------|
| [`query_events`](03-query-inspection.md#query_events) | Query trace events with filters |
| [`get_event`](03-query-inspection.md#get_event) | Retrieve a single event by ID |
| [`get_call_stack`](03-query-inspection.md#get_call_stack) | Reconstruct call stack at an event |
| [`get_execution_summary`](03-query-inspection.md#get_execution_summary) | Summary: counts, top functions, issues |
| [`get_backtrace`](03-query-inspection.md#get_backtrace) | Full backtrace at an event (depth ≤ 50) |
| [`list_threads`](03-query-inspection.md#list_threads) | List all thread IDs in the trace |
| [`state_diff`](03-query-inspection.md#state_diff) | Compare CPU register state between two timestamps |

### Advanced Analysis (15 tools)

| Tool | Description |
|------|-------------|
| [`debug_call_graph`](04-advanced-analysis.md#debug_call_graph) | Build the full call graph |
| [`debug_find_variable_origin`](04-advanced-analysis.md#debug_find_variable_origin) | Trace all mutations to a variable |
| [`debug_find_crash`](04-advanced-analysis.md#debug_find_crash) | Identify crash point and stack at fatal signal |
| [`debug_detect_races`](04-advanced-analysis.md#debug_detect_races) | Detect concurrent write races |
| [`inspect_causality`](04-advanced-analysis.md#inspect_causality) | Causal history of a memory address |
| [`debug_expand_hotspot`](04-advanced-analysis.md#debug_expand_hotspot) | Top-N hottest functions by call count/CPU |
| [`debug_get_saliency_scores`](04-advanced-analysis.md#debug_get_saliency_scores) | Saliency scores [0–1] per function |
| [`debug_diff`](04-advanced-analysis.md#debug_diff) | Compare program state between two event IDs |
| [`debug_analyze_memory`](04-advanced-analysis.md#debug_analyze_memory) | Analyze memory accesses in a time window |
| [`debug_get_variables`](04-advanced-analysis.md#debug_get_variables) | Variables in scope at an event |
| [`debug_get_memory`](04-advanced-analysis.md#debug_get_memory) | Raw memory value at a timestamp |
| [`debug_get_registers`](04-advanced-analysis.md#debug_get_registers) | CPU register values at an event |
| [`forensic_memory_audit`](04-advanced-analysis.md#forensic_memory_audit) | Complete audit trail of all writes to an address |
| [`performance_regression_audit`](04-advanced-analysis.md#performance_regression_audit) | Cross-session regression scoring |
| [`evaluate_expression`](04-advanced-analysis.md#evaluate_expression) | Evaluate arithmetic expression with local variables |

### Watchpoints / Subscriptions (3 tools)

| Tool | Description |
|------|-------------|
| [`subscribe_to_symbol`](05-watchpoints.md#subscribe_to_symbol) | Set hardware watchpoint on symbol or address |
| [`get_subscription_events`](05-watchpoints.md#get_subscription_events) | Poll watchpoint events |
| [`unsubscribe_from_symbol`](05-watchpoints.md#unsubscribe_from_symbol) | Remove a watchpoint |

---

## Document Map

| File | Contents |
|------|----------|
| [01-getting-started.md](01-getting-started.md) | Installation, `debug_run`, first trace |
| [02-session-lifecycle.md](02-session-lifecycle.md) | All 9 lifecycle tools |
| [03-query-inspection.md](03-query-inspection.md) | All 7 query/inspection tools |
| [04-advanced-analysis.md](04-advanced-analysis.md) | All 15 analysis tools |
| [05-watchpoints.md](05-watchpoints.md) | All 3 watchpoint tools |
| [06-multi-language.md](06-multi-language.md) | Language-specific setup (Python, JS, Java, Go, eBPF) |
| [07-prompt-examples.md](07-prompt-examples.md) | 20+ complete workflow examples |

---

## Quick Reference: Common Event Types

| Value | Meaning |
|-------|---------|
| `function_entry` | Function call start |
| `function_exit` | Function call return |
| `syscall_enter` | System call invocation |
| `syscall_exit` | System call return |
| `memory_read` | Memory read access |
| `memory_write` | Memory write access |
| `variable_write` | Variable mutation |
| `signal` | OS signal received |

---

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `CHRONOS_DB_PATH` | `~/.local/share/chronos/sessions.redb` | Path to the redb session store |
