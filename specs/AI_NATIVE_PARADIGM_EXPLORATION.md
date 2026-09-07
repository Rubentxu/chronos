# Exploration: AI-Native Documentation Paradigm for Chronos MCP

## Context

Chronos MCP has 35 tools for time-travel debugging. The current documentation draft treats it like traditional debugging (breakpoints, stop-the-world, interactive inspection). The user wants to rethink this completely for **AI agent usage**, where the paradigm is radically different from human debugging.

---

## 1. AI Agent Mental Model for Chronos

### How a Human Thinks About Debugging

```
HUMAN DEBUGGING PARADIGM:
─────────────────────────────────────────────────────────────
1. Set breakpoint at suspected location
2. Run program → stops at breakpoint
3. Inspect current variable values
4. Decide: step into / step over / continue
5. Repeat until bug found
─────────────────────────────────────────────────────────────

Characteristics:
• Sequential, interactive workflow
• One decision point at a time
• Human in the loop for every action
• "What is the state RIGHT NOW?"
• Breakpoints are the primary tool
• Time only moves forward (or via step-back)
```

### How an AI Agent Thinks About Debugging

```
AI AGENT DEBUGGING PARADIGM:
─────────────────────────────────────────────────────────────
1. Execute full program with tracing → get session_id
2. Ask ALL questions about the execution IN PARALLEL
3. Receive complete context immediately
4. No pausing, no breakpoints, no sequential stepping
─────────────────────────────────────────────────────────────

Characteristics:
• Fire-and-forget execution capture
• Parallel analysis of complete trace
• "Tell me EVERYTHING about this execution"
• Session is a frozen, queryable execution history
• Multiple simultaneous queries on same session
• "What value did x have at EVERY point in time?"
```

### The Core Insight: Bulk Trace Analysis

**Bulk trace analysis** means:
- Execute once → capture everything → analyze everything

Not: execute → pause → inspect → step → pause → inspect → step...

**Analogy**: Taking a CT scan vs. exploratory surgery
- **Exploratory surgery** (human debugging): Make an incision, look around, decide next cut, repeat
- **CT scan** (bulk trace): Capture complete 3D image, then examine any slice instantly

### What "One Session, Multiple Analyses" Means in Practice

```python
# Human debugging: sequential, one thing at a time
debugger.set_breakpoint("process_data")
debugger.run()
variable_value = debugger.read_variable("x")  # inspect one thing
debugger.step()
variable_value = debugger.read_variable("y")  # inspect next thing
# ... repeating

# AI agent debugging: parallel, everything at once
session_id = debug_run("./myprogram")  # One capture

# Now ask 10 questions simultaneously on the SAME session
questions = [
    get_execution_summary(session_id),           # What happened overall?
    debug_find_crash(session_id),                # Where did it crash?
    debug_detect_races(session_id),             # Any races?
    debug_expand_hotspot(session_id, top_n=5),  # What was hot?
    debug_call_graph(session_id, max_depth=10), # What's the call structure?
    query_events(session_id, event_types=["exception"]),
    query_events(session_id, event_types=["syscall_enter"]),
    debug_get_saliency_scores(session_id),      # What consumed CPU?
    debug_find_variable_origin(session_id, "error_count"),
    list_threads(session_id),                   # What threads existed?
]

# All 10 can execute in PARALLEL because they all query the SAME frozen session
results = await asyncio.gather(*questions)
```

---

## 2. Tool Groupings for AI Workflows

### 2.1 By Workflow Phase

#### **"Fire Once" Tools (Setup/Teardown)**
These are called ONCE per debugging session:

| Tool | Purpose | Call Pattern |
|------|---------|-------------|
| `debug_run` | Capture full execution | One time, get session_id |
| `debug_attach` | Attach to running process | One time alternative |
| `debug_stop` | Stop ongoing capture | Only if background capture |
| `save_session` | Persist session for later | Optional, after capture |
| `load_session` | Reload saved session | Optional, before analysis |
| `delete_session` | Clean up storage | When done |
| `drop_session` | Remove from memory | When done |

#### **"Analyze in Parallel" Tools (All on Same Session)**
These can ALL be called simultaneously after `debug_run`:

