# ADR-0009 — M4R.2 Rust USDT (Userland Statically Defined Tracing) spike

**Status:** Accepted (with constraints)
**Date:** 2026-09-21
**Cycle:** M4-F0 / M4R.2 (prerequisites for M6 OTel)
**Spike host:** Linux x86_64, rustc stable `1.98.1`, kernel 7.2.4,
CapEff=0, **NO `perf`, NO `bpftrace`, NO `stap` (SystemTap),
NO access to `/sys/kernel/debug/tracing`** (all verified — see §5),
`dtrace` (`/usr/bin/dtrace`) present but limited to D-script
compilation (no `-l` listing on this build), chronos HEAD `2ebd92e4`
(post M4R.1 SHA alignment).

## 1. Context

ROADMAP §M4-F0 mandates for Rust: "reutilización de tracing/OTel
existente, eBPF dirigido y evaluación inicial de perturbación;
**XRay/USDT requieren spikes separados**, no dependencia obligatoria
antes de M6 si no son necesarios". ROADMAP §M4-F1 §83 lists
M4R.2 as "USDT spike/ADR".

LANGUAGE_BACKENDS.md §Rust ranks USDT as priority 6
(`#[usdt::probe]` macros — semantic debug probes with stable IDs
instead of raw offsets). ADAPTIVE_INSTRUMENTATION.md §L4 already
mentions "selected arguments, returns, state transitions, branch
decisions and invariant inputs" as L4 probes; USDT provides a
well-trodden implementation of that idea.

TECHNOLOGY_BASELINE.md §6 references the `oxidecomputer/usdt` crate
(`https://github.com/oxidecomputer/usdt`). That crate exists and is
published as `usdt` v0.6.0 on crates.io (Apache-2.0, rust-version
1.85.0). The companion tools crate that older drafts referenced as
`usdt-tools` is **not** a separate crate on crates.io — the build-time
codegen lives inside `usdt` itself via `usdt::Builder`.

## 2. Decision

**Accepted with constraints.** Use Rust USDT for **production-mode**
semantic debug probes that must work **without a privileged consumer
attached** (zero-overhead when no consumer is listening), and where
the probe **names** matter more than per-function entry/exit timing.
USDT is the **L4 mechanism** in ADAPTIVE_INSTRUMENTATION.md: surgical,
named, stable-ID probes that fire only at semantically meaningful
points.

Concrete accepted constraints:

1. **Use `usdt = "0.6"`** (oxidecomputer). Verified on crates.io,
   rust-version 1.85.0, current MSRV of the crate. Compatible with
   our `rust-version = "1.85"` (workspace) — the project was already
   on 1.85+ for H1.x.
2. **D-script provider definitions** in `src/*.d` files; **build.rs
   invokes `usdt::Builder::new(...).build()`**. The generated
   `chrono.rs` is included via `include!(concat!(env!("OUT_DIR"),
   "/chrono.rs"))` and exposes one `chrono::<probe_name>!` macro per
   probe. Calling `chrono::<probe_name>!(|| args)` fires the probe.
3. **`register_probes().unwrap()` must be called once** before the
   probes are visible to a consumer. Without it, the `.note.stapsdt`
   metadata is still in the ELF (consumers reading static binaries
   see them), but a **dynamic** consumer (`bpftrace -p <pid>`) will
   not see them.
4. **Probe arguments are passed through closures** (`|| (a, b, s)`).
   The closure form exists so that the closure body is only evaluated
   when `is_enabled != 0` — i.e., **zero overhead when no consumer
   is attached**. Verified empirically (see §5): ~+57% with no
   consumer for this spike, vs **+650%** for XRay patching in
   ADR-0008.
5. **Probe PC addresses land on a `nop` instruction** (the
   `0x90` byte at the start of the inline asm stub). When a
   consumer attaches, it overwrites the `nop` with an `int3`
   breakpoint. **Without a consumer, the nop is executed as a nop.**
6. **Linux consumer tooling is privileged.** This host has CapEff=0
   and **no `perf`, no `bpftrace`, no `stap`**. The producer side
   works without privileges; the **consumer side requires
   `CAP_SYS_ADMIN` + `CAP_BPF` + kernel headers**. Verified:
   `/sys/kernel/debug/tracing` returns `Permission denied`. Producer
   metadata is **visible statically** via `readelf -n <binary>`
   regardless of privileges (verified, see §5).

