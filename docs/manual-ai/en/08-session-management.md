# 08 — Session Management

Sessions are the atomic unit of Chronos. A session represents one complete execution capture — immutable, queryable indefinitely, shareable between agents and across time.

## The Session Lifecycle

```
debug_run() ──► [in-memory session] ──► query, analyze, compare
                      │
                      ├── save_session() ──► [persisted to disk]
                      │
                      ├── load_session() ◄── [load from disk]
                      │
                      ├── drop_session() ──► [removed from memory, stays on disk]
                      │
                      └── delete_session() ──► [removed from disk + memory]
```

## In-Memory vs Persisted

**In-memory only** — created by `debug_run` (synchronous mode). Available immediately. Lost on server restart.

**Persisted** — explicitly saved with `save_session` or `auto_save: true` in `debug_run`. Survives server restart. Loadable with `load_session`.

## save_session

Persists an in-memory session to the persistent store. Use this after `debug_run` completes if you want the trace to survive server restarts or be shareable.

**Parameters:**
- `session_id` (string, required) — the session to persist
- `language` (string, required) — language hint: `"python"`, `"rust"`, `"java"`, `"go"`, `"javascript"`, `"native"`
- `target` (string, required) — program path or name

**Parallel-safe:** Yes — read-only operation on the session

**Example call:**
```json
{
  "tool": "save_session",
  "params": {
    "session_id": "sess_a1b2c3",
    "language": "rust",
    "target": "/usr/bin/my-service"
  }
}
```

**Response:**
```json
{
  "session_id": "sess_a1b2c3",
  "auto_saved": false,
  "events_stored": 142857,
  "unique_hashes": 8934
}
```

**When to use:**
- After a successful `debug_run` if you need the trace to persist
- Before shutting down the server
- As part of a CI/CD pipeline after capturing a baseline

---

## load_session

Loads a previously persisted session from the store into memory, making it queryable.

**Parameters:**
- `session_id` (string, required) — the session to load

**Parallel-safe:** Yes — read-only operation on the store

**Example call:**
```json
{
  "tool": "load_session",
  "params": {
    "session_id": "sess_baseline_v2"
  }
}
```

**Response:**
```json
{
  "session_id": "sess_baseline_v2",
  "status": "loaded",
  "language": "rust",
  "target": "/usr/bin/my-service",
  "event_count": 142857,
  "duration_ms": 3421
}
```

**When to use:**
- Loading a saved baseline for regression comparison
- Loading a production trace for post-incident analysis
- Sharing sessions between agents (one captures, another analyzes later)

---

## list_sessions

Lists all persisted sessions in the store.

**Parameters:** None required

**Example call:**
```json
{
  "tool": "list_sessions",
  "params": {}
}
```

**Response:**
```json
{
  "sessions": [
    {
      "session_id": "sess_baseline_v2",
      "language": "rust",
      "target": "/usr/bin/my-service",
      "created_at": "2026-04-20T10:30:00Z",
      "event_count": 142857,
      "duration_ms": 3421
    },
    {
      "session_id": "sess_prod_incident_0420",
      "language": "python",
      "target": "api-server.py",
      "created_at": "2026-04-20T14:22:00Z",
      "event_count": 89234,
      "duration_ms": 1204
    }
  ]
}
```

**When to use:**
- Before starting analysis, to find available baseline sessions
- In CI/CD, to check for existing golden traces
- To find sessions from a specific time range

---

## delete_session

Removes a session from both the persistent store and memory. This is a **destructive, irreversible** operation.

**Parameters:**
- `session_id` (string, required)

**Example call:**
```json
{
  "tool": "delete_session",
  "params": {
    "session_id": "sess_old_baseline"
  }
}
```

**Response:**
```json
{
  "session_id": "sess_old_baseline",
  "deleted": true,
  "message": "Session removed from store and memory"
}
```

**When to use:**
- Cleaning up old baseline sessions before creating new ones
- GDPR/data lifecycle management
- Removing failed test sessions from CI

**⚠️ Warning:** There is no soft-delete. Once deleted, the session cannot be recovered.

---

## drop_session

Removes a session from **memory only** without touching the persistent store. Idempotent — safe to call even if the session is already gone from memory.

**Parameters:**
- `session_id` (string, required)

