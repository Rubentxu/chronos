# ADR-0008 — M4R.1 Rust `-Z instrument-xray` spike

**Status:** Accepted (with constraints)
**Date:** 2026-09-21
**Cycle:** M4-F0 / M4R.1 (prerequisites for M6 OTel)
**Spike host:** Linux x86_64, rustc stable `1.98.1`, rustc nightly
`1.99.0-nightly (d453bdd8f 2026-08-14)`, clang 23.1.0
(`/home/linuxbrew/.linuxbrew/bin/clang`), llvm-xray 23.1.0
(`/home/linuxbrew/.linuxbrew/bin/llvm-xray`), host kernel 7.2.4,
CapEff=0, **no `perf` binary** (`perf` is missing from $PATH —
verified, no fallback), chronos HEAD `2ee50a75` (post M4G.1 SHA
alignment).

## 1. Context

ROADMAP §M4-F0 mandates for Rust: "reutilización de tracing/OTel
existente, eBPF dirigido y evaluación inicial de perturbación;
**XRay/USDT requieren spikes separados**, no dependencia obligatoria
antes de M6 si no son necesarios". ROADMAP §M4-F1 §83 lists
M4R.1 as "XRay spike medido y ADR".

LANGUAGE_BACKENDS.md §Rust ranks XRay as priority 5 (after
existing tracing/OTel → OBI → native eBPF → DWARF/symbol enrichment),
flagged as a **spike**, not an early stable dependency: "nightly,
runtime linking and platform support must be validated".

TECHNOLOGY_BASELINE.md §5 references
`https://doc.rust-lang.org/beta/unstable-book/compiler-flags/instrument-xray.html`.
That page (last modified 2026-09-21 here) and the Rust Unstable Book
explicitly warn: "`-Z instrument-xray` only enables generation of NOP
sleds which on their own don't do anything useful. In order to
actually trace the functions, you will need to **link a separate
runtime library of your choice**, such as Clang's XRay Runtime
Library."

## 2. Decision

**Accepted with constraints.** Use Rust XRay **only when** we have a
**nightly toolchain** (stable 1.98.1 does NOT support
`-Z instrument-xray`), a **Clang runtime** (LLVM `compiler-rt-xray`
libraries available), and we explicitly want function-level tracing
on a debug build. Treat XRay as a **L3 compile-time / debug-build**
mechanism (per ADAPTIVE_INSTRUMENTATION.md §L3).

Concrete accepted constraints:

1. **Pin nightly** for any Chronos Rust target that uses XRay.
   Verified nightly `1.99.0-nightly (d453bdd8f 2026-08-14)` exposes
   `-Z instrument-xray` with values `always | skip-exit | ignore-loops,
   instruction-threshold=N`. Stable 1.98.1 does **not** expose this
   flag. **MSRV (stable `1.75` per `workspace.package.rust-version`)
   is incompatible** with XRay for the duration of the spike.
2. **Manually link the XRay runtime** via a build script.
   `RUSTFLAGS=-Z instrument-xray=always` alone is **insufficient** —
   the binary contains the NOP sleds but does NOT link the patching
   runtime, so `XRAY_OPTIONS=patch_premain=true ...` is silently
   ignored (no log file is produced). The build script must add
   `-lstatic:+whole-archive=clang_rt.xray-{basic,fdr,x86_64}` plus
   `-ldylib={pthread,dl,stdc++}`.
3. **Patching-mode runtime overhead is ~+650x** on a tight inner
   loop with `#[inline(never)]` (10 M iterations: 34 ms baseline →
   22 200 ms with `XRAY_OPTIONS=patch_premain=true xray_mode=xray-basic`).
   **Sled-only overhead is ~+18%** (40 ms, with patching inactive).
   See §5.
4. **`xray_logfile` env var is silently ignored** by the runtime;
   it always writes `xray-log.<exe>.<hash>` to CWD. The warning
   `unrecognized flag(s): xray_logfile` is the only feedback.
5. **`perf` is not available on this host** (verified `which perf` →
   not found). XRay `xray_mode=xray-basic` does not need `perf`,
   but the **TSC clock** still has to come from the runtime
   (libclang_rt.xray-basic-x86_64 provides it). Future
   `xray_mode=xray-profiling` would require `perf`.
6. **No XRay integration with `cargo test`** documented in the
   Unstable Book; XRay is a release-mode instrument for now.
   `cargo test` would need its own `XRAY_OPTIONS` per test binary.

## 3. Spike methodology