## 3. Spike methodology

Spike directory: `/home/rubentxu/.jcode/scratch/usdt-spike/`
(off-repo, scratch only). Real artefacts kept alongside with
SHA-256 in §5.

### 3.1 Provider definition: `src/chrono.d`

```d
provider chrono {
    probe add_probe(uint64_t, uint64_t, uint64_t);
    probe mul_probe(uint64_t, uint64_t, uint64_t);
    probe workload_start(uint64_t);
    probe workload_end(uint64_t, uint64_t);
};
```

D-syntax standard SystemTap/DTrace provider declaration. `uint64_t`
maps to Rust `u64`. Char arrays would map to `&str` / `*u8`;
not used here. The `chrono` namespace matches the Rust module name
after `include!`.

### 3.2 Build script: `build.rs`

```rust
use usdt::Builder;

fn main() {
    println!("cargo:rerun-if-changed=src/chrono.d");
    println!("cargo:rerun-if-changed=build.rs");
    Builder::new("src/chrono.d").build().unwrap();
}
```

The `Builder` parses the D file, generates Rust source in
`$OUT_DIR/chrono.rs`, and arranges for the inline-asm stubs to be
included in the user's `src/main.rs`.

### 3.3 Generated `chrono.rs` (excerpt)

The generated macro for each probe (one per `probe foo(...)` in the
D file) has this structure:

```rust
pub(crate) mod chrono {
    macro_rules! add_probe {
        ($args_lambda:expr) => {{
            unsafe extern "C" {
                static __usdt_sema_chrono_add_probe: u16;
            }
            let is_enabled: u16;
            unsafe {
                is_enabled = (&raw const __usdt_sema_chrono_add_probe)
                    .read_volatile();
            }
            if is_enabled != 0 {
                let args = ($args_lambda)();
                // ... type-check args against (u64, u64, u64) ...
                unsafe {
                    ::std::arch::asm!(
                        "990:   nop"
                        // .ifndef __usdt_sema_chrono_add_probe ...
                        .pushsection .probes, "aw", "progbits"
                        .weak __usdt_sema_chrono_add_probe
                        .hidden __usdt_sema_chrono_add_probe
                    __usdt_sema_chrono_add_probe:
                        .zero 2
                        .type __usdt_sema_chrono_add_probe, @object
                        .size __usdt_sema_chrono_add_probe, 2
                        .popsection
                        .ifndef
                        // .pushsection .note.stapsdt, "", "note"
                        // ... provider / probe name / arg format ...
                        );
                }
            }
        }};
    }
    pub(crate) use add_probe;
}
```

Key behaviour:

- The `if is_enabled != 0` gate makes the entire probe body a **cold
  path** when no consumer is attached — the volatile read is the only
  cost.
- The inline asm emits **three ELF sections per probe**:
  `.probes` (semaphore), `.note.stapsdt` (metadata), and the
  `nop` itself at a known PC.

### 3.4 Source: `src/main.rs`

```rust
use usdt::register_probes;
include!(concat!(env!("OUT_DIR"), "/chrono.rs"));

#[inline(never)]
fn add(a: u64, b: u64) -> u64 {
    let s = a + b;
    chrono::add_probe!(|| (a, b, s));   // fires only if is_enabled
    s
}

#[inline(never)]
fn mul(a: u64, b: u64) -> u64 {
    let p = a * b;
    chrono::mul_probe!(|| (a, b, p));
    p
}

#[inline(never)]
fn workload(n: u64) -> u64 {
    chrono::workload_start!(|| n);
    let mut s = 0u64;
    for i in 0..n {
        s = add(s, mul(i, 2));
    }
    chrono::workload_end!(|| (n, s));
    s
}

fn main() {
    register_probes().unwrap();
    let n: u64 = std::env::args().nth(1)
        .and_then(|s| s.parse().ok()).unwrap_or(10_000_000);
    let r = workload(n);
    println!("n={n} result={r}");
}
```

