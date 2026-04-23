# 07 — Raw Access Tools

Raw access tools provide direct, low-level inspection of memory contents and CPU register values at specific moments in the trace. They bypass the higher-level symbol resolution and variable interpretation of drill-down tools.

**When to use raw access tools:** Rarely. These tools are needed only when:
- Debug symbols are stripped or unavailable
- You need to inspect memory regions not covered by named variables (heap metadata, custom allocators, MMapped regions)
- You need raw register values for binary analysis or exploit research
- You're analyzing a language runtime internals (GC metadata, JVM heap, etc.)

If debug symbols are available, prefer `debug_get_variables` and `get_call_stack` — they are more readable and require less interpretation.

---

## `debug_get_memory`

**One-line description:** Read the raw byte contents of a memory address at a specific timestamp.

**When to call:** When you need to inspect memory that is not covered by named variables — e.g., heap chunk headers, custom allocator free lists, or memory-mapped regions. Requires knowing the address and timestamp.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `address` | int | required | Memory address (decimal) to read |
| `timestamp_ns` | int | required | Timestamp (nanoseconds) at which to read memory state |

### Example call

```json
{
  "tool": "debug_get_memory",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "address": 140734799995981,
    "timestamp_ns": 4230978000
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "address": 140734799995981,
  "address_hex": "0x7fff1a2b3c4d",
  "timestamp_ns": 4230978000,
  "bytes": "deadbeefdeadbeef0000000000000000",
  "bytes_hex_dump": [
    "0x7fff1a2b3c4d: de ad be ef de ad be ef  00 00 00 00 00 00 00 00",
    "0x7fff1a2b3c5d: 41 41 41 41 41 41 41 41  41 41 41 41 41 41 41 41"
  ],
  "size_bytes": 32
}
```

### Interpretation notes

- `0xdeadbeef` pattern → common use-after-free marker (allocated by some allocators)
- `0x41414141` (`AAAA`) → classic buffer overflow fill pattern
- All zeros → either valid zero-initialized memory or cleared-on-free
- Use this alongside `forensic_memory_audit` to confirm the write that set these bytes

---

## `debug_get_registers`

**One-line description:** Get the exact CPU register values at a specific event.

**When to call:** After `get_call_stack` or `debug_find_crash` identifies a specific event_id, when you need the raw register state (e.g., for exploit analysis, to verify calling convention compliance, or to check for register corruption).

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `event_id` | int | required | Event ID at which to snapshot registers |

### Example call

```json
{
  "tool": "debug_get_registers",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "event_id": 182940
  }
}
```

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "event_id": 182940,
  "timestamp_ns": 4230981234,
  "thread_id": 1,
  "architecture": "x86_64",
  "registers": {
    "rax": "0x0000000000000000",
    "rbx": "0x000000010a2b0000",
    "rcx": "0x00000000000001c8",
    "rdx": "0x0000000000000008",
    "rsi": "0x7fff1a2b3c4d",
    "rdi": "0x000000010a2b3c50",
    "rbp": "0x4141414141414141",
    "rsp": "0x7fffffffd3a0",
    "rip": "0x00007fff1a2b3c4d",
    "r8":  "0x0000000000000000",
    "r9":  "0x0000000000000000",
    "r10": "0x0000000000000000",
    "r11": "0x0000000000000246",
    "r12": "0x000000010a2b3c50",
    "r13": "0x0000000000000000",
    "r14": "0x0000000000000000",
    "r15": "0x0000000000000000",
    "rflags": "0x0000000000010246",
    "cs": "0x0033",
    "ss": "0x002b"
  }
}
```

### Interpretation notes

- `rip` (instruction pointer) pointing into the stack → return address overwrite (stack smashing)
- `rbp` = `0x4141414141414141` → base pointer corrupted with 'A' fill pattern
- `rsp` decreasing → stack growing (normal); suddenly large increase → stack pivot
- For x86_64 System V ABI: `rdi`, `rsi`, `rdx`, `rcx`, `r8`, `r9` are argument registers

---

## `debug_analyze_memory`

**One-line description:** Show all memory accesses (reads and writes) within an address range during a time window.

**When to call:** When investigating heap corruption, buffer overflows, or memory aliasing — when you suspect a region of memory is being accessed incorrectly, but you don't know the exact address. This tool sweeps a range.

**Parallel-safe?** Yes.

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `session_id` | string | required | Session ID |
| `start_address` | int | required | Start of memory address range (decimal) |
| `end_address` | int | required | End of memory address range (decimal) |
| `start_ts` | int | required | Start of time window (nanoseconds) |
| `end_ts` | int | required | End of time window (nanoseconds) |

### Example call

Scan the stack region (RSP ± 1KB) in the 2 milliseconds before the crash:

```json
{
  "tool": "debug_analyze_memory",
  "params": {
    "session_id": "sess_a1b2c3d4",
    "start_address": 140733193384352,
    "end_address": 140733193386400,
    "start_ts": 4228981234,
    "end_ts": 4230981234
  }
}
```

(Range: `0x7fffffffd3a0` to `0x7fffffffdba0`, i.e., RSP ± 1024 bytes)

### Example response

```json
{
  "session_id": "sess_a1b2c3d4",
  "address_range": {
    "start": "0x7fffffffd3a0",
    "end": "0x7fffffffdba0"
  },
  "time_range": {
    "start_ns": 4228981234,
    "end_ns": 4230981234
  },
  "total_accesses": 48,
  "accesses": [
    {
      "event_id": 182901,
      "address": "0x7fffffffd490",
      "timestamp_ns": 4229100000,
      "thread_id": 1,
      "access_type": "write",
      "value": "0x000000010a2b3c50",
      "size_bytes": 8,
      "function": "parse_record"
    },
    {
      "event_id": 182930,
      "address": "0x7fffffffd490",
      "timestamp_ns": 4230900000,
      "thread_id": 1,
      "access_type": "write",
      "value": "0xdeadbeefdeadbeef",
      "size_bytes": 8,
      "function": "parse_record",
      "note": "overwrote return address"
    }
  ],
  "suspicious_accesses": [
    {
      "event_id": 182930,
      "reason": "write to return address slot",
      "address": "0x7fffffffd490"
    }
  ]
}
```

### When to use vs `inspect_causality`

| Tool | Use when |
|------|----------|
| `inspect_causality` | You know the exact address, want full lifecycle |
| `debug_analyze_memory` | You know a region but not the exact address; sweeping for anomalies |
| `forensic_memory_audit` | You know the exact address, want writes only |

---

## Raw Access Decision Guide

```
Need raw bytes at an address?          → debug_get_memory
Need CPU register values?              → debug_get_registers
Suspicious about a memory region?      → debug_analyze_memory
Have named variable?                   → debug_get_variables (prefer this)
Have call stack?                       → get_call_stack (prefer this)
```

## Architecture Notes

- All raw access tools work only on native/ptrace-captured sessions
- For Java, Python, JS, and Go sessions, register-level data may be absent or translated through the runtime's representation
- `debug_analyze_memory` on large address ranges (> 10MB) with wide time windows can be slow and return large result sets — always bound both the address range and time window tightly