| Tool | What It Answers |
|------|----------------|
| `get_execution_summary` | "What happened overall?" |
| `debug_find_crash` | "Where did it crash?" |
| `debug_detect_races` | "Any data races?" |
| `debug_expand_hotspot` | "What was CPU-intensive?" |
| `debug_get_saliency_scores` | "What matters most?" |
| `debug_call_graph` | "What's the call structure?" |
| `list_threads` | "What threads existed?" |
| `query_events` (filtered) | "Show me specific events" |
| `debug_find_variable_origin` | "Where did this variable change?" |
| `inspect_causality` | "What wrote to this memory?" |

### 2.2 By Question Type

#### **"What Happened" Questions** (Descriptive)
These tools answer: "What occurred in this execution?"

| Tool | Answer |
|------|--------|
| `get_execution_summary` | Executive summary |
| `list_threads` | All threads that ran |
| `query_events` | Raw events filtered by type/time |
| `debug_call_graph` | Functions called, how often |
| `debug_expand_hotspot` | Most-called functions |

#### **"Why Did It Happen" Questions** (Causal)
These tools answer: "What caused this behavior?"

| Tool | Answer |
|------|--------|
| `debug_find_crash` | Root cause of crash |
| `debug_find_variable_origin` | Lineage of variable changes |
| `inspect_causality` | What wrote to a memory address |
| `debug_detect_races` | Conflicting writes |
| `get_call_stack` | Call path to a point |
| `debug_diff` | What changed between two points |

#### **"What If" Questions** (Exploratory)
These tools enable forensic investigation without stopping:

| Tool | Purpose |
|------|---------|
| `debug_get_memory` | "What was at this address?" |
| `debug_get_variables` | "What variables existed here?" |
| `debug_get_registers` | "What were CPU registers?" |
| `evaluate_expression` | "What is x + y * 2?" |
| `forensic_memory_audit` | "Full audit trail for address?" |
| `debug_analyze_memory` | "Memory accesses in range?" |

### 2.3 By Analysis Depth (Lazy Loading Levels)

**Level 0 - Orientation** (FIRST calls):
```json
{
  "get_execution_summary": "High-level picture",
  "list_threads": "What threads existed?",
  "debug_get_saliency_scores": "What dominated CPU?"
}
```

**Level 1 - Hotspot Zoom** (After summary):
```json
{
  "debug_expand_hotspot": "Drill into hot functions",
  "debug_call_graph": "See call relationships"
}
```

**Level 2 - Deep Dive** (Forensic):
```json
{
  "debug_find_variable_origin": "Variable lineage",
  "inspect_causality": "Memory address history",
  "forensic_memory_audit": "Full memory write audit"
}
```

**Level 3 - Microscopic** (When you need raw data):
```json
{
  "debug_get_memory": "Raw memory at address",
  "debug_get_registers": "CPU register state",
  "get_backtrace": "Full stack trace"
}
```

---

## 3. Key AI Agent Workflows

### Workflow 1: Crash Investigation
**Scenario**: Program crashed, need to find root cause

```python
# Step 1: Capture the crash
session_id = debug_run("./program_that_crashed")

# Step 2: Get orientation in parallel
summary, threads = await asyncio.gather(
    get_execution_summary(session_id),
    list_threads(session_id)
)

# Step 3: Find the crash point
crash_result = await debug_find_crash(session_id)
# Returns: signal type, event_id, call stack at crash

# Step 4: Examine what led to crash (parallel)
crash_event_id = crash_result["event_id"]
context_analysis = await asyncio.gather(
    get_call_stack(session_id, event_id=crash_event_id),
    query_events(session_id, 
        event_types=["exception"],
        limit=50),
    debug_find_variable_origin(session_id, "error_count"),
)

# Step 5: Full forensic audit on suspicious addresses
audit = await forensic_memory_audit(session_id, 
    address=0xDEADBEEF)
```

**Why this works**: No need to reproduce the crash. The trace contains everything.

---

### Workflow 2: Performance Regression Detection
**Scenario**: Function X is now 50% slower. Find why.