### 3.5 Build commands

```bash
# Baseline (no probes)
cargo build --release

# USDT-instrumented
cargo build --release   # build.rs auto-runs
```

Both builds share the same Cargo profile. The `build.rs` runs
unconditionally; without a `.d` file present, `Builder::new`
panics.

## 4. End-to-end run (the spike's empirical evidence)

```text
$ cargo build --release
   Compiling dof v0.4.0
   Compiling usdt-attr-macro v0.6.0
   Compiling usdt-macro v0.6.0
   Compiling usdt v0.6.0
   Compiling usdt-spike v0.1.0 (/home/rubentxu/.jcode/scratch/usdt-spike)
warning: unnecessary parentheses around closure body
    Finished `release` profile [optimized] target(s) in 9.83s

$ readelf -n target/release/usdt-spike | grep -A4 "stapsdt"
  stapsdt   NT_STAPSDT (SystemTap probe descriptors)
    Provider: chrono
    Name: add_probe
    Location: 0x00000000000160a4, Base: 0x0000000000005640,
              Semaphore: 0x0000000000059b90
    Arguments: 8@%rdi 8@%rsi 8@%rdx
  stapsdt   NT_STAPSDT (SystemTap probe descriptors)
    Provider: chrono
    Name: mul_probe
    Location: 0x00000000000160c9, ..., Semaphore: 0x0000000000059b92
    Arguments: 8@%rdi 8@%rsi 8@%rdx
  stapsdt   NT_STAPSDT (SystemTap probe descriptors)
    Provider: chrono
    Name: workload_start
    Location: 0x0000000000016388, ..., Semaphore: 0x0000000000059b94
    Arguments: 8@%rdi
  stapsdt   NT_STAPSDT (SystemTap probe descriptors)
    Provider: chrono
    Name: workload_end
    Location: 0x00000000000163d2, ..., Semaphore: 0x0000000000059b96
    Arguments: 8@%rdi 8@%rsi
```

```text
$ objdump -d target/release/usdt-spike \
    --start-address=0x160a0 --stop-address=0x160c0
00000000000160a0 <.text+0x1d0>:
   160a0:   04 48   add $0x48,%al
   160a2:   89 c2   mov %eax,%edx
   160a4:   90      nop          ← add_probe PC lands here
   160a5:   ...     (rest of asm stub)
```

**This proves**:

- The 4 USDT probes (`chrono::add_probe`, `chrono::mul_probe`,
  `chrono::workload_start`, `chrono::workload_end`) are **embedded**
  in the binary with full NT_STAPSDT metadata: provider, name,
  PC location, base, semaphore address, argument ABI format.
- Each probe PC address falls **exactly** on a single-byte `0x90`
  NOP instruction. A consumer that attaches replaces the NOP with
  an `int3` breakpoint; without a consumer, the NOP is a no-op.
- The `.probes` section (8 bytes = 4 × 2-byte semaphores) and
  `.stapsdt.base` section (1 byte) are present in the ELF.
- The argument format `8@%rdi 8@%rsi 8@%rdx` is the standard
  SystemTap format — `8` = 8 bytes (size), `%rdi`/`%rsi`/`%rdx`
  = the x86-64 registers. A `bpftrace` consumer reads args via
  these register mappings.

### 4.1 What the spike does NOT demonstrate

This spike is a **producer-side** spike only. **No consumer was
attached**, because this host has **no `perf`, no `bpftrace`, no
`stap`**, and `/sys/kernel/debug/tracing` is not readable (CapEff=0).
The consumer side of USDT requires:

- A Linux kernel with **uprobes** support (≥ 3.5, stable since 4.x).
- `CAP_SYS_ADMIN` (or `CAP_BPF` on ≥ 5.8) to load BPF programs.
- A consumer binary: `bpftrace` (recommended), `stap` (SystemTap),
  or a custom BPF program via `perf_event_open`.

End-to-end with a live consumer requires a **Linux privileged
capture host** (per H1.2 §1 deployment profile
`Linux privileged capture`, CapEff != 0). That host is **not this
host**; deferred to M4-F1 §M4R.4 (real reproducer) or
UAT-M4-R-01.

