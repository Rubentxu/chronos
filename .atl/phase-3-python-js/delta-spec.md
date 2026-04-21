# Phase 3: Python Adapter — Delta Specification

## Project: chronos
## Change: phase-3-python-js
## Status: Draft
## Created: 2026-04-21

---

## Overview

This document specifies the Python Adapter for Chronos, enabling time-travel debugging of Python scripts via `sys.settrace`. The adapter spawns a `python3 -u` subprocess with a bootstrap trace function, streams newline-delimited JSON events through stdout, and feeds them into the existing `QueryEngine` pipeline as `TraceEvent`s with a new `PythonFrame` variant.

**JavaScript/Node.js tracing is out of scope for this phase (deferred to phase-3b).**

---

## Module Architecture

```
crates/
  chronos-python/           ← NEW crate
    src/
      lib.rs                — public exports
      python_adapter.rs     — PythonAdapter struct, TraceAdapter impl
      event_translator.rs   — JSON → TraceEvent conversion
      bootstrap.rs          — Python settrace bootstrap code
      subprocess.rs         — python3 process lifecycle management
    Cargo.toml
```

**Key dependencies**: `chronos-domain` (types), `tokio` (async process/pipe I/O), `serde_json`.

---

## New EventData Variant: PythonFrame

### File: `crates/chronos-domain/src/trace/event.rs`

Add to `EventData`:

```rust
/// Python function call frame captured via sys.settrace.
PythonFrame {
    /// Function name (e.g. "foo" from "module.foo").
    name: String,
    /// Fully qualified name (e.g. "mymodule.MyClass.method").
    qualified_name: String,
    /// Path to the .py file.
    filename: String,
    /// Line number in the source file.
    lineno: u32,
    /// Local variables captured at this frame.
    locals: Vec<VariableInfo>,
    /// True if this is a generator frame.
    is_generator: bool,
}
```

**Serde**: `#[derive(Serialize, Deserialize)]` must cover all fields. JSON shape from Python:

```json
{
  "event": "call",
  "name": "foo",
  "qualified_name": "mymodule.foo",
  "file": "/path/to/script.py",
  "line": 10,
  "locals": { "x": "42", "name": "Alice" },
  "is_generator": false
}
```

`is_generator` is `false` for MVP (generator support is deferred). Locals are a JSON object mapping variable name → string-representation value.

---

## REQ-PY-01: Python Process Spawning

**Requirement**: Given a `CaptureConfig` targeting a `.py` file, when `debug_run` is called, Chronos must spawn `python3 -u` with a settrace bootstrap and capture stdout events.

### Scenario: Successful Python script capture

**Given** a `CaptureConfig` created from path `"script.py"` with `language = Some(Language::Python)`
**And** `capture_variables = true`
**When** `PythonAdapter::start_capture(config)` is called
**Then** Chronos spawns `python3 -u -c "<bootstrap>"` where `<bootstrap>` contains the settrace hook
**And** bootstrap code runs the target `script.py` via `exec(compile(read('script.py'), 'script.py', 'exec'))`
**And** stdout of the subprocess is captured line-by-line as UTF-8
**And** each line is parsed as JSON and forwarded as a `TraceEvent`
**And** the `CaptureSession` has `pid = 0` (placeholder; Python has no real pid tracked)

### Scenario: Python not available

**Given** `python3` is not on `PATH`
**When** `PythonAdapter::is_available()` is called
**Then** it returns `false`

### Scenario: Non-.py target language mismatch

**Given** a `CaptureConfig` with `language = Some(Language::Rust)` and target `"main.rs"`
**When** `AdapterRegistry::get(Language::Python)` is called
**Then** it returns `None` (the Rust adapter does not claim Python)

---

## REQ-PY-02: sys.settrace Event Capture

**Requirement**: The trace function must capture `call`, `return`, and `exception` events with sufficient context to reconstruct call stacks.

### Scenario: Function call event

**Given** the Python trace function is active
**When** a Python function `foo` is entered at `script.py:10`
**Then** a JSON line is printed to stdout:
```json
{"event": "call", "name": "foo", "qualified_name": "__main__.foo", "file": "script.py", "line": 10, "locals": {}, "is_generator": false}
```

### Scenario: Function return event

**Given** the Python trace function is active
**When** a Python function `foo` returns at `script.py:15`
**Then** a JSON line is printed to stdout:
```json
{"event": "return", "name": "foo", "qualified_name": "__main__.foo", "file": "script.py", "line": 15, "locals": {}, "is_generator": false}
```

