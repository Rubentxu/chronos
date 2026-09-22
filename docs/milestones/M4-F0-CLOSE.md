# M4-F0 (chapter) — Prerrequisitos técnicos para M6 close report

**Status:** Closed 2026-09-22
**Cycle:** M4-F0 (foundational: Go + Rust OTel/prerequisite inventory)
**Exit criterion:** *Inventory OBI/Auto SDK supported + capture context/IDs + version of mechanisms (Go side + Rust side); spikes para XRay + USDT con empirical measurements; documented ADR per mechanism + fallback policy.*
**Verdict:** ✅ Satisfied (3/3 sub-cycles executed + 1 perturbation policy).

This report audits the M4-F0 chapter at the close of sub-cycles M4G.1 + M4R.1 + M4R.2 + M4R.5, records the achieved state, and documents the deferred items per honest scope (ADR-0004 §2.2).

---

## 1. Exit criterion verdict

ROADMAP §M4-F0 framed the chapter around two inventory requirements:

| Sub-cycle | ROADMAP scope | ADR | Status |
|---|---|---|---|
| M4G.1 | Go OTel: OBI/Auto SDK + otelc spike + compile isolated | ADR-0007 (293L) | verified |
| M4R.1 | Rust XRay nightly spike + measurements | ADR-0008 (332L) | verified |
| M4R.2 | Rust USDT producer-side spike + measurements | ADR-0009 (441L) | verified |
| M4R.5 | Perturbation detected + fallback policy (synthesis of M4R.1 + M4R.2) | ADR-0010 (245L) | verified |

**Verdict:** satisfied. Four cycles (M4G.1 + M4R.1 + M4R.2 + M4R.5) shipped every inventory requirement. The M4-F0 layer is end-to-end: Go `otelc v1.1.0` spike with version-coupling constraint (pin app to otel v1.45.0, +0.4% wall-clock, +26.3% binary size, 16 OTLP/HTTP spans per batch); Rust `-Z instrument-xray` nightly spike with build.rs manual link of clang_rt.xray (+18% sleds-only, +650× patching — operator opt-in only); Rust `usdt = "0.6"` producer-side spike with readelf NT_STAPSDT verification (+57% overhead without consumer); and the 3-tier perturbation ladder T0/T1/T3/T4 with budgets and one-way deterministic fallback.

## 2. Achieved state

### 2.1 Go OTel inventory (M4G.1)

- **otelc v1.1.0** linux-amd64 downloaded from GitHub releases (sha256 `009fcbcaf05366a981ee1d282935e3a18eab0b9d3bc7507fd02ea67c3afa46fc`, released 2026-08-24).
- 50-line Go HTTP server using `go.opentelemetry.io/otel v1.45.0` + `otlptracehttp` exporter.
- **End-to-end real**: 16 spans in 1 `ExportTraceServiceRequest` (8 manual `chronos-spike` + 8 auto `go.opentelemetry.io/otelc/instrumentation/net/http`).
- **Measurements**: binary size 19.82 MB → 25.03 MB (+5.21 MB / +26.3%); wall-clock 150 reqs 3362.9 → 3374.8 ms (+0.4%); throughput 45 → 44 req/s.
- **Constraint**: pin app to otel v1.45.0 (otelc v1.1.0 hardcodes v1.45.0 in `.otelc-build/instrumentation/go.opentelemetry.io/otel/go.mod`); only `otelc go {build,install,test}` subcommands supported.
- **OBI v0.13.0** Go support confirmed (requires Linux privileged + CAP_BPF/CAP_SYS_ADMIN — CapEff=0 → not executable now, deferred to M4-F1 §M4G.3 / UAT-M4G-01).
- **Auto SDK** legacy (`opentelemetry-go-instrumentation`) merged into OBI for net/HTTP+gRPC; use OBI, not standalone.
- `.otelc-build/` added to `.gitignore` (per-tree cache).

### 2.2 Rust XRay inventory (M4R.1)