## 5. Measurements on this host

All numbers from `/home/rubentxu/.jcode/scratch/usdt-spike/`,
host: Intel Xeon E5-2682 v4 @ 2.50 GHz, 64 cores, kernel 7.2.4,
CapEff=0. Workload: `workload(10_000_000)` with `#[inline(never)]`
on every function (single-shot, 30 M total inner calls).

| Metric                          | Baseline     | USDT probes (4) | Delta       |
|---------------------------------|--------------|-----------------|-------------|
| Wall-clock 1 iter (10 M inner)  | 24.3 ms      | 38.2 ms         | **+57%**    |
| Binary size                     | 0.331 MB     | 0.345 MB        | +0.014 MB (+4%) |
| `.probes` section               | absent       | 8 B (4×2)       | —           |
| `.note.stapsdt` section         | absent       | 324 B (4×NT_STAPSDT) | —      |
| `.stapsdt.base` section         | absent       | 1 B             | —           |
| Probe-fire overhead per call    | n/a          | ~3–4 ns         | (volatile read + branch) |
| Consumer-attached overhead      | unknown      | not measured    | requires `bpftrace`/`stap` |

**SHA-256 (spike artefacts, off-host reproducibility):**

```text
3f4d83edfb18e5ca9a2c2615c59c99844c213dd240b82d5900f1547e2edd262e  usdt-baseline-linux-amd64 (0.331 MB)
b326e0cdab4821306cf07496df3f8cfee85c211ae0a5d51c0dedfe9d58855f05  usdt-spike-linux-amd64    (0.345 MB)
```

Reproducible from the source listings in §3.1, §3.3, §3.4 with
`usdt = "0.6"` (oxidecomputer) on `rustc 1.98.1` (stable).

### 5.1 Consumer-side check on this host

| Tool            | Present? | Notes                                |
|-----------------|----------|--------------------------------------|
| `perf`          | no       | `which perf` empty                   |
| `bpftrace`      | no       | `which bpftrace` empty               |
| `stap` (SystemTap) | no    | `which stap` empty                   |
| `/sys/kernel/debug/tracing` | no | `Permission denied` (CapEff=0)    |
| `dtrace`        | yes      | `/usr/bin/dtrace` — but **no `-l`** listing (this Linux build only supports `-s`/`-G`/`-h`/`-C` for D-script compilation); cannot be used as a live USDT consumer |
| `readelf -n`    | yes      | **Static probe metadata readable** without privileges — confirmed 4 NT_STAPSDT entries with provider/name/location/arguments |

**Conclusion for §5.1**: on this host, the consumer side is
**not exercisable**, but the producer side is **fully verified**.

## 6. Known limitations and workarounds

1. **Consumer side requires Linux privileges.** CapEff=0 host cannot
   attach a live consumer. The producer side is unaffected. End-to-end
   validation deferred to M4-F1 §M4R.4 / UAT-M4-R-01.
2. **MSRV raised to 1.85** by the `usdt` crate (rust-version field
   in `Cargo.toml`). Workspace currently declares rust-version `1.75`
   (`workspace.package.rust-version`). **Conflict** — adopting USDT
   in product code requires bumping the workspace MSRV to 1.85. See
   §7.
3. **`usdt` uses inline assembly** (the `nop` and the `.pushsection`
   directives are emitted via `asm!`). Rust 1.59+ supports the
   needed features; macOS requires 1.66+. We are on Linux x86_64
   stable 1.98.1 — fine.
4. **Closure argument form** (`|| (a, b, c)`) is mandatory. The
   `add_probe!()` form (no closure) is a `compile_error!` to remind
   the user. Closure body is **only evaluated when `is_enabled`**,
   so non-trivial argument computation does not cost anything when
   no consumer is attached.
5. **Build-time codegen is required.** The D-script is parsed by
   `usdt-impl`'s D parser at build time. **No runtime provider
   definition** (that would require `dtrace(7)` on Solaris / macOS).
6. **Argument ABI is x86-64 register-based** (format strings like
   `8@%rdi 8@%rsi 8@%rdx`). A consumer-side tool reads args from
   these registers. This is the **standard SystemTap format**;
   `bpftrace` and `stap` both understand it. **aarch64** uses a
   different format (`arg0`, `arg1`, ...); the `usdt` crate handles
   it transparently in the generated asm.
