# ADR-0010 — M4R.5 perturbation detected + fallback policy

**Status:** Accepted
**Date:** 2026-09-21
**Cycle:** M4-F1 / M4R.5 (perturbation fallback summary; closes M4-F0
Rust side end-to-end)
**Spike consolidation:** builds on ADR-0008 (M4R.1 XRay) and
ADR-0009 (M4R.2 USDT). No new measurements; this ADR is a
**synthesis and policy ADR**.
**chronos HEAD:** `022cf14d` (post M4R.2 SHA alignment).

## 1. Context

ROADMAP §M4-F1 §83 lists M4R.5 as "perturbación detectada + fallback".
The full text of §M4-F1 says: "Si una opción técnica se rechaza
razonadamente, **conservar el objetivo de evidencia y justificar
sustituto; no falsear una entrega**". In other words: if the
instrumentation we picked perturbs the target enough to invalidate
the evidence we were trying to capture, **detect it, fall back to a
lower-perturbation alternative, and document the substitution**. Do
not pretend the original measurement was valid.

ADAPTIVE_INSTRUMENTATION.md §L1–§L5 already describes the adaptive
ladder; this ADR defines the **policy** that selects where on the
ladder to sit at any moment, based on measured perturbation.

## 2. Decision

**Accepted.** Adopt a **3-tier perturbation ladder** with explicit
budgets and **automatic fallback** when a budget is exceeded. Each
tier maps to a position in the ADAPTIVE_INSTRUMENTATION.md ladder
and a corresponding Chronos capability.

### 2.1 Tier table

| Tier | Mechanism | Wall-clock overhead budget | Auto-fallback trigger |
|------|-----------|----------------------------|-----------------------|
| **T0 — Zero** | No instrumentation; replay-only via existing `tracing` / OTel | **0%** (baseline) | n/a |
| **T1 — USDT producer-only** | USDT probes fire but **no consumer attached** (`is_enabled = 0` short-circuit) | **≤ +60%** wall-clock, **+5%** binary | > +60% wall-clock OR > +5% binary |
| **T2 — USDT consumer-attached** | USDT probes + privileged `bpftrace`/`stap` consumer attached (each probe fires into kernel BPF) | **≤ +5×** wall-clock, **+5%** binary | > +5× wall-clock OR any probe-fire > 5 µs |
| **T3 — XRay sleds-only** | `-Z instrument-xray` sleds inserted, **patching inactive** | **≤ +25%** wall-clock, **+1%** binary | > +25% wall-clock OR > +1% binary |
| **T4 — XRay patching-active** | XRay patching runtime active; full per-function entry/exit capture | **degraded capture** | n/a — operator opt-in only |

**Tier T4 is NOT auto-selected.** It is reserved for **operator
opt-in** during a dedicated debug session with a controlled
iteration budget (per ADR-0008 §6 limitation 3). The auto-ladder
stops at T3.

### 2.2 Measured overhead (from ADR-0008 §5 + ADR-0009 §5)

Synthesised on the **same host** (kernel 7.2.4, Xeon E5-2682 v4,
rustc 1.98.1), **same workload** (`workload(10M)` with
`#[inline(never)]` on every function):

| Tier | Wall-clock 1 iter | Δ vs T0 | Binary Δ | Source |
|------|-------------------|---------|----------|--------|
| T0 (baseline)                | 24.3 ms (USDT baseline) | —     | —      | ADR-0009 §5 |
| T0 (baseline, XRay workload) | 34 ms (XRay baseline)   | —     | —      | ADR-0008 §5 |
| **T1 (USDT producer-only)**  | **38.2 ms**             | **+57%** | +4% | ADR-0009 §5 |
| **T3 (XRay sleds-only)**     | **40 ms**               | **+18%** | +0.2% | ADR-0008 §5 |
| **T2 (USDT consumer-attached)** | est. +1–5 µs/probe | depends on probe count | +4% | ADR-0009 §7 (not measured on this host) |
| **T4 (XRay patching-active)** | **22 200 ms**          | **+650× vs XRay T0, ~+900× vs USDT T0** | +0.2% | ADR-0008 §5 |

**Cross-reference:** both spikes used the **same `workload(10M)`
with `#[inline(never)]`** so the absolute numbers (24.3 ms baseline
USDT vs 34 ms baseline XRay) differ because the **binaries are
different**: the XRay workload uses `add(a, b)` (no USDT probe
calls) while the USDT workload calls `add(a, b)` and fires
`chrono::add_probe!(|| (a, b, s))`. The `+57%` USDT number includes
the volatile semaphore read + branch per probe; the `+18%` XRay
number is just the cost of the unconditional `jmp + NOP` sleds
(10 bytes) at every function entry/exit.