### Scenario: Exception thrown event

**Given** the Python trace function is active
**When** an exception is raised at `script.py:20`
**Then** a JSON line is printed to stdout:
```json
{"event": "exception", "name": "ValueError", "qualified_name": "", "file": "script.py", "line": 20, "locals": {}, "is_generator": false}
```

Note: `name` for exception events carries the exception type (e.g. `"ValueError"`). `locals` for return/exception events is empty `{}` (no local capture on exit for MVP).

### Scenario: Bootstrap code

**Given** the bootstrap code is injected into `python3 -u -c`
**When** Python starts
**Then** `sys.settrace` is installed before any user code runs
**And** the trace function prints JSON events for `call`, `return`, `exception` only
**And** `line` events are NOT emitted (too high frequency for MVP)

---

## REQ-PY-03: Local Variable Capture

**Requirement**: At each `call` event, local variables from `frame.f_locals` must be captured, serialized, and transmitted.

### Scenario: Simple scalar types

**Given** a `call` event for function `process(n=42, name="Alice")` at `script.py:5`
**When** the trace function captures `frame.f_locals`
**Then** the emitted JSON includes `"locals": {"n": "42", "name": "Alice"}`
**And** `VariableInfo::name` = the variable name
**And** `VariableInfo::value` = the string representation
**And** `VariableInfo::type_name` = `"int"`, `"str"`, `"float"`, `"bool"`, or `"NoneType"` as appropriate
**And** `VariableInfo::address` = `0` (no real address in Python)
**And** `VariableInfo::scope` = `VariableScope::Local`

### Scenario: Complex types (list, dict, object)

**Given** a `call` event where a local variable has type `list`, `dict`, or custom class
**When** the trace function captures `frame.f_locals`
**Then** `repr(var)` is called to produce a string
**And** if the `repr` string exceeds 256 characters, it is truncated to 256 chars with `…` suffix
**And** `VariableInfo::type_name` = `"list"`, `"dict"`, or the class name

### Scenario: capture_variables disabled

**Given** `CaptureConfig::capture_variables = false`
**When** events are emitted
**Then** `"locals"` in the JSON is an empty object `{}`
**And** no variable strings are captured (performance optimization)

---

## REQ-PY-04: TraceAdapter Integration

**Requirement**: `PythonAdapter` must implement `TraceAdapter` and integrate with `AdapterRegistry`.

### Scenario: PythonAdapter registration

**Given** `chronos-python` is part of the Cargo workspace
**When** `AdapterRegistry::new()` is called
**Then** it is empty (registration happens in a separate `bootstrap` fn or at startup)
**And** a `PythonAdapter` can be registered via `registry.register(arc_dyn_adapter)`

### Scenario: Adapter retrieval by language

**Given** a `PythonAdapter` is registered for `Language::Python`
**When** `AdapterRegistry::get(Language::Python)` is called
**Then** it returns `Some(Arc<PythonAdapter>)`

### Scenario: Adapter availability check

**Given** `PythonAdapter::new()` is constructed
**When** `is_available()` is called
**Then** it checks `python3` is on PATH via `std::process::Command::new("python3").arg("--version")`
**And** returns `true` if the command succeeds, `false` otherwise

### Scenario: Adapter name

**Given** `PythonAdapter::new()` is constructed
**When** `name()` is called
**Then** it returns `"python"`

### Scenario: Adapter language identity

**Given** `PythonAdapter` implements `TraceAdapter`
**When** `get_language()` is called
**Then** it returns `Language::Python`

### File: `crates/chronos-capture/src/factory.rs`

Add registration of `PythonAdapter` in the default registry setup (or document that callers must register it explicitly if the capture crate does not link `chronos-python`).

---

## REQ-PY-05: Event Pipe Protocol

**Requirement**: Python stdout carries newline-delimited JSON (NDJSON). Rust deserializes and converts to `TraceEvent`.

### Scenario: Valid JSON line deserialization

**Given** a Python stdout line: `{"event": "call", "name": "foo", "qualified_name": "main.foo", "file": "script.py", "line": 3, "locals": {}, "is_generator": false}\n`
**When** `EventTranslator::translate_line(line)` is called
**Then** it returns `Ok(TraceEvent)` with:
- `event_type = EventType::FunctionEntry`
- `location.file = Some("script.py")`
- `location.line = Some(3)`
- `location.function = Some("foo")`
- `data = EventData::PythonFrame { name: "foo", qualified_name: "main.foo", filename: "script.py", lineno: 3, locals: vec![], is_generator: false }`

