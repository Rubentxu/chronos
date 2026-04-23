# Chronos MCP — AI-Native Time-Travel Debugging Manual

## What Is Chronos?

Chronos is a time-travel debugging system exposed as MCP (Model Context Protocol) tools, designed from the ground up for AI agents — not humans. It captures a complete, frozen trace of program execution in a single shot, then allows unlimited parallel queries against that trace.

## The Core Paradigm

```
Traditional (human) debugging:
  set breakpoint → run → pause → inspect → step → repeat → repeat → repeat

AI-native (Chronos) debugging:
  debug_run() → ONE frozen session → query EVERYTHING in parallel → done
```

This difference is not cosmetic. It changes how you think about debugging entirely.

A human debugger works interactively because humans can only hold a few things in mind at once. An AI agent can issue dozens of analysis queries simultaneously and synthesize all results in one pass. Chronos is built for this model.

## The "One Capture, N Analyses" Pattern

```
                    ┌─────────────────┐
                    │   debug_run()   │
                    │  (one capture)  │
                    └────────┬────────┘
                             │ session_id
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
   get_execution_    debug_get_      list_threads()
      summary()    saliency_scores()
              │              │              │
              └──────────────┼──────────────┘
                             │ orientation done
              ┌──────────────┼──────────────┐
              │              │              │
              ▼              ▼              ▼
   debug_find_crash() debug_detect_  debug_expand_
                        races()       hotspot()
              │              │              │
              └──────────────┼──────────────┘
                             │ bulk analysis done
                    (drill into findings)
```

All queries at the same level run in parallel. The session is immutable — no query modifies it.

## Quick Start

### 1. Capture a trace

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/path/to/my-binary",
    "args": ["--config", "app.toml"],
    "trace_syscalls": true,
    "capture_registers": true
  }
}
```

Response includes `session_id` (e.g. `"sess_a1b2c3"`).

### 2. Run orientation tools IN PARALLEL

```json
[
  { "tool": "get_execution_summary",      "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_get_saliency_scores",  "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "list_threads",               "params": { "session_id": "sess_a1b2c3" } }
]
```

### 3. Run bulk analysis IN PARALLEL based on symptoms

```json
[
  { "tool": "debug_find_crash",    "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_detect_races",  "params": { "session_id": "sess_a1b2c3" } },
  { "tool": "debug_call_graph",    "params": { "session_id": "sess_a1b2c3" } }
]
```

### 4. Drill down into specific findings

Use `query_events`, `get_call_stack`, `debug_get_variables`, etc. — only after you know what to look for.

## Tool Layers at a Glance

| Layer | Tools | When |
|-------|-------|------|
| **Capture** | `debug_run`, `debug_attach` | Always first |
| **Orientation** | `get_execution_summary`, `debug_get_saliency_scores`, `list_threads` | Immediately after capture, always in parallel |
| **Bulk analysis** | `debug_find_crash`, `debug_detect_races`, `debug_expand_hotspot`, `performance_regression_audit`, `debug_call_graph` | After orientation, in parallel based on symptom |
| **Forensics** | `forensic_memory_audit`, `inspect_causality`, `debug_find_variable_origin` | After bulk identifies suspicious address/variable |
| **Drill-down** | `query_events`, `get_call_stack`, `evaluate_expression`, `debug_get_variables`, `state_diff`, `debug_diff`, `get_event` | After forensics/bulk narrows scope |
| **Raw access** | `debug_get_memory`, `debug_get_registers`, `debug_analyze_memory` | Rarely — only for hardware-level investigation |
| **Session mgmt** | `save_session`, `load_session`, `list_sessions`, `delete_session`, `drop_session`, `compare_sessions` | CI/CD, multi-agent, persistence |

## Supported Languages

| Language | Capture mechanism |
|----------|-------------------|
| Native / C / C++ / Rust | ptrace |
| Java | JDWP |
| Python | DAP / debugpy |
| JavaScript / Node.js | CDP (Chrome DevTools Protocol) |
| Go | Delve DAP |
| eBPF | aya uprobes |

## Manual Structure

- **[01-core-pattern.md](01-core-pattern.md)** — Deep explanation of the AI-native paradigm
- **[02-capture.md](02-capture.md)** — debug_run and debug_attach in full detail
- **[03-orientation.md](03-orientation.md)** — Mandatory first-pass tools
- **[04-bulk-analysis.md](04-bulk-analysis.md)** — Bulk answer tools
- **[05-forensics.md](05-forensics.md)** — Causal investigation tools
- **[06-drill-down.md](06-drill-down.md)** — Targeted inspection tools
- **[07-raw-access.md](07-raw-access.md)** — Low-level memory and register access
- **[08-session-management.md](08-session-management.md)** — Persistence, CI/CD, multi-agent
- **[09-multi-language.md](09-multi-language.md)** — Language-specific setup and gotchas
- **[10-anti-patterns.md](10-anti-patterns.md)** — What NOT to do
- **[11-prompt-examples.md](11-prompt-examples.md)** — 20+ complete agent workflow examples