```python
# Capture baseline (good) and current (slow)
baseline_id = debug_run("./program", env={"VERSION": "v1.0.0"})
current_id = debug_run("./program", env={"VERSION": "v2.0.0"})

# Compare them
comparison = await performance_regression_audit(
    baseline_session_id=baseline_id,
    target_session_id=current_id,
    top_n=20
)
# Returns: functions with >10% cycle increase, severity, call count changes

# If specific function identified, zoom in
hotspot_detail = await debug_expand_hotspot(current_id, top_n=10)
call_graph = await debug_call_graph(current_id, max_depth=5)

# Compare specific function between versions
fn_comparison = await asyncio.gather(
    query_events(baseline_id, function_pattern="slow_function"),
    query_events(current_id, function_pattern="slow_function")
)
```

---

### Workflow 3: Data Race Detection
**Scenario**: Concurrent program has intermittent bugs from race conditions

```python
session_id = debug_run("./concurrent_program")

# Detect races
races = await debug_detect_races(session_id, threshold_ns=100)
# Returns: all writes to same address within 100ns on different threads

# For each race, get full context
for race in races["races"]:
    # Get call stacks for both conflicting writes
    stack_a = await get_call_stack(session_id, event_id=race["write_a"]["event_id"])
    stack_b = await get_call_stack(session_id, event_id=race["write_b"]["event_id"])
    
    # See what variables were involved
    vars_a = await debug_get_variables(session_id, event_id=race["write_a"]["event_id"])
    vars_b = await debug_get_variables(session_id, event_id=race["write_b"]["event_id"])
```

**Key insight**: The trace contains ALL thread interleavings, so races that only happened once in a billion runs are captured.

---

### Workflow 4: Memory Corruption Forensics
**Scenario**: Heap corruption causing sporadic crashes

```python
session_id = debug_run("./program_with_corruption")

# Find the crash first
crash = await debug_find_crash(session_id)

# Audit suspicious memory addresses
for address in [0xDEADBEEF, 0xBADF00D, 0x8BADF00D]:
    audit = await forensic_memory_audit(session_id, address=address)
    # Shows ALL writes to this address with call stacks
    
# Find where corrupted value was written
causality = await inspect_causality(session_id, address=0xDEADBEEF)
# Shows lineage of mutations to that memory location

# Analyze memory region for patterns
memory_analysis = await debug_analyze_memory(
    session_id,
    start_address=0x10000,
    end_address=0x20000,
    start_ts=0,
    end_ts=crash["timestamp_ns"]
)
```

---

### Workflow 5: Multi-Language Service Investigation
**Scenario**: Python API calls Rust library, something goes wrong

```python
# Capture Python session
py_session_id = debug_run("python api_server.py")

# Get summary to understand the flow
summary = await get_execution_summary(py_session_id)

# Find Rust library calls
rust_calls = await query_events(
    py_session_id, 
    function_pattern="*rust*"
)

# Get call stack at Rust boundary
for event in rust_calls["events"][:5]:
    stack = await get_call_stack(py_session_id, event_id=event["event_id"])
    # Shows Python caller → Rust callee chain

# If Python has Python-level trace, can cross-reference
py_frames = await query_events(
    py_session_id,
    event_types=["function_entry"],
    function_pattern="*handle_request*"
)
```

---

### Workflow 6: Production Incident Replay
**Scenario**: Bug only happens in production with real data

```python
# Production trace captured (via background debug_run or attach)
# Agent got session_id from monitoring system

session_id = "prod-incident-2024-01-15-001"

# Load it for analysis
await load_session(session_id)

# Immediately start parallel analysis
initial_findings = await asyncio.gather(
    get_execution_summary(session_id),
    debug_find_crash(session_id),
    debug_detect_races(session_id, threshold_ns=1000),  # Wider threshold for prod
    debug_get_saliency_scores(session_id)
)

# Based on findings, drill deeper
if initial_findings[1]["crash_found"]:
    # Detailed crash analysis
    crash_detail = await asyncio.gather(
        get_backtrace(session_id, event_id=crash["event_id"]),
        debug_find_variable_origin(session_id, "last_error"),
        query_events(session_id, 
            timestamp_end=crash["timestamp_ns"],
            limit=100)
    )
```

---

### Workflow 7: System Call Investigation
**Scenario**: Program making mysterious network calls