### 2.3 Fallback policy (the "M4R.5 deliverable")

When Chronos is investigating a target with a chosen tier T_n and
the **measured perturbation exceeds the budget for that tier**:

1. **Drop one tier** (T_n → T_{n-1}); re-measure perturbation;
   if within budget, proceed.
2. If even **T1 exceeds budget**, the target is so sensitive that
   **instrumentation is unsafe**. Report `unsupported` per
   No-Silent-Lies (ADR-0004) and offer the operator a **coarse
   coarse-only** alternative: capture only the entry/exit of the
   target's top-level function, no probes inside.
3. The fallback chain is **deterministic and one-way**: never
   auto-promote from T1 → T2 or T2 → T3 without explicit operator
   opt-in (matching the T4 rule).

### 2.4 What is NOT perturbation

Perturbation in this ADR means **runtime overhead** observable in
wall-clock latency and **binary-size growth**. It does **not**
include:

- **Cold-start latency** (USDT `register_probes()` is a one-shot
  syscall; XRay runtime init is also one-shot). Cold start is
  amortised and does not invalidate evidence.
- **Trace-data volume** (60 B per probe fire in the ADR-0009
  spike; 6 µs TSC + buffer write in ADR-0008). This is the
  **observation**, not perturbation.
- **Memory footprint** of the captured data in Chronos itself
  (separate concern, governed by `ReplayService` capacity — out
  of M4R.5 scope).

## 3. Why these tiers and not others

The 3-tier ladder maps to **what we can do without changing code**:

| Decision factor | T1 (USDT producer) | T3 (XRay sleds) |
|-----------------|--------------------|------------------|
| Requires nightly | No (stable 1.85+) | **Yes** (stable 1.98.1 does not expose `-Z instrument-xray`) |
| Requires source edit | **Yes** (probe macros in source) | No |
| Requires MSRV bump | **Yes** (workspace 1.75 → 1.85) | **Yes** (separate nightly pin) |
| Auto-fallback reachable? | **Yes** (drop to T0) | Yes (drop to T1 then T0) |
| Privilege required (consumer) | Yes (CapEff ≠ 0) | n/a (sleds are passive) |

T1 (USDT producer-only) is the **cheapest non-zero tier** because:
- The volatile semaphore read + branch is ~3–4 ns per call.
- The closure argument form means non-trivial argument computation
  is skipped when no consumer is attached.
- No kernel↔user transition.

T2 (USDT consumer-attached) is **NOT in the auto-ladder** because:
- Consumer attachment requires `CAP_SYS_ADMIN` + `CAP_BPF` + kernel
  headers (ADR-0009 §5.1 — host with CapEff=0 cannot exercise it).
- Each probe fire now costs ~1–5 µs (kernel BPF program execution
  + perf_event_output).
- The +650× overhead of T4 (XRay patching) makes T4 unusable for
  any but the shortest debug sessions (ADR-0008 §6 limitation 3).

The **M4R.5 deliverable is the fallback policy itself**, not a new
mechanism. ADR-0008 and ADR-0009 each measured one mechanism in
isolation; this ADR closes the loop by specifying **how to combine
them and degrade gracefully**.

## 4. Empirical basis

All measurements in §2.2 come from the off-repo spikes in
`/home/rubentxu/.jcode/scratch/{xray,usdt}-spike/` documented in
ADR-0008 §5 and ADR-0009 §5. SHA-256 of the spike binaries is in
those ADRs.

| Spike | Binary | SHA-256 (excerpt) |
|-------|--------|-------------------|
| ADR-0008 baseline | `xray-spike-baseline-linux-amd64` | `3b6396d99f0ce0f7b9d426fa960abf73b6fcd82f1d1676fcfe0e681a4390a90b` |
| ADR-0008 XRay-built | `xray-spike-xray-linux-amd64` | `9550d191b838a2763d7b5ef3c55fdc2c3e3896615f80ecbd87669f681c376f27` |
| ADR-0008 trace | `xray-trace-impl-10000000.bin` | `7f5bf1c82222edff3835cfd0eedc90eb655bf90724720799d14e1070b35decfc` |
| ADR-0009 baseline | `usdt-baseline-linux-amd64` | `3f4d83edfb18e5ca9a2c2615c59c99844c213dd240b82d5900f1547e2edd262e` |
| ADR-0009 USDT-built | `usdt-spike-linux-amd64` | `b326e0cdab4821306cf07496df3f8cfee85c211ae0a5d51c0dedfe9d58855f05` |