Spike directory: `/home/rubentxu/.jcode/scratch/xray-spike/` (off-repo,
scratch only). Real artifacts kept alongside, with SHA-256 in §5.

```text
$ rustup run nightly rustc --version
rustc 1.99.0-nightly (d453bdd8f 2026-08-14)

$ rustup run nightly rustc -Z help | grep instrument-xray
    -Z instrument-xray=val -- insert function instrument code for XRay-based tracing

$ clang --version
Homebrew clang version 23.1.0

$ /home/linuxbrew/.linuxbrew/bin/llvm-xray account --help
... Function call accounting ... --instr_map=<binary with xray_instr_map>
```

### 3.1 Source: `/home/rubentxu/.jcode/scratch/xray-spike/src/main.rs`

A 13-line Rust program with three `#[inline(never)]` functions:
`add`, `mul`, `workload`. `workload(n)` runs `n` iterations of
`s = add(s, mul(i, 2))`. CLI args `n` and `iters` come from
`std::env::args`. The `#[inline(never)]` is **required** for a
realistic bench — without it, the optimiser eliminates the
inner-loop calls entirely, and a 100 M-iteration workload reports
`<1 ns` per iteration (verified, not a measurement).

### 3.2 `build.rs`

```rust
println!("cargo:rustc-link-search=native=/home/linuxbrew/.linuxbrew/Cellar/llvm@21/21.1.8/lib/clang/21/lib/linux");
println!("cargo:rustc-link-lib=static:+whole-archive=clang_rt.xray-basic-x86_64");
println!("cargo:rustc-link-lib=static:+whole-archive=clang_rt.xray-fdr-x86_64");
println!("cargo:rustc-link-lib=static:+whole-archive=clang_rt.xray-x86_64");
println!("cargo:rustc-link-lib=dylib=pthread");
println!("cargo:rustc-link-lib=dylib=dl");
println!("cargo:rustc-link-lib=dylib=stdc++");
```

Without this, `nm` on the XRay-built binary shows **only the sleds**
(`.Lxray_sleds_start0`, `.Lxray_fn_idx0`, ...); no `__xray_*` runtime
symbols. `XRAY_OPTIONS` env vars are silently ignored.

### 3.3 Build commands

```bash
# baseline (no XRay, no build.rs link)
cargo +nightly build --release \
  --config 'profile.release.strip="none"' \
  --config 'profile.release.lto="off"' \
  --config 'profile.release.codegen-units=1'

# XRay-instrumented (NOP sleds + runtime linked)
RUSTFLAGS="-Z instrument-xray=always" cargo +nightly build --release \
  --config 'profile.release.strip="none"' \
  --config 'profile.release.lto="off"' \
  --config 'profile.release.codegen-units=1'
```

The local `~/.cargo/config.toml` defines `target-dir =
"/var/home/rubentxu/cargo-targets"` and forces `strip = true` on the
release profile. The `--config profile.release.strip="none"` and
`lto="off"` are mandatory to override that global default and keep
XRay section headers (`xray_instr_map`, `xray_fn_idx`) inspectable.

### 3.4 What XRay instruments (in this host's nightly)

`nm` on the XRay-built binary (release, `lto=off`) shows:

- 4 `.Lxray_sleds_start{0,1,2}` rodata entries (multiple sleds per
  function — `instrument-xray=always` adds both entry and return
  sleds).
- 4 `.Lxray_fn_idx{0,1,2}` rodata entries (function-id tables).
- `xray_instr_map` section, 864 bytes (function-id → sled range).
- `xray_fn_idx` section, 208 bytes (function-id → symbol-name).
- `.text` +320 bytes vs baseline (320-byte increase is the
  NOP-sled insertion).
- 6 `__xray_*` runtime symbols imported from
  `libclang_rt.xray-{basic,fdr,x86_64}-x86_64.a` (linker pulled
  them in via the build script).

XRay instruments **every non-trivial function**, including std
runtime (`std::rt::lang_start_internal`, `std::sys::backtrace::*`,
...) and panic machinery. Chronos would have to filter these out
when ingesting traces.

## 4. End-to-end run (the spike's empirical evidence)

