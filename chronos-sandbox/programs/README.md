# Test Program Fixtures

This directory contains multi-language target programs used for testing Chronos MCP sandbox capture capabilities across different runtimes and languages.

## Directory Structure

```
programs/
├── c/           # C programs using clone(), fork(), pthreads
├── rust/        # Rust programs with hotspots and data races
├── python/      # Python programs with nested calls and exceptions
├── go/          # Go programs with goroutines and race conditions
├── java/        # Java programs with threads (placeholder)
└── js/          # JavaScript programs with async/await (placeholder)
```

## Language Support Notes

### Rust Programs (`rust/`)

**compute_hotspot.rs**
- CPU-bound hotspot with nested loops (1M iterations)
- Purpose: Hotspot identification, CPU profiling, saliency scoring
- Events: Function calls (main → compute_hotspot → inner_loop)

**data_race.rs**
- Intentional data race with two threads incrementing shared counter
- Purpose: Race condition detection, thread sanitizer testing
- Events: Thread creation, unsynchronized writes, data race
- NOTE: Final value will be incorrect due to race (this IS the test case)

### Python Programs (`python/`)

**nested_calls.py**
- Nested function calls with depth of 5
- Purpose: Call stack reconstruction, deep call chain capture
- Events: Function entry/exit for each level (main → level1 → ... → level5)

**exception_chain.py**
- Chained exceptions with causal chain (error_a → error_b → error_c)
- Purpose: Exception event capture, causality chain reconstruction
- Events: Exception raised/caught at each level, full cause chain preserved

### Go Programs (`go/`)

**goroutine_chain.go**
- Chain of goroutines: main → spawner → worker → nested
- Purpose: Goroutine lifecycle testing, channel operation capture
- Events: Goroutine creation, channel send/receive, termination

**goroutine_race.go**
- Intentional race condition with 3 goroutines
- Purpose: Race detection, simultaneous write identification
- Events: Goroutine creation, unsynchronized writes, data race
- NOTE: Final value will be incorrect due to race (this IS the test case)

### Java Programs (`java/`) — Placeholder

**ThreadedSum.java**
- Status: Placeholder (may require JDK installation)
- Multi-threaded sum computation with 3 threads
- Purpose: Java thread lifecycle capture
- Events: Thread.start(), Thread.run(), Thread.join()

### JavaScript Programs (`js/`) — Placeholder

**async_chain.js**
- Status: Placeholder (may require Node.js installation)
- Async/await chain with 3 levels
- Purpose: Async function call capture, Promise event tracking
- Events: Async function calls, await points, Promise resolution

## Common Patterns

Each program follows a consistent structure:
1. **HEADER COMMENT**: Documents KNOWN_BEHAVIOR, expected events, and purpose
2. **IMPLEMENTATION**: Self-contained, compilable/runnable code
3. **EXIT**: Clean exit (0) unless testing crash/race conditions

## Compilation

- **C**: Compiled via `build.rs` using the `cc` crate
- **Rust**: Standard `rustc` compilation
- **Go**: `go build` (standard library only)
- **Python**: Interpreted (no compilation needed)
- **Java**: `javac` (requires JDK)
- **JavaScript**: Interpreted by Node.js

## Usage in Tests

These programs are used by integration tests to verify:
- Function call tracing across languages
- Exception/crash detection
- Thread/goroutine lifecycle tracking
- Data race detection capabilities
- Async operation capture
- Causality chain reconstruction
