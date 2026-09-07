# tokio-console Setup for Chronos

[tokio-console](https://github.com/tokio-console/console) is a debugging tool that visualizes async task behavior in Rust applications. It provides real-time insight into task states, polling behavior, and resource contention.

## What is tokio-console?

tokio-console exposes internal Tokio runtime instrumentation via a dedicated subscriber. It allows you to:

- View all active async tasks and their current state (idle, running, blocked)
- Identify tasks that are being polled frequently without yielding
- Detect resource contention (tasks waiting on locks, channels, etc.)
- See task spawns and drops in real-time
- Debug slow operations by identifying where tasks spend time

## Installation

```bash
cargo install tokio-console
```

The `tokio-console` CLI connects to a running Chronos process that has tracing enabled.

## Building Chronos with tokio-console Support

Chronos is configured to always include the `tokio_unstable` cfg flag via `.cargo/config.toml`:

```bash
# Normal build (works as before)
cargo build

# Run the MCP server
cargo run --bin chronos-mcp

# In another terminal, connect tokio-console
tokio-console
```

By default, tokio-console connects to `http://127.0.0.1:6669`. The Chronos server must be running with the console subscriber enabled.

## Running Tests with Tracing

To capture tokio-console traces during test execution:

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo test
```

## What to Look For

### Task States

| State | Meaning |
|-------|---------|
| **Idle** | Task is waiting for I/O or a future to be ready |
| **Running** | Task is actively executing |
| **Blocked** | Task is waiting on a resource (lock, channel, timer) |

### Common Issues

1. **Tasks polling continuously**: A task that never yields indicates a hot loop or compute-heavy future that should be wrapped with `tokio::task::spawn_blocking`.

2. **Resource contention**: Tasks blocked on locks or channels indicate contention. Consider:
   - Reducing lock scope
   - Using more fine-grained locking
   - Batching operations

3. **Slow probe operations**: If probe operations are slow, check if they're blocking the async context. Look for:
   - Blocking calls inside async functions
   - Sequential I/O that could be parallelized
   - Large synchronous computations

4. **Session management**: Long-lived sessions can accumulate tasks. Watch for:
   - Tasks that should have been dropped but remain active
   - Memory growth over time from accumulated session state

## Chronos-Specific Use Cases

### Debugging Slow Probe Operations

Probe operations in Chronos can involve:
- LLM API calls (network I/O)
- File system operations
- Subprocess execution

If probes are slow, tokio-console helps identify whether:
- The delay is in async waiting (expected) or blocking the thread (problematic)
- Multiple probes are contending for resources
- The task is blocked waiting on a channel or lock

### Understanding Session Management

Each session in Chronos may spawn tasks for:
- Event processing
- LLM interaction
- File watching

Use tokio-console to:
- Verify tasks are cleaned up when sessions close
- Identify session-related resource leaks
- Understand task hierarchies during bulk operations

### Multi-Session Concurrency

When running multiple sessions in parallel, tokio-console shows:
- How tasks are distributed across tokio threads
- Whether sessions are competing for shared resources
- If any session is starving others

## Quick Debugging Workflow

1. Start the Chronos MCP server:
   ```bash
   cargo run --bin chronos-mcp
   ```

2. In another terminal, start tokio-console:
   ```bash
   tokio-console
   ```

3. Trigger the operation you want to debug (e.g., start a probe, create a session)

4. Watch the task visualization for:
   - New tasks appearing
   - Task state changes
   - Resources being waited on

5. Click on a task to see its details, including the task name and spawn location.

## Additional Resources

- [tokio-console GitHub](https://github.com/tokio-console/console)
- [Tokio tracing documentation](https://docs.rs/tracing/latest/tracing/)
- [Async debugging guide](https://tokio.rs/docs/async-programming/async-debugging/)