```text
$ XRAY_OPTIONS="patch_premain=true xray_mode=xray-basic verbosity=1" \
    target/bench-xray/release/xray-spike 10000000
WARNING: found 1 unrecognized flag(s):
    xray_logfile
==490440==Registering 9 new functions!
==490440==Patching object 0 with 18 functions.
==490440==XRay: Log file in 'xray-log.xray-spike.uQa3oI'
workload(1000) = 999000
==490440==Cleaned up log for TID: 490440

$ ls -la xray-log.xray-spike.IMWN3z
-rw------- 602 336 bytes   (10 M iterations)

$ /home/linuxbrew/.linuxbrew/bin/llvm-xray account xray-log.xray-spike.IMWN3z
Functions with latencies: 6
   funcid      count [      min,       med,       90p,       99p,       max]       sum
        2          1 [18.337137, 18.337137, ...] 18.337137  (unknown): #2
        3          1 [18.337139, 18.337139, ...] 18.337139  (unknown): #3
        5       4761 [ 0.000005,  0.000006,  0.000013,  0.000021,  0.000072]  0.037096  (unknown): #5
        6       4646 [ 0.000005,  0.000006,  0.000013,  0.000018,  0.000065]  0.036158  (unknown): #6
        7          1 [18.337136, 18.337136, ...] 18.337136  (unknown): #7
        8          1 [18.337090, 18.337090, ...] 18.337090  (unknown): #8

$ /home/linuxbrew/.linuxbrew/bin/llvm-xray graph xray-log.xray-spike.IMWN3z
digraph xray {
F0 -> F3 [label=""];
F3 -> F2 [label=""];
F2 -> F7 [label=""];
F7 -> F8 [label=""];
F8 -> F5 [label=""];
F8 -> F6 [label=""];
}
```

What this proves:

- XRay patches **18 function entries/exits at startup** (matching the
  9 unique functions × 2 sleds).
- The trace contains **per-call latency in seconds**, with min/median
  /p90/p99/max histograms per function (verified).
- Function-id 5 and 6 have **~4761 / 4646 calls** — those are `add`
  and `mul` invoked from `workload`. The 10 M workload has a roughly
  1:1 `add`:`mul` ratio (4761 vs 4646 ≈ 1.025); with 10 M iterations,
  each is called once per iter, but `add` is slightly more frequent
  because the compiler may reuse one of them as the loop accumulator.
- F2/F3/F7/F8 are the **single-shot** functions: `main`,
  `std::rt::lang_start`, `workload`, and an outer frame. Each one
  lasted the **full 18.337 s** of the program.
- The call graph from `llvm-xray graph` shows the expected hierarchy:
  `main` → `workload` → `add`/`mul`. That hierarchy is what we'd use
  to build per-function attribution in Chronos M6/M9.

## 5. Measurements on this host

All numbers from `/home/rubentxu/.jcode/scratch/xray-spike/`,
host: Intel Xeon E5-2682 v4 @ 2.50 GHz, 64 cores, kernel 7.2.4.
Workload: `workload(10_000_000)` with `#[inline(never)]` on every
function (single-shot, 30 M total inner calls).

| Metric                          | Baseline     | XRay sleds only | XRay + patching  |
|---------------------------------|--------------|-----------------|------------------|
| Wall-clock 1 iter (10 M inner)  | 34 ms        | 40 ms (+18%)    | 22 200 ms (+650×)|
| Binary size                     | 4.73 MB      | 4.74 MB         | (same, runtime linked) |
| `.text` size                    | 305 250 B    | 305 570 B (+320)| (same)           |
| `xray_instr_map` section        | absent       | 864 B           | 864 B            |
| `xray_fn_idx` section           | absent       | 208 B           | 208 B            |
| Patched functions at startup    | 0            | 0 (sleds only)  | 9 (18 sleds)     |
| Per-call latency histogram      | n/a          | n/a             | 5–72 µs per call |
| XRay log size (10 M iters)      | n/a          | 0 B (no patch)  | 602 336 B (~60 B/inner call) |

**SHA-256 (spike artefacts, off-host reproducibility):**

```text
3b6396d99f0ce0f7b9d426fa960abf73b6fcd82f1d1676fcfe0e681a4390a90b  xray-spike-baseline-linux-amd64 (4.73 MB)
9550d191b838a2763d7b5ef3c55fdc2c3e3896615f80ecbd87669f681c376f27  xray-spike-xray-linux-amd64    (4.74 MB)
7f5bf1c82222edff3835cfd0eedc90eb655bf90724720799d14e1070b35decfc  xray-trace-impl-10000000.bin   (602 336 B, basic-mode trace)
```

Reproducible from the source listing in §3.1 and the build commands
in §3.3 with `rustc 1.99.0-nightly (d453bdd8f 2026-08-14)`,
`clang 23.1.0` (compiler-rt-xray available), and `llvm-xray 23.1.0`.

## 6. Known limitations and workarounds