```python
session_id = debug_run("./mysterious_program")

# Get all syscalls
syscalls = await query_events(
    session_id,
    event_types=["syscall_enter", "syscall_exit"],
    limit=1000
)

# Group by syscall type
from collections import Counter
syscall_counts = Counter(e["data"]["syscall"]["name"] for e in syscalls["events"])

# Find suspicious patterns
for syscall_name, count in syscall_counts.most_common(20):
    # Get details on each
    details = await query_events(
        session_id,
        event_types=["syscall_enter"],
        function_pattern=syscall_name
    )

# Look for file descriptor leaks
open_events = await query_events(session_id, function_pattern="*open*")
close_events = await query_events(session_id, function_pattern="*close*")
# Compare counts, look for leaks

# Track specific FDs
fd_4_events = await query_events(
    session_id,
    function_pattern="*",
    # Custom filter by address if tracing specific FD
)
```

---

### Workflow 8: Memory Leak Detection
**Scenario**: Program memory grows without bound

```python
session_id = debug_run("./leaky_program")

# Get memory allocation summary
summary = await get_execution_summary(session_id)
# Look for high memory_allocated_bytes

# Get memory-related events
memory_events = await query_events(
    session_id,
    event_types=["memory_alloc", "memory_free"],
    limit=5000
)

# Group allocations by function
from collections import defaultdict
alloc_by_func = defaultdict(list)
for event in memory_events["events"]:
    if event["event_type"] == "memory_alloc":
        alloc_by_func[event["function"]].append(event)

# Find functions that allocate but rarely free
leak_candidates = []
for func, events in alloc_by_func.items():
    alloc_count = len(events)
    # Estimate free count by searching for free events in same function
    free_count = len([e for e in memory_events["events"] 
                      if e["event_type"] == "memory_free" 
                      and e["function"] == func])
    if alloc_count > free_count * 2:  # Significantly more alloc than free
        leak_candidates.append((func, alloc_count, free_count))

# Forensic audit on top leak candidates
for func, alloc, free in sorted(leak_candidates, key=lambda x: x[1]-x[2], reverse=True)[:3]:
    print(f"Function {func}: {alloc} allocs, {free} frees")
```

---

### Workflow 9: Test Failure Investigation
**Scenario**: Integration test fails, need to understand why

```python
# Run test with capture
session_id = debug_run("pytest tests/integration/test_payment.py")

# Get summary
summary = await get_execution_summary(session_id)

# Find the failure point
exceptions = await query_events(
    session_id,
    event_types=["exception"],
    limit=50
)

# Get full context around exception
for exc in exceptions["events"]:
    stack = await get_call_stack(session_id, event_id=exc["event_id"])
    vars_at_crash = await debug_get_variables(session_id, event_id=exc["event_id"])
    
    # Print formatted output for AI to analyze
    print(f"Exception at {exc['location']}")
    print(f"Stack: {stack}")
    print(f"Variables: {vars_at_crash}")

# Compare with successful test if available
baseline_id = "test_payment_success_20240115"
if baseline_id exists:
    comparison = await compare_sessions(baseline_id, session_id)
    # Shows divergence point and what changed
```

---

### Workflow 10: Continuous Profiling
**Scenario**: Profile production workload continuously

```python
# Background capture of production
await debug_run(
    "./production_service",
    background=True,
    auto_save=True
)
# Returns session_id immediately

# Later, analyze the captured session
session_id = "background-capture-2024-01-15-143022"

# Periodic saliency analysis
scores = await debug_get_saliency_scores(session_id, limit=50)
# Returns functions ranked by CPU consumption

# Hotspot expansion
for fn in scores["scores"][:10]:
    hotspot = await debug_expand_hotspot(session_id, top_n=5)
    if hotspot["function"] == fn["function"]:
        print(f"Function: {fn['function']}")
        print(f"  Calls: {fn['call_count']}")
        print(f"  CPU: {fn['total_cycles']} cycles")

# Call graph for hot paths
graph = await debug_call_graph(session_id, max_depth=5)
# Shows who calls the hot functions
```

---

## 4. Anti-Patterns — What NOT to Do

### ❌ Don't Use Chronos Like a Traditional Debugger

**WRONG** (human debugging pattern):
```python
# DON'T: Step-by-step debugging
session_id = debug_run("./program")
breakpoint_id = set_breakpoint(session_id, "function_x")
continue(session_id)  # Wait for breakpoint
inspect_variables(session_id)  # Look around
step_into(session_id)  # Go deeper
step_over(session_id)  # Skip ahead
# ... repeating
```