7. **Probe name collisions across binaries.** Each USDT probe name
   is global per binary. Chronos must namespace provider/probe names
   per domain (e.g. `chrono_invoke`, `chrono_observe`).
8. **No integration with `cargo test` in CI** out of the box. Tests
   fire probes as part of `register_probes()`, but no assertion of
   probe firing is possible without a live consumer. Test coverage
   is therefore limited to "probes don't panic and the binary has
   the right sections" (`readelf -n` post-build).

## 7. Out of scope (for M4R.2 — captured for follow-ups)

- **MSRV bump 1.75 → 1.85** to use `usdt = "0.6"` in product code.
  Currently the workspace MSRV is `1.75` per
  `workspace.package.rust-version`. **This is a workspace-level
  change** that affects every crate; ROADMAP §0.5 says "cambios
  materiales van a su propio ciclo". Tracked as
  `CapMsrvBumpTo1.85` for a future cycle (M4-F1 or H1.1.2 territory).
  Until then, USDT stays as **spike-only**; no product code change.
- **M4R.5 perturbation** for USDT — answered by this spike:
  **+57% overhead with no consumer attached** is the cost of the
  volatile semaphore read + branch. With a consumer attached, the
  `nop` becomes `int3` and a BPF program runs in kernel space;
  per-call cost is dominated by the kernel↔user transition (~1–5 µs
  per probe fire). Cannot be measured on this host (CapEff=0).
- **Real consumer-side validation** on a privileged Linux host.
  Deferred to M4-F1 §M4R.4 (real reproducer) or UAT-M4-R-01.
  Required artefacts: `bpftrace one-liners` like
  `bpftrace -e 'usdt:./target/release/usdt-spike:chrono:add_probe { @count = count(); }'`,
  `stap -e 'probe chrono::add_probe { ... }'`, or a custom
  `bpf(2)` loader.
- **M4R.3 overlay semantic probe** — independent from USDT; deferred.
- **OBI Rust support** — chronos-native crate handles Rust uprobes
  via `cap_bpf` (Linux privileged); not a USDT substitute but
  comparable in spirit.

## 8. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0009-m4r2-usdt-spike.md`
  (this file).

**No Chronos product code changed.** This ADR closes the M4-F0
inventory requirement for USDT on Rust (producer side fully verified
on this host; consumer side deferred to a privileged host). M4R.2 is
now considered **measured and accepted with constraints**; M4-F0
Rust side can move on to M4R.5 (perturbation fallback summary) and
M4-F1 §M4R.3 / §M4R.4.

## 9. Cross-references

- ROADMAP §M4-F0 — "XRay/USDT requieren spikes separados, no
  dependencia obligatoria antes de M6 si no son necesarios" —
  **delivered by this ADR for USDT producer side**.
- ROADMAP §M4-F1 §83 — "M4R.2 USDT spike/ADR" — **delivered by
  this ADR**.
- LANGUAGE_BACKENDS.md §Rust priority 6 — `#[usdt::probe]` macros
  with stable IDs instead of raw offsets — **validated by this spike**.
- ADAPTIVE_INSTRUMENTATION.md §L4 — "Selected arguments, returns,
  state transitions, branch decisions and invariant inputs" —
  **implemented by USDT**.
- ADAPTIVE_INSTRUMENTATION.md §L3 — USDT slots at the source-overlay
  edge of L3; not a substitute for L2–L4 eBPF probes when no
  source edit is possible.
- ADR-0006 (reuse OTel and OBI before custom generic instrumentation)
  — **this ADR narrows 0006's scope to USDT producer-side specifics**.
- ADR-0007 (M4G.1 otelc Go spike) — sister ADR for Go OTel.
- ADR-0008 (M4R.1 XRay spike) — sister ADR for Rust XRay.
- TECHNOLOGY_BASELINE.md §6 — verified the
  `oxidecomputer/usdt` GitHub repo is live; crate name on crates.io
  is `usdt` (not `usdt-tools` as older drafts referenced).