Both spikes were run on the same workload
(`workload(10M)` + `#[inline(never)]`). The absolute T0 differs
between ADR-0008 and ADR-0009 because the binaries differ (one
calls `add(a,b)`; the other calls `add(a,b)` and fires a probe) —
this is expected and documented.

## 5. Operational rules

1. **Always start at T1** when a target has USDT probes. T0 is the
   **fallback** only. The default Chronos stance is "have probes
   ready, fire them only when a consumer attaches".
2. **Promote to T2** only when the operator explicitly requests
   "attach a consumer now". Promotion logs a `probe_attach` event
   in the ExecutionLog with the consumer PID, host fingerprint,
   and perturbation measurement.
3. **T3 (XRay sleds)** requires a **separate debug build** with
   `-Z instrument-xray=always` plus the build.rs link per ADR-0008
   §3.2. The **production binary** stays on stable Rust, no XRay.
4. **T4 (XRay patching-active)** is **never auto-selected**. Use
   only with operator opt-in and a **fixed iteration budget** (e.g.
   10 M iterations, after which the program exits cleanly so the
   XRay buffer flushes).
5. **T0 fallback** disables ALL probe firing by setting the
   semaphore to 0 on each USDT probe (via
   `__usdt_sema_chrono_*`). The probes stay in the binary; they
   just cost nothing. **The probe metadata is preserved** so the
   investigation can resume at T1 without recompilation.

## 6. Limitations and out-of-scope

1. **T2 perturbation is not measured on this host** (CapEff=0; no
   `bpftrace`/`stap`). The 1–5 µs/profire estimate is from the
   Linux perf / BPF literature, not from this spike. Re-measure on
   a privileged host when M4R.4 / UAT-M4-R-01 is executed.
2. **Memory-pressure perturbation is not modelled.** USDT fires
   into Chronos's ReplayService; XRay fires into its own buffer
   before flushing. Neither path is bounded in this ADR. Tracked
   as `CapReplayBound` follow-up.
3. **Probe argument evaluation cost is not modelled.** The closure
   form (`|| (a, b, s)`) defers evaluation until `is_enabled`, so
   the cost is paid only when a consumer is attached. Non-trivial
   argument expressions (e.g. `format!("{}", s)`) need to be
   measured per-probe when the probe is in T2.
4. **Cross-arch overhead is not measured.** x86_64 only in this
   spike. aarch64 has different volatile read / branch costs;
   re-measure when chronos targets aarch64.
5. **Cold-start latency is not modelled** (per §2.4). If the
   target is a short-lived batch process, T1 cold-start may be
   a significant fraction. Tracked as `CapColdStartLatency`.

## 7. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0010-m4r5-perturbation-fallback.md`
  (this file).

**No Chronos product code changed.** This ADR closes M4-F0's
perturbation requirement and **opens** M4-F1's M4R.5 entry. The
fallback policy (§2.3) is **policy-only**; implementation belongs to
M4R.3 (`InstrumentationSpec` determinism) and M4G.2.

## 8. Cross-references

- ROADMAP §M4-F1 §83 — "M4R.5 perturbación detectada + fallback" —
  **delivered by this ADR**.
- ROADMAP §M4-F1 preamble — "conservar el objetivo de evidencia y
  justificar sustituto; no falsear una entrega" — **operationalised
  by §2.3 fallback policy**.
- ADAPTIVE_INSTRUMENTATION.md §L1–§L5 — ladder positions mapped to
  tiers in §2.1.
- ADR-0004 (No Silent Lies) — applies to perturbation: when
  instrumentation perturbs too much, `unsupported` is a valid
  outcome (§2.3 step 2).
- ADR-0006 (reuse OTel/OBI before custom) — T1 (USDT producer-only)
  is upstream-owned probe instrumentation; Chronos does not write
  probe code, only consumes.
- ADR-0008 (M4R.1 XRay) — provides the T3 and T4 measurements.
- ADR-0009 (M4R.2 USDT) — provides the T1 measurement and T2
  estimate.
- TECHNOLOGY_BASELINE.md §5 (XRay), §6 (USDT) — both referenced
  here with their measured numbers.

## 9. Closing note

M4-F0 (the inventory + spike phase) is now **completely closed**:

- Go side: M4G.1 (`otelc`) — accepted with constraints.
- Rust side: M4R.1 (XRay) + M4R.2 (USDT) + M4R.5 (this ADR,
  perturbation policy) — all accepted.

M4-F1 is the **real-work** phase: M4G.2 (spec), M4G.3 (Go
checkout-bug), M4R.3 (overlay), M4R.4 (Rust state-corruption).
None of those can start without this perturbation policy in place,
because every one of them measures overhead against the budgets
defined here.