**RIGHT** (AI-native pattern):
```python
# DO: Capture everything, then query everything in parallel
session_id = debug_run("./program")
summary, crash, races, hotspots = await asyncio.gather(
    get_execution_summary(session_id),
    debug_find_crash(session_id),
    debug_detect_races(session_id),
    debug_expand_hotspot(session_id, top_n=10)
)
```

### ❌ Don't Make Sequential Calls When Parallel Would Work

**WRONG**:
```python
# DON'T: Wait for each result before next call
summary = await get_execution_summary(session_id)  # Wait...
crash = await debug_find_crash(session_id)          # Wait again...
hotspots = await debug_expand_hotspot(session_id)  # Wait again...
# 3 sequential waits
```

**RIGHT**:
```python
# DO: All queries at once
summary, crash, hotspots = await asyncio.gather(
    get_execution_summary(session_id),
    debug_find_crash(session_id),
    debug_expand_hotspot(session_id)
)
# 1 wait for all results
```

### ❌ Don't Use Breakpoints to Navigate

**WRONG**:
```python
# DON'T: Set breakpoints to explore
set_breakpoint(session_id, "step1")
continue(session_id)  # Stop at step1
set_breakpoint(session_id, "step2")  # Move breakpoint
continue(session_id)  # Stop at step2
```

**RIGHT**:
```python
# DO: Query the complete trace
events = await query_events(session_id, 
    timestamp_start=0,
    timestamp_end=end_time,
    limit=10000)
# All events in one query
```

### ❌ Don't Think of Sessions as Temporary

**WRONG**:
```python
# DON'T: Session is only valid during debugging
session_id = debug_run("./program")
analyze(session_id)
# Session lost after program ends (if not saved)
```

**RIGHT**:
```python
# DO: Sessions persist, can be analyzed later
session_id = debug_run("./program", auto_save=True)
# Session saved automatically

# Can reload later
await load_session(session_id)
analyze(session_id)

# Or compare with another session
comparison = await compare_sessions(session_a, session_b)
```

### ❌ Don't Query Without Filters First

**WRONG**:
```python
# DON'T: Get all events then filter
all_events = await query_events(session_id, limit=1000000)  # Too much data
filtered = [e for e in all_events if e["type"] == "syscall"]
```

**RIGHT**:
```python
# DO: Let the query engine filter
syscalls = await query_events(
    session_id,
    event_types=["syscall_enter"],
    limit=1000
)
# Only syscall events returned
```

### ❌ Don't Ignore the Summary First

**WRONG**:
```python
# DON'T: Jump straight to detailed queries
variable_history = await debug_find_variable_origin(session_id, "x")
# Without knowing context first
```

**RIGHT**:
```python
# DO: Get orientation first
summary = await get_execution_summary(session_id)
# Then target detailed queries based on findings
if "crash" in summary["potential_issues"]:
    crash = await debug_find_crash(session_id)
```

---

## 5. The "Single Session, Multiple Analyses" Pattern — Core Insight

### The Fundamental Difference

```
TRADITIONAL DEBUGGING:
────────────────────────────────────────────────────────────
Human: "I'm at breakpoint at line 50. What is x?"
     → debugger returns x
Human: "Now let me step to line 51. What is x?"
     → debugger returns x (maybe changed)
Human: "Now I need to know about function_y. 
        Let me set breakpoint there and run again."
     → CANNOT ask about different execution point
       without RE-RUNNING the program
────────────────────────────────────────────────────────────

AI AGENT DEBUGGING WITH CHRONOS:
────────────────────────────────────────────────────────────
Agent: "Capture full execution"
     → session_id returned

Agent: "Tell me about line 50, function x, every memory write, 
        crashes, races, and call graph"
     → ALL OF THESE QUERIES execute on the SAME session_id
     → The execution is "frozen" as a queryable dataset
     → Can ask about ANY point in execution at ANY time
────────────────────────────────────────────────────────────
```

### Session as a Time-Travel Database