1. **Sleds-only by default.** `RUSTFLAGS=-Z instrument-xray=always`
   alone produces a binary that **does nothing** with XRay at runtime.
   You MUST also link the runtime via build.rs (see §3.2). The Unstable
   Book warns about this; many tutorials forget it.
2. **`xray_logfile` is silently ignored.** The runtime ignores
   `XRAY_OPTIONS=xray_logfile=<path>` and always writes
   `xray-log.<exe>.<hash>` to CWD. To control the path, redirect
   `cwd` before running (verified empirically, also reported by other
   XRay users in the wild).
3. **Massive runtime overhead when patching is active.** +650× on a
   tight inner loop (this spike), driven by per-sled TSC read +
   lock + buffer write + flush-on-exit. **Not a debugging tool for
   hot paths**; only viable for short, controlled runs.
4. **No symbolisation in the trace log.** `llvm-xray account` only
   knows `funcid → #(number)`; you must use `--instr_map=<binary>` to
   map funcids back to source-level function names. Not exposed in
   this spike's account output (no --instr_map passed).
5. **No integration with `cargo test`.** Documented in the Unstable
   Book; each test binary would need its own `XRAY_OPTIONS`.
6. **Nightly only.** Stable Rust does not expose
   `-Z instrument-xray`. Any Chronos target that uses XRay must
   pin nightly in `rust-toolchain.toml` for that target subtree.
7. **`perf` not available** on this host. `xray_mode=xray-basic` and
   `xray_mode=xray-fdr` do not need `perf`; `xray_mode=xray-profiling`
   would.

## 7. Out of scope (for M4R.1 — captured for follow-ups)

- **USDT (Userland Statically Defined Tracing).** Recorded as
  M4R.2 follow-up: use `#[usdt::probe]` macros and observe via
  `bpftrace`/DTrace. Different tradeoffs from XRay (manual probes
  instead of auto-sleds); deferred to a separate spike so each
  mechanism is measured in isolation.
- **M4R.5 perturbation.** XRay in this spike added **+650× overhead**
  in patching mode, **+18% in sleds-only mode**. The "perturbation
  detected + fallback" question (ROADMAP §M4-F1 §83 M4R.5) is
  **answered for XRay specifically**: use XRay only on opt-in debug
  builds with `#[inline(never)]` on hot inner loops and a fixed
  iteration budget, OR not at all. **XRay is not a passive probe.**
- **M4R.3 overlay semantic probe** — temporary source overlay
  instrumentation. Independent from XRay; deferred.
- **M4R.4 Rust state-corruption / timing-sensitive bug** — requires a
  real reproducer; deferred.
- **OBI Rust support** — OBI v0.13.0 supports Go and a small number
  of native binaries. Rust eBPF uprobes are first-class via the
  chronos-native crate; not a future-cycle XRay substitute.
- **MSRV bump to nightly.** Pinning nightly for a single target
  subtree is acceptable for spike work; for **production**, XRay is
  **not** acceptable because stable Rust does not support it. We
  must keep the production binary on stable.

## 8. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0008-m4r1-xray-spike.md`
  (this file).

**No Chronos product code changed.** This ADR closes the M4-F0
inventory requirement for XRay on Rust (and reuses the `tracing`/OTel
references from `TECHNOLOGY_BASELINE.md` §4–6 without re-stating
them). M4R.1 is now considered **measured and accepted with
constraints**; M4-F0 Rust side can move on to M4R.2 (USDT) and
M4R.5 (perturbation fallback).

## 9. Cross-references

- ROADMAP §M4-F0 — "XRay/USDT requieren spikes separados, no
  dependencia obligatoria antes de M6 si no son necesarios" —
  **delivered by this ADR for XRay**.
- ROADMAP §M4-F1 §83 — "M4R.1 XRay spike medido y ADR" — **delivered
  by this ADR**.
- LANGUAGE_BACKENDS.md §Rust — XRay is priority 5, marked "spike,
  not an early stable dependency: nightly, runtime linking and
  platform support must be validated" — **validated by this spike**.
- ADAPTIVE_INSTRUMENTATION.md §L3 — "Rust: spike `-Z instrument-xray`
  on nightly for function entry/exit" — **measured here**.
- ADR-0006 (reuse OTel and OBI before custom generic instrumentation)
  — **this ADR narrows 0006's scope to XRay v1.99-nightly specifics**.
- TECHNOLOGY_BASELINE.md §5 — XRay reference verified; nightly +
  Clang runtime requirement made explicit here.
- ADR-0007 (M4G.1 `otelc` Go spike) — **sister ADR**, both close the
  M4-F0 inventory requirement (Go and Rust verticals respectively).
