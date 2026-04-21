# ⏳ Chronos — Time-Travel Debugging MCP Server

> **Transform program execution into a queryable temporal database for AI agents.**

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compatible-blue.svg)](https://modelcontextprotocol.io/)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-green.svg)](LICENSE)

Chronos is an MCP (Model Context Protocol) server that gives AI agents the ability to **debug programs post-mortem** by recording complete execution traces and exposing them as temporal queries. Instead of the traditional breakpoint → inspect → continue cycle, Chronos captures everything and lets the AI agent ask questions like:

- *"What was the call stack at the moment of the crash?"*
- *"Show me all syscalls that used file descriptor 3"*
- *"What functions were called between timestamp 2.1s and 2.5s?"*
- *"What changed in the register state between these two events?"*

---

## 🏗️ Architecture

Chronos is built as a **modular Rust workspace** with clean DDD + SOLID separation:

```
chronos/
├── crates/
│   ├── chronos-domain/     # Core domain types (TraceEvent, indices, queries)
│   ├── chronos-format/     # Binary trace file format (bincode + LZ4)
│   ├── chronos-capture/    # Capture pipeline with TraceAdapter trait
│   ├── chronos-native/     # Linux ptrace tracer, ELF symbol resolver, breakpoints
│   ├── chronos-index/      # In-memory shadow + temporal indices
│   ├── chronos-query/      # Query engine with temporal/shadow lookups
│   ├── chronos-mcp/        # MCP server exposing 10 tools to AI agents
│   └── chronos-e2e/        # End-to-end integration tests
├── tests/fixtures/         # C test programs for integration testing
└── specs/                  # Architecture documentation
```

### How It Works

```
┌──────────┐    ptrace     ┌──────────────┐    bincode    ┌──────────────┐
│  Target   │─────────────▶│   Capture    │─────────────▶│  Trace File  │
│  Program  │  (C/C++/Rust)│  Pipeline    │   + LZ4      │  (.chronos)  │
└──────────┘               └──────────────┘               └──────┬───────┘
                                                                  │ read
                                                                  ▼
┌──────────┐    MCP      ┌──────────────┐    query     ┌──────────────┐
│  AI Agent │◀───────────│  Chronos MCP │◀─────────────│  Query Engine │
│  (Claude) │  10 tools  │    Server    │              │  + Indices    │
└──────────┘             └──────────────┘              └──────────────┘
```

---

## 🛠️ MCP Tools

Chronos exposes **10 tools** through the Model Context Protocol:

| Tool | Description |
|------|-------------|
| `debug_run` | Launch a program under time-travel capture |
| `debug_attach` | Attach to a running process |
| `debug_stop` | Stop a capture session |
| `query_events` | Query events with filters (type, time, function, thread) |
| `get_event` | Get a single event by ID |
| `get_call_stack` | Reconstruct call stack at any point in time |
| `get_execution_summary` | Get statistics: event counts, top functions, issues detected |
| `state_diff` | Compare register state between two timestamps |
| `list_threads` | List all threads seen during execution |
| `get_backtrace` | Get backtrace at a specific event |

---

## 🚀 Quick Start

### Prerequisites

- **Rust** 1.75+ (`rustup`)
- **Linux** x86-64 (ptrace-based capture)
- **gcc** (for C/C++ test fixtures)

### Build

```bash
# Clone
git clone git@github.com:Rubentxu/chronos.git
cd chronos

# Build
cargo build --release

# Binary is at target/release/chronos-mcp
```

### Run Tests

```bash
# All tests (171 tests, 0 ignored)
cargo test --workspace

# Ptrace tests require single-threaded execution (fork safety)
cargo test --workspace -- --test-threads=1

# Integration tests only (compiles C fixtures with gcc)
cargo test --package chronos-e2e
```

---

## 🔌 Connect to AI Clients

### Claude Desktop

Add to your `claude_desktop_config.json`:

**Development** (runs from source):
```json
{
  "mcpServers": {
    "chronos": {
      "command": "cargo",
      "args": ["run", "--release", "--package", "chronos-mcp", "--bin", "chronos-mcp"],
      "cwd": "/path/to/chronos"
    }
  }
}
```

**Release** (pre-built binary):
```json
{
  "mcpServers": {
    "chronos": {
      "command": "/path/to/chronos-mcp",
      "env": { "RUST_LOG": "info" }
    }
  }
}
```

### Any MCP Client

Chronos uses **stdio transport** — it reads JSON-RPC from stdin and writes to stdout. Compatible with any MCP client.

---

## 💡 Example Session

Once connected to an AI agent (e.g., Claude):

```
Human: I have a crash in my C program at ./my_app. Can you debug it?

AI: I'll run your program under Chronos to capture the full execution trace.
    → calls debug_run(program="./my_app")

AI: The program crashed with SIGSEGV. Let me analyze the execution.
    → calls get_execution_summary(session_id="...")
    → calls get_call_stack(session_id="...", event_id=47)

AI: The crash occurred in function `process_buffer()` at the point where
    it calls `memcpy()`. The call stack was:
      main() → handle_request() → process_buffer() → memcpy()
    
    Looking at the state diff between the last successful operation and the crash:
    → calls state_diff(session_id="...", timestamp_a=..., timestamp_b=...)

AI: Register RDI (destination pointer) was 0x0 — a null pointer was passed
    to memcpy. The issue is in process_buffer() at line 42 where `buffer`
    is used without a null check after `malloc()` fails.
```

---

## 📊 Project Status

### Phase 1 MVP ✅ (v0.2.0 — Current)

- [x] Domain model (events, indices, queries)
- [x] Binary trace file format (bincode + LZ4 compression)
- [x] Capture pipeline with backpressure
- [x] Linux ptrace tracer (fork/exec/attach)
- [x] ELF symbol resolution (address → function name)
- [x] INT3 breakpoint manager — intercepts every ELF function entry
- [x] Syscall name table — 340+ x86_64 syscall names (write, mmap, exit…)
- [x] Signal delivery — SIGSEGV/SIGABRT forwarded cleanly, crashes terminate gracefully
- [x] In-memory shadow + temporal indices
- [x] Query engine with call stack reconstruction
- [x] MCP server with 10 tools — synchronous capture, no race conditions
- [x] Noisy event filtering — internal register snapshots excluded from query results
- [x] End-to-end integration tests (171 tests, 0 ignored)
- [x] Server binary + MCP config files

### Phase 2 (Planned)

- [ ] High-level language support (Python, JavaScript, Java)
- [ ] DWARF debug info for source-level resolution
- [ ] Variable capture (frame-pointer-relative)
- [ ] FlatBuffers for zero-copy large trace files

### Phase 3 (Planned)

- [ ] RocksDB persistent storage for large traces
- [ ] Trace file streaming / partial loading
- [ ] Multi-session management
- [ ] Expression evaluation in stopped context

### Phase 4 (Planned)

- [ ] eBPF-based capture (low overhead)
- [ ] Reverse debugging (step backwards)
- [ ] Distributed trace correlation
- [ ] Web UI for trace visualization

---

## 🧪 Test Coverage

| Crate | Tests | Description |
|-------|-------|-------------|
| chronos-domain | 50 | Core domain types, serialization, indices |
| chronos-format | 10 | Binary encode/decode, trace file roundtrip |
| chronos-capture | 8 | Adapter registry, pipeline, backpressure |
| chronos-native | 90 | Ptrace, ELF symbols, breakpoints, syscall table |
| chronos-index | 5 | Index builder, range queries, temporal chunks |
| chronos-query | 24 | Query engine, call stack, state diff |
| chronos-mcp | 9 | Server construction, tool params, error handling |
| chronos-e2e | 13 | Full pipeline: compile → capture → index → query |
| **Total** | **171** | All passing, 0 ignored |

---

## 📦 Tech Stack

| Component | Technology |
|-----------|-----------|
| Language | Rust 1.75+ |
| Async Runtime | Tokio |
| MCP SDK | rmcp 1.5 |
| Process Tracing | nix 0.29 (ptrace) |
| ELF Parsing | object 0.36 |
| Serialization | bincode 1.3 + serde |
| Compression | lz4_flex 0.11 |
| Logging | tracing + tracing-subscriber |

---

## 📄 License

Licensed under either of Apache License, Version 2.0 or MIT license at your option.

---

## 🙏 Acknowledgments

Chronos was designed with the vision that **AI agents deserve better debugging tools than humans have**. Instead of adapting human debuggers for AI, Chronos builds a new paradigm: complete execution capture with temporal queries, designed from the ground up for machine consumption.