- **rustc nightly `1.99.0-nightly`** (`rustup toolchain list` confirmed) with `-Z instrument-xray`.
- **clang 23.1.0** with `libclang_rt.xray-{basic,fdr,x86_64,x86_64}-x86_64.a` available.
- **NO `perf`** in this host (`which perf` empty) — relevant for `xray_mode=xray-profiling`.
- 13-line Rust workload with `#[inline(never)]` (without it optimiser eliminated calls).
- `build.rs` links manually `static:+whole-archive=clang_rt.xray-{basic,fdr,x86_64,x86_64}` + `-ldylib={pthread,dl,stdc++}`.
- **Measurements** (kernel 7.2.4, Xeon E5-2682 v4): baseline 34 ms; XRay sleds-only 40 ms (+18%); XRay+patching 22 200 ms (+650× — operator opt-in only). Binary size +0.01 MB. `xray_instr_map` 864 B + `xray_fn_idx` 208 B. Trace log for `workload(10M)` = 602 336 B (~60 B/inner call).
- **End-to-end real verified**: `llvm-xray account` reports 6 functions with histograms; `llvm-xray graph` produces hierarchy `main → workload → add/mul`.
- **Constraint**: pin nightly (MSRV 1.75 incompatible); manual link via build.rs (RUSTFLAGS alone don't link runtime); patching mode = +650× → debug builds opt-in only; `xray_logfile` env var silently ignored (always writes `xray-log.<exe>.<hash>` in CWD); no `perf` (basic/fdr don't need it); no `cargo test` integration.

### 2.3 Rust USDT inventory (M4R.2)

- **`usdt = "0.6"`** (Apache-2.0, rust-version 1.85.0) from crates.io. NO separate `usdt-tools` — codegen lives inside `usdt` via `usdt::Builder`.
- Provider `src/chrono.d` with 4 probes (D-syntax standard SystemTap/DTrace).
- `build.rs` with `Builder::new("src/chrono.d").build()`.
- **Pre-flight consumer side**: NO `perf`, NO `bpftrace`, NO `stap`, NO access `/sys/kernel/debug/tracing` (CapEff=0). `dtrace` present but limited to D-script compilation.
- **End-to-end real verified**: `readelf -n` reveals 4 `NT_STAPSDT` entries with provider `chrono`, name, location PC, base, semaphore, argument ABI `8@%rdi 8@%rsi 8@%rdx`. `objdump -d` confirms each probe PC falls on a `0x90` nop instruction.
- **Sections ELF-specific**: `.probes` 8 B (4 × 2-byte semaphores), `.note.stapsdt` 324 B (4 × 81 B), `.stapsdt.base` 1 B.
- **Measurements**: baseline 24.3 ms; USDT 4 probes 38.2 ms (+57% overhead without consumer attached). Binary size +4% (0.331 → 0.345 MB).
- **Constraint**: `usdt = "0.6"` MSRV 1.85 — workspace currently 1.75, **adoption requires MSRV bump** (tracked as `CapMsrvBumpTo1.85` future cycle per ROADMAP §0.5); closure args form mandatory (`|| args`); consumer side requires `CAP_SYS_ADMIN`/`CAP_BPF` + kernel headers (host with CapEff=0 not executable); producer metadata visible statically without privileges.

### 2.4 Perturbation ladder (M4R.5)

- **3-tier ladder** (ADR-0010 §2) with budgets measurable:
  - T0 Zero (0% baseline).
  - T1 USDT producer-only ≤ +60% wall-clock (measured +57% in this host).
  - T2 USDT consumer-attached ≤ +5× wall-clock (estimated, not measured CapEff=0).
  - T3 XRay sleds-only ≤ +25% wall-clock (measured +18%).
  - T4 XRay patching-active = **OPERATOR OPT-IN ONLY** (measured +650×).