```python
# The session is a complete execution capture
session_id = debug_run("./my_program")

# It's queryable by TIME
events_at_1s = await query_events(session_id, 
    timestamp_start=1_000_000_000,
    timestamp_end=2_000_000_000)

# By ADDRESS/MEMORY
writes_to_0xDEAD = await query_events(session_id,
    address=0xDEAD)

# By FUNCTION
all_process_data_calls = await query_events(session_id,
    function_pattern="process_data*")

# By EVENT TYPE
all_syscalls = await query_events(session_id,
    event_types=["syscall_enter", "syscall_exit"])

# By THREAD
thread_5_events = await query_events(session_id,
    thread_id=5)

# ALL AT THE SAME TIME (parallel)
all_queries = await asyncio.gather(
    query_events(session_id, timestamp_start=1_000_000, timestamp_end=2_000_000),
    query_events(session_id, address=0xDEAD),
    query_events(session_id, function_pattern="*critical*"),
    query_events(session_id, event_types=["exception"])
)
```

### The Implication: One Capture, Infinite Questions

```python
# Human debugging requires RE-RUNNING for each question
# "What was x at line 50?" → run to line 50
# "What about line 100?" → run again to line 100
# "What called function_y?" → set breakpoint, run again

# AI debugging with Chronos:
session_id = debug_run("./program")  # ONE capture

# Now ask INFINITE questions on that one capture
questions = [
    "What was x at line 50?",
    "What about line 100?",
    "What called function_y?",
    "Were there any races?",
    "What was the call stack at crash?",
    "How many times was malloc called?",
    "What threads existed?",
    "What syscalls happened?",
    # ... ad infinitum
]

# All answered by querying the SAME session
results = [await ask_question(session_id, q) for q in questions]

# Or in parallel:
results = await asyncio.gather(*[
    ask_question(session_id, q) for q in questions
])
```

---

## 6. Recommended First-Pass Sequence for AI Agents

### Standard First-Pass: Always Call These First

**Why**: These give orientation before deep diving into specifics.

```python
async def chronos_first_pass(session_id: str) -> dict:
    """
    Standard first-pass analysis for any new session.
    Returns orientation data before detailed investigation.
    """
    # ALL of these can run in PARALLEL
    summary, threads, saliency, crash, races = await asyncio.gather(
        get_execution_summary(session_id),
        list_threads(session_id),
        debug_get_saliency_scores(session_id, limit=20),
        debug_find_crash(session_id),
        debug_detect_races(session_id, threshold_ns=1000)
    )
    
    return {
        "session_id": session_id,
        "orientation": {
            "execution_summary": summary,
            "threads": threads,
            "saliency_scores": saliency,
            "crash": crash,
            "races": races
        }
    }
```

### Decision Tree After First-Pass

```
┌─────────────────────────────────────────────────────────────────┐
│                    FIRST-PASS RESULTS                            │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
              ┌───────────────────────────────────┐
              │      crash["crash_found"]?        │
              └───────────────────────────────────┘
                     │                    │
                    YES                   NO
                     │                    │
                     ▼                    ▼
        ┌────────────────────┐    ┌─────────────────────────────────┐
        │  debug_find_crash  │    │  saliency_scores tells us:    │
        │  get_call_stack   │    └─────────────────────────────────┘
        │  query exceptions  │              │              │
        └────────────────────┘            HIGH            LOW
                                         │              │
                                         ▼              ▼
                            ┌──────────────────┐   ┌──────────────────┐
                            │ debug_expand_    │   │ Summary looks   │
                            │ hotspot          │   │ normal? Likely  │
                            │ debug_call_graph │   │ no obvious bug  │
                            └──────────────────┘   │ → Done or       │
                                                    │   investigate   │
                            RACE DETECTED?         │   further if    │
                            │                       │   specific Q     │
                            ▼                       └──────────────────┘
                 ┌────────────────────┐
                 │ For each race:     │
                 │ - get_call_stack   │
                 │ - debug_get_       │
                 │   variables        │
                 │ - inspect_        │
                 │   causality        │
                 └────────────────────┘
```

### Deep-Dive Sequences Based on Findings

