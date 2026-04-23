# 10 — Anti-Patterns

These are the most common mistakes when using Chronos with AI agents. Each anti-pattern shows the wrong approach, the right approach, and why it matters.

---

## Anti-Pattern 1: query_events Without Filters

**❌ Wrong:**
```json
{ "tool": "query_events", "params": { "session_id": "sess_abc123" } }
```
Calling `query_events` without any filters returns the first 100 events — which may be nothing useful. A typical program generates millions of events. The agent gets noise, wastes tokens, and may miss the signal.

**✅ Right:**
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry", "function_exit"],
    "function_pattern": "process_*",
    "timestamp_start": 5000000000,
    "timestamp_end": 6000000000,
    "limit": 50
  }
}
```
Always apply filters. Start with orientation tools (`get_execution_summary`, `debug_get_saliency_scores`) to narrow scope before querying raw events.

**Why it matters:** Without filters, the agent receives an arbitrary slice of execution — not necessarily where the bug is. Filters turn `query_events` from a noisy dump into a targeted probe.

---

## Anti-Pattern 2: Sequential Calls When Parallel Is Possible

**❌ Wrong:**
```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "sess_abc123" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_abc123" } },
  { "tool": "list_threads", "params": { "session_id": "sess_abc123" } },
  { "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } }
]
```
Sequential JSON payload. The agent waits for each response before sending the next. Four round-trips of latency.

**✅ Right:**
Send all four in parallel in a single batch:
```json
{
  "tool_batch": [
    { "tool": "get_execution_summary", "params": { "session_id": "sess_abc123" } },
    { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_abc123" } },
    { "tool": "list_threads", "params": { "session_id": "sess_abc123" } },
    { "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } }
  ]
}
```
One round-trip. All results available simultaneously. The agent synthesizes everything at once.

**Why it matters:** Round-trip latency compounds. Four sequential calls at 100ms each = 400ms total. Four parallel calls = 100ms total. For complex analysis with 10+ tools, the difference between 2 seconds and 200ms.

---

## Anti-Pattern 3: Using get_event in a Loop

**❌ Wrong:**
```python
# Pseudo-code: AI agent loops over event IDs
for event_id in suspicious_event_ids:
    result = call_tool("get_event", session_id=session_id, event_id=event_id)
    analyze(result)
```
Fetching one event at a time. If there are 100 suspicious events, that's 100 round-trips.

**✅ Right:**
Use `query_events` with a time range or pattern filter to get all relevant events at once:
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry"],
    "function_pattern": "suspicious_*",
    "timestamp_start": 5000000000,
    "timestamp_end": 6000000000,
    "limit": 100
  }
}
```
One call. Up to 100 events returned with full context. The agent processes the batch.

**Why it matters:** Loop-round-trips are the most expensive pattern. A 100-iteration loop at 50ms round-trip = 5 seconds. One filtered `query_events` = 50ms.

---

## Anti-Pattern 4: Discarding Sessions Without save_session

**❌ Wrong:**
```json
{ "tool": "debug_run", "params": { "program": "./service" } }
// ... analysis ... then agent moves on
// Session lost when server restarts
```

**✅ Right:**
```json
{ "tool": "debug_run", "params": { "program": "./service", "auto_save": true } }
// or explicitly:
{ "tool": "save_session", "params": { "session_id": "sess_abc123", "language": "rust", "target": "./service" } }
```
Persist the session. Either use `auto_save: true` in `debug_run` or call `save_session` afterward.

**Why it matters:** An in-memory session is lost on server restart. If the trace is valuable — a production incident, a baseline for CI, a rare bug — persist it. The cost is negligible (redb is a fast embedded store). The value is enormous: a persisted session can be loaded, compared, and shared indefinitely.

---

## Anti-Pattern 5: Calling Drill-Down Tools Before Orientation

**❌ Wrong:**
```json
[
  { "tool": "get_call_stack", "params": { "session_id": "sess_abc123", "event_id": 12345 } },
  { "tool": "debug_get_variables", "params": { "session_id": "sess_abc123", "event_id": 12345 } },
  { "tool": "inspect_causality", "params": { "session_id": "sess_abc123", "address": 140734193800032 } }
]
```
The agent is calling deep inspection tools on arbitrary event IDs and addresses it hasn't validated as relevant.

**✅ Right:**
Always start with orientation:
```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "sess_abc123" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_abc123", "limit": 10 } },
  { "tool": "list_threads", "params": { "session_id": "sess_abc123" } }
]
```
// Then bulk analysis:
```json
[
  { "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } },
  { "tool": "debug_detect_races", "params": { "session_id": "sess_abc123" } }
]
```
// THEN drill down on confirmed findings

**Why it matters:** Without orientation, the agent has no basis for choosing which event IDs and addresses to inspect. It's guessing. Orientation tools give the agent a map — hot functions, crash location, thread list — to target drill-down precisely.

---

## Anti-Pattern 6: Using state_diff / debug_diff Without Knowing What to Compare

**❌ Wrong:**
```json
{ "tool": "state_diff", "params": { "session_id": "sess_abc123", "timestamp_a": 1000000000, "timestamp_b": 2000000000 } }
```
The agent picked two random timestamps. The diff might show a meaningless change (e.g., a stack pointer increment) and miss the actual bug.

**✅ Right:**
Use `state_diff` only after `debug_find_crash` or `query_events` narrows down the relevant window:
```json
// First: find the crash point
{ "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } }
// Response: crash at event_id=45123, timestamp=5892341002

// Then: diff the state just before the crash
{ "tool": "debug_diff", "params": { "session_id": "sess_abc123", "event_id_a": 45120, "event_id_b": 45123 } }
```

**Why it matters:** `state_diff` and `debug_diff` are precision tools. Using them on unvalidated windows produces noise and wastes context. They belong at the end of an investigation chain, not the beginning.

---

## Anti-Pattern 7: Using background Mode for Polling

**❌ Wrong:**
```json
{ "tool": "debug_run", "params": { "program": "./long-running-service", "background": true } }
// Then polling:
while (true) {
    status = call_tool("get_session_status", session_id=session_id)
    if (status == "finalized") break
    sleep(1)
}
```
This is interactive debugging in disguise. The agent is polling, waiting for the capture to finish.

**✅ Right:**
Use synchronous mode — it waits for completion automatically:
```json
{ "tool": "debug_run", "params": { "program": "./long-running-service" } }
// Response arrives when capture is complete
```
The server blocks until the capture finishes. The agent gets the result directly.

**When background mode IS appropriate:** Long-running services where you want to start capture and do other work, but only if the agent has genuinely other work to do. Not for polling.

**Why it matters:** Polling loops are antithetical to the AI-native model. The agent should issue a command and wait for the result. Synchronous `debug_run` is one round-trip. Polling is N round-trips until a condition is met.

---

## Anti-Pattern 8: Using debug_attach When debug_run Would Work

**❌ Wrong:**
```json
{ "tool": "debug_attach", "params": { "pid": 12345 } }
// Agent attaches to a running process
// Problem: limited event window, process may be in an inconsistent state
```

**✅ Right:**
```json
{ "tool": "debug_run", "params": { "program": "./my-service", "args": ["--config", "prod.toml"] } }
```
Use `debug_run` to capture a complete execution from the start. Full trace, full context, all events from timestamp 0.

**When debug_attach IS appropriate:** Inspecting a process that's already running and cannot be restarted. Production debugging where you can't restart the service. Live process investigation.

**Why it matters:** `debug_attach` captures from the moment of attachment onward — no entry events for functions already on the stack, no context about what happened before attachment. For most cases, `debug_run` gives a complete trace.

---

## Anti-Pattern 9: Ignoring Program Language When It Matters

**❌ Wrong:**
```json
{ "tool": "debug_run", "params": { "program": "my_script.py" } }
// Omits program_language
// Python auto-detected but DAP connection params missing
```

**✅ Right:**
For Python and JavaScript, always specify the language and debug connection params:
```json
{ "tool": "debug_run", "params": {
    "program": "my_script.py",
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true
  }
}
```

**Why it matters:** Without `debug_port`, Chronos doesn't know where to connect for Python/JavaScript. The capture will return a "pending" status but won't produce a queryable session. The agent waits indefinitely for results that never come.

---

## Quick Reference: Anti-Pattern → Right Pattern

| Anti-pattern | Right pattern |
|---|---|
| `query_events` with no filters | Always filter by event_types, function_pattern, or time range |
| Sequential tool calls | Batch all parallel-safe calls into one round-trip |
| `get_event` in a loop | `query_events` with filters to get N events at once |
| Session not persisted | Use `auto_save: true` or call `save_session` |
| Drill-down before orientation | Always run orientation tools first |
| `state_diff` with random timestamps | Only after `debug_find_crash` or `query_events` narrows scope |
| background + polling | Use synchronous `debug_run` — it blocks until complete |
| `debug_attach` for new processes | Use `debug_run` for complete traces |
| Python/JS without `debug_port` | Always include `debug_port` + `wait_for_connection` for interpreted languages |