### Scenario: Return event translation

**Given** a Python stdout line: `{"event": "return", "name": "foo", "qualified_name": "main.foo", "file": "script.py", "line": 7, "locals": {}, "is_generator": false}\n`
**When** `EventTranslator::translate_line(line)` is called
**Then** it returns `Ok(TraceEvent)` with `event_type = EventType::FunctionExit`

### Scenario: Exception event translation

**Given** a Python stdout line: `{"event": "exception", "name": "RuntimeError", "qualified_name": "", "file": "script.py", "line": 12, "locals": {}, "is_generator": false}\n`
**When** `EventTranslator::translate_line(line)` is called
**Then** it returns `Ok(TraceEvent)` with `event_type = EventType::ExceptionThrown`
**And** `data = EventData::Exception { type_name: "RuntimeError", message: "" }`

### Scenario: Malformed JSON line

**Given** a Python stdout line that is not valid JSON
**When** `EventTranslator::translate_line(line)` is called
**Then** it returns `Err(TraceError::ParseError(...))` (non-fatal; adapter logs and continues)

### Scenario: Unknown event type

**Given** a Python stdout line with `"event": "line"` (unsupported)
**When** `EventTranslator::translate_line(line)` is called
**Then** it returns `Err(TraceError::ParseError("unsupported event type: line"))`

### Scenario: Timestamp assignment

**Given** `EventTranslator::translate_line` processes a line
**When** it creates a `TraceEvent`
**Then** `timestamp_ns` is assigned from a monotonically increasing counter starting at 0
**And** `event_id` is assigned from a monotonically increasing counter starting at 1
**And** `thread_id = 0` (Python scripts are single-threaded for MVP)

---

## REQ-PY-06: Graceful Termination

**Requirement**: Python subprocess lifecycle must be handled cleanly in all exit paths.

### Scenario: Python exits normally

**Given** the Python subprocess has finished executing
**When** `PythonAdapter::drain_events()` returns all remaining buffered events
**And** the subprocess exits with code 0
**Then** `CaptureSession::finalize()` is called
**And** `SessionState` transitions to `Finalized`

### Scenario: Python exits with error

**Given** the Python subprocess exits with a non-zero exit code
**When** `PythonAdapter::stop_capture()` is called or the session is dropped
**Then** `CaptureSession::error()` is called
**And** `SessionState` transitions to `Error`
**And** stderr output (if captured) is stored in the session for error reporting

### Scenario: Rust adapter dropped

**Given** `PythonAdapter` holds an active `Child` process
**When** `PythonAdapter` is dropped (e.g., `debug_run` cancelled)
**Then** `SIGTERM` is sent to the Python subprocess
**And** stdout pipe is drained before process handle is dropped
**And** any remaining events are processed

### Scenario: Broken pipe on stdout

**Given** the Python subprocess crashes before closing stdout
**When** a `BrokenPipe` or `IOError` occurs on the stdout reader
**Then** the adapter enters drain mode
**And** any already-read events are processed
**And** `SessionState` transitions to `Error`

---

## REQ-PY-07: MCP Tool Compatibility

**Requirement**: Existing MCP tools must operate unchanged on Python traces.

### Scenario: query_events on Python trace

**Given** a `CaptureSession` with `language = Language::Python`
**When** `query_events(session_id, filter)` is called via MCP
**Then** `QueryEngine` returns `PythonFrame` events matching the filter
**And** `EventData` payload is `PythonFrame { ... }`

### Scenario: get_call_stack on Python trace

**Given** a `TraceEvent` with `event_type = FunctionEntry` and `data = PythonFrame { ... }`
**When** `reconstruct_call_stack(event_id)` is called
**Then** it traverses `TraceEvent`s with matching `thread_id`
**And** returns frames in oldest-to-newest order
**And** each frame includes `PythonFrame` data (name, file, line)

### Scenario: get_backtrace on Python trace

**Given** a `TraceEvent` at a specific `event_id`
**When** `get_backtrace(event_id, max_depth)` is called
**Then** it returns the call stack using `PythonFrame::qualified_name` for frame labels
**And** respects `max_depth`

### Scenario: inspect_causality on Python trace

**Given** a `PythonFrame` event at `event_id = N`
**When** `inspect_causality(event_id)` is called
**Then** the causality index returns all events caused by or causing `N`
**And** `PythonFrame` events participate in the causality graph normally