**If Crash Found:**
```python
async def analyze_crash(session_id: str, crash_info: dict):
    crash_event_id = crash_info["event_id"]
    
    # Parallel: everything about the crash point
    crash_context = await asyncio.gather(
        get_call_stack(session_id, event_id=crash_event_id),
        get_backtrace(session_id, event_id=crash_event_id),
        debug_get_variables(session_id, event_id=crash_event_id),
        query_events(session_id, 
            timestamp_end=crash_info["timestamp_ns"],
            limit=100),  # Events leading up to crash
    )
    
    # Find what caused the crash
    for event in crash_context[3]["events"][-20:]:  # Last 20 events
        if event["event_type"] == "variable_write":
            origin = await debug_find_variable_origin(
                session_id, 
                event["data"]["variable"]["name"]
            )
            # Shows lineage of variable that might have caused crash
    
    return crash_context
```

**If Performance Issue Suspected:**
```python
async def analyze_performance(session_id: str):
    # Get top hot functions
    hotspots = await debug_expand_hotspot(session_id, top_n=10)
    
    # For each hot function, get details in parallel
    hot_functions = [h["function"] for h in hotspots["hotspot_functions"]]
    
    details = await asyncio.gather(*[
        query_events(session_id, function_pattern=fn, limit=100)
        for fn in hot_functions
    ])
    
    # Get call graphs for hottest functions
    call_graphs = await asyncio.gather(*[
        debug_call_graph(session_id, max_depth=5)
        for _ in hot_functions
    ])
    
    return {
        "hotspots": hotspots,
        "call_graphs": call_graphs,
        "details": details
    }
```

---

## 7. Session Persistence Model for AI Workflows

### Why Persistence Enables AI Workflows

```
┌─────────────────────────────────────────────────────────────────┐
│              SESSION PERSISTENCE MODEL                          │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   debug_run() ──► session_id ──► [In-Memory Query Engine]       │
│                           │                                     │
│                           │ save_session()                      │
│                           ▼                                     │
│                    [Persistent Store]                            │
│                           │                                     │
│            ┌──────────────┼──────────────┐                     │
│            │              │              │                     │
│            ▼              ▼              ▼                     │
│     [session-A]    [session-B]    [golden-session]             │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### Key Persistence Operations

```python
# 1. Auto-save on capture
session_id = await debug_run("./program", auto_save=True)
# Session persisted automatically after capture

# 2. Manual save
await save_session(session_id, language="rust", target="./program")

# 3. List all saved sessions
sessions = await list_sessions()
# Returns metadata for all persisted sessions

# 4. Load for analysis
await load_session("prod-incident-2024-01-15-001")

# 5. Compare sessions
comparison = await compare_sessions(
    "baseline-v1.0", 
    "current-v2.0"
)

# 6. Delete when done
await delete_session("old-session-2023")
```

### AI Workflows Enabled by Persistence

#### Workflow: Production vs Staging Comparison
```python
# Capture in staging
staging_id = await debug_run("./service", 
    env={"ENV": "staging"},
    auto_save=True)

# Capture in production  
prod_id = await debug_run("./service",
    env={"ENV": "production"},
    auto_save=True)

# Compare them (can be days later)
comparison = await compare_sessions(staging_id, prod_id)

# If divergence found, load and analyze
if comparison["similarity_pct"] < 95:
    await load_session(prod_id)
    # Deep dive into differences
    diff_events = await query_events(prod_id, 
        event_types=["exception"])
    # ... investigate
```

#### Workflow: Golden Trace Regression Testing
```python
# Register golden trace (known good behavior)
golden_id = await debug_run("./test --known-good")
await save_session(golden_id, language="rust", target="integration-test")

# Later, run test and compare
current_id = await debug_run("./test --current-code")
comparison = await compare_sessions(golden_id, current_id)

# Automated pass/fail
if comparison["similarity_pct"] < 90:
    print("REGRESSION DETECTED")
    # Generate report
    regression = await performance_regression_audit(
        baseline_session_id=golden_id,
        target_session_id=current_id
    )
```

#### Workflow: Sharing Traces Between AI Agents
```python
# Agent A: Investigates production issue
session_id = await debug_run("./prod-service")
await save_session(session_id, language="rust", target="prod-service")
# Shares session_id: "prod-incident-2024-01-15-143022"

# Agent B: Later continues investigation
await load_session("prod-incident-2024-01-15-143022")
# Continues where Agent A left off

# Agent B can also compare with Agent A's baseline
agent_a_baseline = "agent-a-baseline-run"
comparison = await compare_sessions(agent_a_baseline, "prod-incident-2024-01-15-143022")
```

#### Workflow: Historical Analysis
```python
# List all sessions from a time range
all_sessions = await list_sessions()

