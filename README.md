# Chronos — Time-Travel Debugging MCP Server

Chronos is an MCP (Model Context Protocol) server that enables AI agents to perform time-travel debugging on any language runtime.

## Features

- **Multi-language support**: Native (C/C++/Rust via ptrace), Python (sys.settrace), Java (JDWP), Go (Delve)
- **eBPF tracing**: Low-overhead kernel-level tracing via aya
- **Time-travel queries**: Reconstruct call stacks, detect races, trace variable origins
- **Persistent sessions**: BLAKE3 content-addressable storage with LZ4 compression
- **Golden trace comparison**: Detect regressions by diffing execution traces
- **32+ MCP tools**: Full debugging API for AI agents

## Quick Start

### Prerequisites
- Rust 1.77+
- For Python: `python3` in PATH
- For Java: `java` + `javac` in PATH
- For Go: `dlv` (Delve) in PATH

### Build

```bash
cargo build -p chronos-mcp --release
```

### Run as MCP server (stdio mode)

```bash
./target/release/chronos-mcp --stdio
```

### Configure with Claude Desktop / OpenCode

Add to your MCP config:
```json
{
  "mcpServers": {
    "chronos": {
      "command": "/path/to/chronos-mcp",
      "args": ["--stdio"]
    }
  }
}
```

## MCP Tools

### Capture
| Tool | Description |
|------|-------------|
| `debug_run` | Run a program under time-travel capture |
| `debug_attach` | Attach to a running process |

### Query
| Tool | Description |
|------|-------------|
| `query_events` | Filter and paginate trace events |
| `get_event` | Get a single event by ID |
| `reconstruct_call_stack` | Rebuild call stack at any point |
| `detect_races` | Find data races in the trace |
| `query_causality` | Trace memory access causality |
| `find_variable_origin` | Track where a variable was set |
| `get_execution_summary` | Overview of the execution |
| `expand_hotspot` | Top N hot functions |
| `get_saliency_scores` | CPU saliency per function |

### Persistence
| Tool | Description |
|------|-------------|
| `save_session` | Persist current session to disk |
| `load_session` | Restore a saved session |
| `list_sessions` | List all saved sessions |
| `delete_session` | Remove a session |
| `compare_sessions` | Diff two sessions for regressions |

## Architecture

```
chronos-mcp          ← MCP server (32+ tools)
├── chronos-native   ← ptrace adapter (C/C++/Rust)
├── chronos-ebpf     ← eBPF uprobes via aya
├── chronos-python   ← sys.settrace adapter
├── chronos-java     ← JDWP client adapter
├── chronos-go       ← Delve DAP adapter
├── chronos-capture  ← AdapterRegistry, CaptureSession
├── chronos-query    ← QueryEngine (time-travel queries)
├── chronos-store    ← redb + BLAKE3 CAS + TraceDiff
└── chronos-domain   ← Shared types (TraceEvent, EventData)
```

## Docker

```bash
docker build -t chronos-mcp .
docker run -v chronos-data:/data/chronos chronos-mcp
```

## Development

```bash
cargo test --workspace --lib -- --test-threads=1
cargo bench -p chronos-query
cargo bench -p chronos-store
```

## License
MIT
