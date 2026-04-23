# 05 — Forensic Tools

Forensic tools investigate the causal history of specific memory addresses and variables. They answer the question "how did this get into this state?" by tracing every read and write across the entire execution.

**When to use forensic tools:** After bulk analysis has identified a suspicious memory address (e.g., from `debug_find_crash`'s `crash_address`, or from `debug_detect_races`'s race addresses), or when you need to trace the origin of a corrupted value.

All three forensic tools are parallel-safe and can be called simultaneously.

---

## `forensic_memory_audit`

**One-line description:** Show every write that ever touched a specific memory address, in chronological order.

**When to call:** After `debug_find_crash` returns a `crash_address`, or after `debug_detect_races` returns a race address. This is the primary tool for buffer overflow and use-after-free investigation.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `address` | int | required | Memory address (decimal) to audit |
| `limit` | int | `100` | Maximum number of write entries to return |

### Example call

```json
{
  "tool": "forensic_memory_audit",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "address": 140734799995981,
    "limit": 50
  }
}
```

(Note: `0x7fff1a2b3c4d` = `140734799995981` decimal)

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "address": 140734799995981,
  "address_hex": "0x7fff1a2b3c4d",
  "total_writes": 8,
  "writes": [
    {
      "event_id": 1024,
      "timestamp_ns": 102400000,
      "thread_id": 1,
      "function": "alloc_record",
      "value": "0x0000000000000000",
      "size_bytes": 8,
      "context": "initial allocation"
    },
    {
      "event_id": 44201,
      "timestamp_ns": 1050000010,
      "thread_id": 1,
      "function": "parse_record",
      "value": "0x000000010a2b3c4d",
      "size_bytes": 8,
      "context": "pointer assignment"
    },
    {
      "event_id": 91204,
      "timestamp_ns": 2100000100,
      "thread_id": 2,
      "function": "worker_update_counter",
      "value": "0x0000000000000005",
      "size_bytes": 8,
      "context": "concurrent write"
    },
    {
      "event_id": 182938,
      "timestamp_ns": 4230980000,
      "thread_id": 1,
      "function": "parse_record",
      "value": "0xdeadbeefdeadbeef",
      "size_bytes": 8,
      "context": "write before crash"
    }
  ]
}
```

### What to extract

- **Write immediately before crash** → the likely corruption source
- **Writes from unexpected threads** → concurrent access without synchronization
- **Writes with suspicious values** (`0xdeadbeef`, `0xffffffff`, pointer-sized ints) → corruption patterns
- **Multiple threads writing** → cross-reference with `debug_detect_races`
- Use `event_id` from any write → pass to `get_call_stack` to see the full call chain at that write

---

## `inspect_causality`

**One-line description:** Show the complete causal history of a memory address — every read AND write, with the originating function, to reconstruct full data lineage.

**When to call:** After `forensic_memory_audit` identifies suspicious writes, or when you need to understand not just writes but also reads (e.g., use-after-free where a freed address is later read).

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `address` | int | required | Memory address (decimal) |
| `limit` | int | `100` | Maximum number of causal entries (reads + writes combined) |

### Example call

```json
{
  "tool": "inspect_causality",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "address": 140734799995981,
    "limit": 100
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "address": 140734799995981,
  "address_hex": "0x7fff1a2b3c4d",
  "total_accesses": 24,
  "accesses": [
    {
      "event_id": 1024,
      "timestamp_ns": 102400000,
      "thread_id": 1,
      "access_type": "write",
      "function": "alloc_record",
      "value": "0x0000000000000000"
    },
    {
      "event_id": 1025,
      "timestamp_ns": 102400100,
      "thread_id": 1,
      "access_type": "read",
      "function": "init_record",
      "value": "0x0000000000000000"
    },
    {
      "event_id": 44201,
      "timestamp_ns": 1050000010,
      "thread_id": 1,
      "access_type": "write",
      "function": "parse_record",
      "value": "0x000000010a2b3c4d"
    },
    {
      "event_id": 160042,
      "timestamp_ns": 3800000000,
      "thread_id": 1,
      "access_type": "write",
      "function": "free_record",
      "value": "0x0000000000000000",
      "note": "memory freed here"
    },
    {
      "event_id": 182935,
      "timestamp_ns": 4230978000,
      "thread_id": 1,
      "access_type": "read",
      "function": "parse_record",
      "value": "0x0000000000000000",
      "note": "read after free — use-after-free detected"
    }
  ]
}
```

### What to extract

- **Read after a `free_record` write** → use-after-free bug
- **Reads returning unexpected values** → trace backwards to find the write that set that value
- **Interleaved reads and writes from multiple threads** → confirms race condition
- The full chronological sequence reveals the exact lifecycle of the data

### Difference from `forensic_memory_audit`

| Tool | Returns | Use when |
|------|---------|----------|
| `forensic_memory_audit` | Writes only | Finding what corrupted a value |
| `inspect_causality` | Reads AND writes | Finding use-after-free, verifying full data lifecycle |

---

## `debug_find_variable_origin`

**One-line description:** Trace the complete mutation history of a named variable — every assignment from first write to last.

**When to call:** When you know the name of a variable that holds an unexpected value (e.g., "how did `count` become -1?", "when was `ptr` set to null?"). Requires that the variable name is available in debug symbols.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `variable_name` | string | required | Variable name (exact match, as it appears in debug symbols) |
| `limit` | int | `100` | Maximum number of mutations to return |

### Example call

```json
{
  "tool": "debug_find_variable_origin",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "variable_name": "shared_counter",
    "limit": 50
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "variable_name": "shared_counter",
  "total_mutations": 14,
  "mutations": [
    {
      "event_id": 502,
      "timestamp_ns": 50200000,
      "thread_id": 1,
      "function": "init_counters",
      "old_value": null,
      "new_value": "0",
      "source_file": "src/counters.rs",
      "source_line": 18
    },
    {
      "event_id": 44890,
      "timestamp_ns": 1060000000,
      "thread_id": 2,
      "function": "worker_update_counter",
      "old_value": "4",
      "new_value": "5",
      "source_file": "src/worker.rs",
      "source_line": 77
    },
    {
      "event_id": 44920,
      "timestamp_ns": 1060000100,
      "thread_id": 3,
      "function": "worker_update_counter",
      "old_value": "5",
      "new_value": "5",
      "source_file": "src/worker.rs",
      "source_line": 77,
      "note": "lost update — race condition"
    },
    {
      "event_id": 182910,
      "timestamp_ns": 4230900000,
      "thread_id": 1,
      "function": "finalize",
      "old_value": "11",
      "new_value": "-1",
      "source_file": "src/counters.rs",
      "source_line": 204,
      "note": "integer overflow"
    }
  ]
}
```

### What to extract

- **Lost update** → two threads read the same value, both increment, one write overwrites the other (classic non-atomic increment race)
- **Unexpected final value** → trace back to the mutation that set it; check `old_value` vs `new_value`
- **Write from unexpected thread** → confirms which thread is the source of corruption
- **Write from unexpected function** → indicates an architectural violation (something wrote to a variable it shouldn't own)
- `source_file` + `source_line` → direct navigation to the bug location in code

### Requirements

- Debug symbols must be present (compiled with `-g` for C/C++/Rust, or with source maps for JS, or not stripped for Go/Java)
- Variable name must be the exact symbol name as compiled — use the name as written in source code (not mangled)
- For Rust, use the unmangled name (e.g., `shared_counter`, not `mymodule::shared_counter` unless fully qualified)

---

## Forensic Tools Workflow Example

Given a crash at address `0x7fff1a2b3c4d` identified by `debug_find_crash`:

```json
[
  {
    "tool": "forensic_memory_audit",
    "params": {
      "session_id": "sess_a1b2c3d4",
      "address": 140734799995981
    }
  },
  {
    "tool": "inspect_causality",
    "params": {
      "session_id": "sess_a1b2c3d4",
      "address": 140734799995981
    }
  },
  {
    "tool": "debug_find_variable_origin",
    "params": {
      "session_id": "sess_a1b2c3d4",
      "variable_name": "record_ptr"
    }
  }
]
```

All three run simultaneously. `forensic_memory_audit` shows what wrote to the address. `inspect_causality` shows the full read/write lifecycle. `debug_find_variable_origin` traces the variable by name if the address corresponds to a known symbol.