**Parallel-safe:** Yes — pure memory cleanup, no effect on store

**Example call:**
```json
{
  "tool": "drop_session",
  "params": {
    "session_id": "sess_a1b2c3"
  }
}
```

**Response:**
```json
{
  "session_id": "sess_a1b2c3",
  "dropped": true
}
```

**When to use:**
- Freeing memory after analysis is complete
- Before loading a different session with the same ID
- Routine memory management in long-running AI agent workflows

**Key difference from `delete_session`:**
- `drop_session` → removes from memory only, data survives in store
- `delete_session` → removes from BOTH memory and store (permanent)

---

## compare_sessions

Performs a hash-based diff between two sessions. Returns which events are unique to each session and which are shared.

**Parameters:**
- `session_a` (string, required) — first session ID
- `session_b` (string, required) — second session ID

**Parallel-safe:** Yes — both sessions are read-only

**Example call:**
```json
{
  "tool": "compare_sessions",
  "params": {
    "session_a": "sess_baseline_v2",
    "session_b": "sess_current_pr"
  }
}
```

**Response:**
```json
{
  "session_a": "sess_baseline_v2",
  "session_b": "sess_current_pr",
  "events_in_a_only": 1247,
  "events_in_b_only": 3891,
  "events_in_both": 138966,
  "hash_mismatch_rate": 0.023,
  "significant_changes": [
    {
      "function": "process_request",
      "calls_in_a": 142,
      "calls_in_b": 287,
      "delta": "+102%"
    }
  ]
}
```

**When to use:**
- Comparing baseline vs current PR
- Comparing staging vs production
- Detecting unexpected behavioral changes between versions

---

## CI/CD Pattern: Baseline Regression Gate

The most powerful session management workflow for AI agents:

```
1. debug_run()     → capture baseline session → save_session("baseline_v1")
2. [code changes]
3. debug_run()     → capture current session
4. compare_sessions() → baseline vs current
5. performance_regression_audit() → detailed comparison
   If regression_score > threshold → FAIL CI
```

**Example JSON sequence:**
```json
[
  { "tool": "debug_run",        "params": { "program": "./service", "auto_save": true } },
  { "tool": "save_session",     "params": { "session_id": "${SESSION_1}", "language": "rust", "target": "./service" } },
  { "tool": "debug_run",        "params": { "program": "./service", "auto_save": true } },
  { "tool": "save_session",     "params": { "session_id": "${SESSION_2}", "language": "rust", "target": "./service" } },
  { "tool": "performance_regression_audit", "params": { "baseline_session_id": "${SESSION_1}", "target_session_id": "${SESSION_2}" } }
]
```

---

## Multi-Agent Pattern

Sessions enable sophisticated multi-agent workflows:

```
Agent A (CI/CD runner):
  debug_run() → save_session("build_${GIT_SHA}") → store

Agent B (on-call analysis):
  load_session("build_${GIT_SHA}") → analyze production-like trace

Agent C (compare):
  load_session("build_main") → compare_sessions("build_main", "build_${GIT_SHA}")
```

Sessions survive the agent that created them. Any agent can load and query any persisted session by ID.

---

## Auto-Save

Instead of manually calling `save_session`, pass `auto_save: true` to `debug_run`:

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/my-service",
    "auto_save": true,
    "program_language": "rust"
  }
}
```

The session is automatically persisted to the store after capture completes. No extra round-trip needed.

---

## Session Storage Location

By default, sessions are stored in:
```
~/.local/share/chronos/sessions.redb
```

Override with the `CHRONOS_DB_PATH` environment variable:

```bash
CHRONOS_DB_PATH=/var/lib/chronos/sessions.redb chronos-mcp
```

The store uses `redb`, an embedded key-value database — no external database server required.

---

## Summary Table

| Tool | Target | Persists | Idempotent | Parallel-safe |
|------|--------|----------|------------|---------------|
| `save_session` | In-memory → store | Yes | No (overwrites) | Yes |
| `load_session` | Store → memory | No | No (error if missing) | Yes |
| `list_sessions` | Store | No | Yes | Yes |
| `delete_session` | Both | Deletes | No (error if missing) | Yes |
| `drop_session` | Memory only | No | Yes | Yes |
| `compare_sessions` | Both | No | Yes | Yes |