# Filter by date (sessions have created_at metadata)
recent_sessions = [
    s for s in all_sessions["sessions"]
    if s["created_at"] > "2024-01-01"
]

# Analyze trends across multiple sessions
for session in recent_sessions:
    await load_session(session["session_id"])
    summary = await get_execution_summary(session["session_id"])
    print(f"{session['session_id']}: {summary['total_events']} events")
```

---

## 8. Tool Quick Reference: AI-Native Categorization

### Capture Phase (One Time)
| Tool | When to Use |
|------|-------------|
| `debug_run` | Normal execution capture |
| `debug_attach` | Attach to running process |
| `debug_run(background=True)` | Long-running processes |
| `debug_stop` | Stop background capture |

### Orientation Phase (First Pass — Always These)
| Tool | What It Tells You |
|------|-------------------|
| `get_execution_summary` | High-level picture |
| `list_threads` | What threads existed |
| `debug_get_saliency_scores` | What dominated CPU |
| `debug_find_crash` | Where did it crash? |
| `debug_detect_races` | Any data races? |

### Hotspot Analysis Phase (After Orientation)
| Tool | When to Use |
|------|-------------|
| `debug_expand_hotspot` | Drill into hot functions |
| `debug_call_graph` | See call relationships |
| `debug_get_saliency_scores` | Ranked importance |

### Causal Analysis Phase (Why Did It Happen)
| Tool | What It Answers |
|------|-----------------|
| `debug_find_crash` | Root cause of crash |
| `debug_find_variable_origin` | Variable lineage |
| `inspect_causality` | Memory address history |
| `get_call_stack` | Path to point |
| `debug_diff` | What changed between points |

### Forensic Phase (Deep Dive)
| Tool | What It Shows |
|------|---------------|
| `forensic_memory_audit` | All writes to address |
| `debug_analyze_memory` | Memory accesses in range |
| `debug_get_memory` | Raw memory at address |
| `debug_get_registers` | CPU registers |
| `get_backtrace` | Full stack trace |

### Query Phase (Event Filtering)
| Tool | What It Returns |
|------|-----------------|
| `query_events` | Filtered events |
| `get_event` | Single event detail |
| `list_threads` | All threads |
| `evaluate_expression` | Arithmetic on variables |

### Persistence Phase (Session Management)
| Tool | Purpose |
|------|--------|
| `save_session` | Persist for later |
| `load_session` | Reload saved session |
| `list_sessions` | See all saved |
| `compare_sessions` | Compare two |
| `delete_session` | Clean up |
| `drop_session` | Remove from memory |
| `performance_regression_audit` | Compare performance |

---

## 9. Summary: Key Principles for AI-Native Documentation

### Core Principles

1. **One Capture, Infinite Queries**: `debug_run` captures everything. Then query any aspect without re-running.

2. **Parallel Over Sequential**: All analysis tools can run simultaneously on the same session.

3. **Orientation Before Deep Dive**: Always start with `get_execution_summary`, `list_threads`, `debug_get_saliency_scores`.

4. **Sessions Persist**: Saved sessions can be analyzed later, compared, and shared between agents.

5. **Bulk Over Interactive**: Don't step through execution. Query the complete trace.

6. **Lazy Loading**: Summary → Hotspot → Detail. Don't start at level 3.

### Mental Model Shift

```
HUMAN DEBUGGING          →    AI AGENT DEBUGGING
─────────────────────────────────────────────────────
Interactive               →    Bulk capture
Step-by-step              →    Parallel queries
One breakpoint            →    Complete trace
"What is x now?"          →    "What was x at EVERY point?"
Re-run to see different   →    Query frozen session
points
Sequential decisions       →    All analysis at once
Temporary session         →    Persistent, queryable
                              session
```

### Documentation Structure Recommendation

For each tool, document:
1. **What question does it answer?** (not "what does it do")
2. **When to call it** (orientation vs deep dive)
3. **What other tools can run in parallel with it**
4. **Example AI workflow showing it in context**

---

*This exploration document provides the foundation for redesigning Chronos MCP documentation with the AI-native paradigm as the central theme.*
