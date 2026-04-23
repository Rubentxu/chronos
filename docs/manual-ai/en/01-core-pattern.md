# 01 — The AI-Native Debugging Paradigm

## Why Traditional Debugging Fails AI Agents

Traditional debuggers (gdb, lldb, pdb, delve) are designed around a human interaction loop:

1. Set a breakpoint at a suspected location
2. Run the program — it pauses at the breakpoint
3. Inspect variables, stack, registers
4. Step forward one instruction or one line
5. Re-inspect
6. Repeat hundreds of times

This model is optimized for a human who:
- Can only hold a few variables in mind
- Cannot issue 30 queries simultaneously
- Needs to narrow scope manually through hypothesis → test cycles
- Has unlimited time to iterate

An AI agent is the opposite:
- Can synthesize hundreds of data points at once
- Can issue all queries simultaneously in one LLM turn
- Must not waste tokens on interactive loops
- Has a limited context window — every round trip costs

**Chronos eliminates the loop entirely.** There is no "pause and inspect." There is "capture once, query everything."

## The Frozen Session Model

When `debug_run` executes, Chronos:

1. Launches the target program under a tracer (ptrace, JDWP, DAP, CDP, Delve, or eBPF depending on language)
2. Records every function entry/exit, system call, memory access, register state, and thread switch
3. Lets the program run to completion (or timeout)
4. Builds query indices over the captured trace
5. Returns a `session_id`

The session is **immutable and frozen**. Every query against it is read-only. You can issue 50 queries against the same session and get consistent, reproducible answers. Nothing you query changes the session.

This enables true parallel analysis.

## Parallel Analysis — The Core AI Advantage

Because the session is frozen and read-only, all analysis tools are **parallel-safe**. They can be called simultaneously with no ordering dependency.

### Orientation level (always parallel)

```
get_execution_summary()   ──┐
debug_get_saliency_scores() ─┼──► all three simultaneously
list_threads()            ──┘
```

These three tools answer:
- "How many events? Any obvious issues?" (`get_execution_summary`)
- "Which functions consumed the most CPU?" (`debug_get_saliency_scores`)
- "How many threads? What are their IDs?" (`list_threads`)

### Bulk analysis level (parallel, based on symptoms)

After orientation reveals the symptom type, fire the relevant bulk tools simultaneously:

**Crash investigation:**
```
debug_find_crash()     ──┐
debug_call_graph()     ──┼──► all simultaneously
debug_expand_hotspot() ──┘
```

**Concurrency investigation:**
```
debug_detect_races()   ──┐
list_threads()         ──┼──► all simultaneously
debug_call_graph()     ──┘
```

**Performance investigation:**
```
debug_expand_hotspot()         ──┐
debug_get_saliency_scores()    ──┼──► all simultaneously
performance_regression_audit() ──┘
```

### Forensic level (depends on bulk results)

Once bulk analysis identifies a suspicious memory address or variable name, forensic tools investigate it:

```
forensic_memory_audit(address=0x7fff1234) ──┐
inspect_causality(address=0x7fff1234)     ──┼──► all simultaneously
debug_find_variable_origin(var="count")   ──┘
```

### Drill-down level (depends on forensic results)

After forensics identifies a specific event_id or timestamp range, drill-down tools inspect that exact moment:

```
get_call_stack(event_id=4721)         ──┐
debug_get_variables(event_id=4721)    ──┼──► all simultaneously
debug_get_registers(event_id=4721)    ──┘
```

## The Five-Level Analysis Pyramid

```
                    ┌─────────────┐
                    │   CAPTURE   │  debug_run / debug_attach
                    └──────┬──────┘
                           │
                    ┌──────▼──────┐
                    │ ORIENTATION │  summary + saliency + threads
                    └──────┬──────┘  (always parallel, always first)
                           │
                    ┌──────▼──────┐
                    │    BULK     │  crash / races / hotspot / regression
                    └──────┬──────┘  (parallel, symptom-driven)
                           │
                    ┌──────▼──────┐
                    │  FORENSICS  │  memory audit / causality / variable origin
                    └──────┬──────┘  (parallel, address/variable-driven)
                           │
                    ┌──────▼──────┐
                    │ DRILL-DOWN  │  query_events / call_stack / variables
                    └──────┬──────┘  (parallel, event-driven)
                           │
                    ┌──────▼──────┐
                    │  RAW ACCESS │  memory / registers / memory_analyze
                    └─────────────┘  (rarely needed)
```

**Key rule: Never skip to a lower level without information from a higher level.**

Calling `get_event(event_id=42)` without knowing that event 42 is significant is wasted effort. Always let orientation and bulk analysis guide you to the relevant scope.

## Lazy Evaluation — Only Go Deeper When You Need To

Not every investigation needs all five levels.

| Symptom | Levels needed |
|---------|---------------|
| "Did it crash?" | Capture → Orientation → Bulk (`debug_find_crash`) |
| "Why is it slow?" | Capture → Orientation → Bulk (`debug_expand_hotspot`, `performance_regression_audit`) |
| "Is there a data race?" | Capture → Orientation → Bulk (`debug_detect_races`) |
| "How did variable X get corrupted?" | Capture → Orientation → Bulk → Forensics → Drill-down |
| "What happened at timestamp T?" | Capture → Orientation → Drill-down (if you already know T) |

For simple questions (crash location, race detection), you may never need forensics or drill-down. Stop as soon as you have an answer.

## Token Economy — Why This Matters

Every tool call returns data that consumes context window tokens. Discipline about level-skipping is critical:

- `get_execution_summary` returns ~500 tokens
- `debug_find_crash` returns ~200 tokens
- `query_events` without filters can return **millions of events** — context window overflow

Always use orientation results to constrain drill-down queries. Always use `limit` and `timestamp_start`/`timestamp_end` filters when calling `query_events`.

## Immutability and Reproducibility

Because the session is frozen:

1. You can re-query the same session hours later and get identical results
2. Multiple AI agents can query the same session simultaneously (multi-agent workflows)
3. Sessions can be saved to disk and loaded in future sessions
4. Sessions from CI runs can be compared against baseline sessions

This enables patterns impossible with interactive debuggers:
- "Compare this PR's trace against main branch baseline"
- "Load the trace from the production incident last Tuesday"
- "Have three agents simultaneously analyze different aspects of the same crash"

## Summary: The Mental Model

Think of a Chronos session as a **database** of everything that happened during program execution. `debug_run` is the ETL process that populates it. All other tools are SQL queries against it — read-only, parallel-safe, reproducible.

Your job as an AI agent is:
1. **Populate the database** (debug_run)
2. **Run a broad scan** (orientation tools)
3. **Run targeted aggregate queries** (bulk tools)
4. **Join and correlate** (forensic tools)
5. **Retrieve specific rows** (drill-down tools)
6. **Read raw bytes only if absolutely necessary** (raw access tools)