### Scenario: debug_run accepts .py files

**Given** a Python script at path `"my_script.py"`
**When** `debug_run("my_script.py", config)` is called
**Then** `Language::from_path("my_script.py")` returns `Language::Python`
**And** `AdapterRegistry::get(Language::Python)` returns `PythonAdapter`
**And** capture proceeds as per REQ-PY-01

---

## File Layout

### `crates/chronos-python/Cargo.toml`

```toml
[package]
name = "chronos-python"
version.workspace = true
edition.workspace = true
rust-version.workspace = true

[dependencies]
chronos-domain = { workspace = true }
tokio = { workspace = true, features = ["process", "io-util", "sync"] }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
tracing = { workspace = true }
```

### `crates/chronos-python/src/lib.rs`

Public exports:
```rust
pub mod python_adapter;
pub mod event_translator;
pub mod bootstrap;
pub mod subprocess;

pub use python_adapter::PythonAdapter;
```

### `crates/chronos-python/src/python_adapter.rs`

`PythonAdapter` struct:
- `subprocess: Mutex<Option<Child>>`
- `event_buffer: Mutex<Vec<TraceEvent>>`
- `event_counter: AtomicU64`
- `timestamp_counter: AtomicU64`
- `shutdown_tx: Mutex<Option<oneshot::Sender<()>>>`

Implements `TraceAdapter`:
- `is_available()` — checks `python3 --version`
- `name()` — `"python"`
- `start_capture(config: CaptureConfig) -> Result<CaptureSession, TraceError>`
  - Spawns `python3 -u -c "<bootstrap>"` with `CaptureConfig::target` as the script path embedded in bootstrap
  - Sets up stdout reader task (tokio `AsyncRead`)
  - Starts background task to parse JSON lines → `TraceEvent`s
- `drain_events()` — returns buffered events, clears buffer
- `stop_capture(session: &CaptureSession)` — sends SIGTERM, drains pipe

### `crates/chronos-python/src/event_translator.rs`

`EventTranslator` struct:
- `translate_line(line: &str) -> Result<TraceEvent, TraceError>`
- Internal `PyEvent` struct for JSON deserialization
- Converts `PyEvent` fields to `TraceEvent` with `PythonFrame` data

### `crates/chronos-python/src/bootstrap.rs`

`get_bootstrap(script_path: &str, capture_locals: bool) -> String`:
- Returns Python source code that:
  1. Sets up `sys.settrace` with a trace function
  2. The trace function prints NDJSON to stdout for `call`, `return`, `exception` events
  3. Locals are captured via `frame.f_locals` (respects `capture_locals`)
  4. Runs the target script via `exec(compile(read(script_path), script_path, exec'))`
- `MAX_LOCALS_LEN: usize = 256` constant for truncation

### `crates/chronos-python/src/subprocess.rs`

`PythonProcess` struct:
- `spawn(script_path: &str, config: &CaptureConfig) -> Result<(Child, oneshot::Receiver<()>), TraceError>`
- `terminate(child: &mut Child) -> Result<(), TraceError>`

---

## Cargo Workspace Changes

### `Cargo.toml`

Add `"crates/chronos-python"` to `members` array.

---

## Backward Compatibility

- `EventData::PythonFrame` is added as a new variant — existing match arms in `QueryEngine` that do not cover `PythonFrame` will produce a compiler warning (not error) due to `#[non_exhaustive]` not being used (explicit handling should be added in `QueryEngine`).
- `Language::Python` already exists in the enum.
- `AdapterRegistry` behavior is unchanged for existing languages.

---

## Test Plan

1. **Unit tests** in each `chronos-python` module
2. **Integration test**: Run a sample `.py` script through `debug_run`, verify `query_events` returns `PythonFrame` entries with correct file/line/name
3. **Error path test**: Run a `.py` script that throws an uncaught exception, verify `ExceptionThrown` event and `SessionState::Error`
4. **Graceful termination test**: Spawn a long-running Python script, cancel mid-execution, verify clean shutdown

---

## Open Questions / Deferred

- **Generator frames**: `is_generator` is always `false` in MVP. Future work may track generator state.
- **Line events**: Not captured in MVP due to high frequency. Feature flag possible in future.
- **Async frames**: Not captured in MVP. Python async support requires `sys.settrace` + `sys.getasyncgenhooks` or similar.
- **Variable mutation tracking**: Not in MVP. Would require separate `VariableWrite` events.
- **Attaching to running Python process**: Not supported (subprocess spawn only for MVP).