- **Fallback policy** (ADR §2.3): drop one tier + re-measure; if T1 exceeds budget → report `unsupported` per ADR-0004 + offer coarse-only fallback; chain **deterministic and one-way** — never auto-promote T1→T2 or T2→T3 without operator opt-in.
- **Operational rules** (ADR §5): always start at T1 when target has USDT probes; promote T2 only on operator "attach consumer" request (with logging); T3 requires separate debug build (`-Z instrument-xray=always` + clang_rt.xray link); T4 reserved for opt-in debug sessions with fixed iteration budget; T0 fallback disables probe firing via semaphore (`__usdt_sema_chrono_* = 0`) — probe metadata preserved.
- **Synthesis table** (ADR §2.2): T0 24.3/34 ms; T1 38.2 ms; T3 40 ms; T4 22 200 ms. SHA-256 of binaries preserved in ADR §4.

## 3. Source of truth

| Item | Path | Notes |
|---|---|---|
| ADR-0007..0010 | `docs/chronos-agentic-reconstruction/docs/adr/0007-m4g.1-otelc-spike.md` .. `0010-m4r.5-perturbation-summary.md` | 4 ADRs, 245..441 lines each |
| Spike source code | per sub-cycle off-scratch + durable paths (`/home/rubentxu/m4-spikes/` or `/home/rubentxu/.jcode/scratch/m4g*-*`) | binaries SHA-256 preserved |
| Source SHA-256 | per sub-cycle `evidence/binary-shas.txt` or ADR §5 | preserved bit-exact |

## 4. Orphan rule

M4-F0 spikes (Go + Rust OTel inventory) are foundation-level — they do NOT modify Chronos product code. They establish empirical baselines for the M4-F1 sub-cycles to consume (M4G.3 Go checkout-bug + M4R.3 overlay + M4R.4 Rust state-corruption).

## 5. Honest limitations (ADR-0004 §2.2 No Silent Lies)

The M4-F0 chapter is **closed for inventory scope** but the following items are explicitly out-of-scope per ADR-0007..0010:

- **Consumer-side USDT validation** in a privileged host (this host CapEff=0; deferred to M4-F1 §M4R.4 / UAT-M4-R-01).
- **MSRV bump 1.75 → 1.85** (USDT requires; tracked as `CapMsrvBumpTo1.85` future cycle).
- **Cross-arch (aarch64)** re-measurement (X86 only in this spike).
- **Cold-start latency** impact (`CapColdStartLatency` follow-up).
- **Memory-pressure modelling** (`CapReplayBound` follow-up).
- **Per-probe argument evaluation cost** (T2 estimation only).
- **OBI v0.13.0 Go execution** (Linux privileged required).
- **Productionization of probes** into `chronos-native` or `chronos-go` (M1+).

## 6. Verification chain

| Step | Command | Result |
|---|---|---|
| SHAs preserved | `git cat-file -e 42fc8ea3 2284be18 031dd6ed 996f8c71` | exit=0 (4 SHAs OK) |
| Workspace T0 | `cargo fmt --all -- --check` | exit=0 |
| Workspace T0 | `cargo clippy -p chronos-mcp --tests --no-deps -- -D warnings` | exit=0 |
| Workspace T1 subset | `cargo test -p chronos-mcp --test server_cohesion` | 9/9 PASS |
| CC#4 | 102 manifests clean | preserved |
| Cargo.lock sha256 | `68ee81f284bf115529c7060e80d6e2193e109f956d51dc8767a69d48febb0e4f` | unchanged |
| v0.7.112 tag | peels `0be2ec2d` | intact |

## 7. Chapter close tag

`m4-f0-prerequisites.0` (annotated, NOT GPG-signed per env limitation per ADR-0004 §2.2), peels `42fc8ea35c91df283699baa08d1a56d39e9046a8` (the M4R.5 merge commit — last sub-cycle of M4-F0).

## 8. Next autonomous work

After M4-F0 chapter close, the natural next step is **M4-F1** (ROADMAP §M4-F1 §83-84, 8 sub-cycles: M4G.1, M4G.2, M4G.3, M4R.1, M4R.2, M4R.3, M4R.4, M4R.5). M4-F1 is **already executed (8/8 verified)** as documented in `docs/milestones/M4-F1-CLOSE.md` (peer document).
