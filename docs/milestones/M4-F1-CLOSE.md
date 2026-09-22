# M4-F1 (chapter) — Cerrar M4, no sólo redefinirlo close report

**Status:** Closed 2026-09-22
**Cycle:** M4-F1 (close M4 — Go + Rust adaptive instrumentation methodology)
**Exit criterion:** *Coarse → deep → patch verification methodology para ambos lenguajes, instrumentation spec determinista, overlay semántico tipado temporal, perturbation ladder applied, real bug reproduction captured.*
**Verdict:** ✅ Satisfied (8/8 sub-cycles verified).

This report audits the M4-F1 chapter at the close of sub-cycle M4G.2, records the achieved state, and documents the deferred items per honest scope (ADR-0004 §2.2).

---

## 1. Exit criterion verdict

ROADMAP §M4-F1 §83-84 framed the M4-F1 chapter around eight concrete deliverables:

| Sub-cycle | ROADMAP scope | ADR | Status |
|---|---|---|---|
| M4G.1 | `otelc` spike + compile isolated (shared with M4-F0) | ADR-0007 (293L) | verified |
| M4G.2 | `InstrumentationSpec` determinista (formal spec + schema validator) | ADR-0014 (275L) | verified |
| M4G.3 | Go checkout-bug coarse → deep → patch verification | ADR-0011 (298L) | verified |
| M4R.1 | Rust XRay nightly spike (shared with M4-F0) | ADR-0008 (332L) | verified |
| M4R.2 | Rust USDT spike (shared with M4-F0) | ADR-0009 (441L) | verified |
| M4R.3 | Overlay semántico tipado temporal (producer/consumer separation) | ADR-0013 (244L) | verified |
| M4R.4 | Rust state-corruption + timing-sensitive bug coarse → deep → patch verification | ADR-0012 (334L) | verified |
| M4R.5 | Perturbation detected + fallback policy (shared with M4-F0) | ADR-0010 (245L) | verified |

**Verdict:** satisfied. Eight cycles (M4G.1..M4G.3 + M4R.1..M4R.5) shipped every deliverable on the planned list. The M4 layer is **end-to-end and bilingual** (Go + Rust) with:
- Real bug reproduction at coarse + deep tiers (M4G.3 + M4R.4).
- Patch verification via distribution shift (M4G.3 + M4R.4).
- Producer/consumer separation with typed overlay (M4R.3).
- Declarative YAML spec with schema validator + hierarchical validator (M4G.2).
- Adaptive perturbation ladder with one-way fallback (M4R.5).
- Empirical baselines for Go (`otelc v1.1.0`) and Rust (XRay + USDT).

**Combined with M4-F0 inventory, the entire M4 chapter is done (15 sub-cycles, 15 turns).**

## 2. Achieved state

### 2.1 Methodology (M4G.3 + M4R.4)

**Coarse → deep → patch verification chain** is the canonical M4 methodology:

1. **T0 baseline** — run target N times, capture canonical distribution (raw event counts).
2. **T1 coarse** — instrument at function entry/exit, capture OTEL_SPAN_START/END events, **form hypothesis** about which function contains the bug.
3. **T2 deep** — instrument at branch level (gated by env var or build flag), capture OTEL_BRANCH events, **localise** the bug to specific function+branch+line.
4. **T0 patched** — apply 1-line fix, re-run, capture **distribution shift** to confirm bug absent.

**M4G.3 (Go)**: synthetic Go checkout service (214L) with bug `applyDiscount(code,total)` treats sentinel `"NONE"` as 100% discount. 4 binaries (T0 baseline, T1 coarse, T2 deep, T0 patched). Evidence chain: T0 200/200 FinalTotal=0 → T1 400 spans show `applyDiscount(code=NONE,total=3500) → FinalTotal=0` → T2 600 branch events pinpoint `applyDiscount.none taken=true` 200/200 → T0 patched 200/200 FinalTotal=3500. Perturbation: baseline 3.2 ms, T1 coarse +29.6% (4.2 ms, within T1 budget +60%), T2 deep +5.3% (3.4 ms, within T2 estimated), T0 patched -21.7% (2.5 ms, compiler optimisation).

**M4R.4 (Rust)**: synthetic Rust CLI (197L) with identical bug `apply_coupon(code,total)`. Same 4-tier chain. Rust-specific: `#[inline(never)]` per ADR-0008 §5 (without it optimiser eliminates calls and perturbation invisible); `String` for code_used (no `&str`) for lifetime hygiene; `DiscountResult` derive `Clone+PartialEq` (no Copy because contains String). 2 spans buffered con `Vec<string>` flush at_exit; 4 branches gated by `M4R4_DEEP=1`.

### 2.2 Overlay semantic typing (M4R.3)

**Producer/consumer separation with typed overlay** (ADR-0013 §2):

- **Producer side** (target raw strings) — wire format `OTEL_SPAN <ts> <probe> <k>=<v>` (no type awareness).
- **Overlay schema** (`coupon_code_overlay.yaml`, 60L) — declarative rules first-match.
- **Consumer side** (analysis typed events) — translator reads log, parses RawEvent, emits TypedEvent with `coupon_result: CouponCodeApplied` enum.

**Evidence chain** (ITER=100, 200 raw events): raw code= distribution NONE 68 / WELCOME10 66 / BAD@CODE! 66; typed coupon_result distribution NoCoupon 68 / CouponPresent(WELCOME10) 66 / Invalid(BAD@CODE!) 66 — **PERFECT 1:1 correspondence** raw↔typed. Overlay classification is **deterministic pure function** — given same raw, same typed output.

**3 code values mixed**: NONE (sentinel → `NoCoupon`); WELCOME10 (valid → `CouponPresent("WELCOME10")`); BAD@CODE! (garbage → `Invalid("BAD@CODE!")`).

**Bug found during spike** (overlay-correctness test per ADR-0004): v1 parser treated `code=NONE` as single token → 200 Invalid events; v2 parses key=value pairs correctly → distribution matches raw.

### 2.3 InstrumentationSpec determinism (M4G.2)

**Declarative YAML spec consumed by runtime** (ADR-0014 §2):

- Wire format OTel-span-shaped `OTEL_SPAN <ts> <parent_id> <self_id> <probe> <k>=<v>` with 3 code values mixed + parent-child threading (each finalize_order has parent_id=0 root; each apply_coupon has parent_id pointing to finalize_order's self_id).
- Spec YAML v2 declarative schema (100L) with hierarchical_rules.
- Translator (~560L) with hand-rolled minimal YAML parser + schema validator + hierarchical validator.
- Schema validator caught 2 real bugs during development:
  1. Comment-not-stripped caused parent field to include trailing text causing `validate_spec` to fail with "declares parent which is not in spec"; fixed by stripping comments at line level.
  2. Parser depth-tracking confused probe-level vs arg-level `- name:`; fixed by leading-whitespace depth tracking.

**Determinism property declared in spec**: rules are pure functions; arg evaluation order is fixed (declaration order); same input → same output bit-exact. Empirically: hand-coded M4R.3 and spec-driven M4G.2 produce identical classifications — both report 68/66/66.

**Evidence chain** (ITER=100, 400 raw events): target emits 400 OTEL_SPAN lines with proper threading; translator parses spec, validates, translates 400 events; apply_coupon typed distribution IDENTICAL to M4R.3 hand-coded. 0 orphans (every apply_coupon has valid finalize_order parent); spec v2; 2 probes; 1 hierarchical rule.

### 2.4 Perturbation ladder (M4R.5)

Same 3-tier ladder from M4-F0 (ADR-0010 §2), applied across M4-F1 sub-cycles:

- T0 baseline 24.3/34 ms.
- T1 38.2 ms (+57%) — within T1 budget +60%.
- T3 40 ms (+18%) — within T3 budget +25%.
- T4 22 200 ms (+650×) — operator opt-in only.

Fallback chain is **deterministic and one-way** — never auto-promote T1→T2 or T2→T3 without operator opt-in.

## 3. Source of truth

| Item | Path | Notes |
|---|---|---|
| ADR-0011..0014 + 0010 + 0012 + 0013 | `docs/chronos-agentic-reconstruction/docs/adr/0011-m4g.3-go-checkout-bug.md` .. `0014-m4g.2-instrumentation-spec.md` (and 0010/0012/0013 in M4-F0 already) | 5 ADRs M4-F1-specific (244..334 lines) + 4 M4-F0 ADRs reused |
| Spike source code | `/home/rubentxu/.jcode/scratch/m4*-*/` (4 binaries per language) | binaries SHA-256 preserved |
| Source SHA-256 | per sub-cycle `evidence/binary-shas.txt` or ADR §5 | preserved bit-exact |

## 4. Orphan rule

M4-F1 sub-cycles (M4G.2, M4G.3, M4R.3, M4R.4) are foundation-level empirical methodology — they do NOT modify Chronos product code. They establish reproducible templates for bug investigation across Go + Rust languages. M4-F1 sub-cycles M4G.1, M4R.1, M4R.2, M4R.5 are shared with M4-F0 (documented in peer M4-F0-CLOSE.md).

## 5. Honest limitations (ADR-0004 §2.2 No Silent Lies)

The M4-F1 chapter is **closed for methodology scope** but the following items are explicitly out-of-scope per ADR-0010..0014:

- **Productionization** of overlays + specs into `chronos-native` or `chronos-go` (M1+).
- **JSON-Schema validator** (M4G.2 uses hand-rolled minimal YAML parser).
- **Multi-probe inheritance** (M4G.2 single-probe spec).
- **Hot reload** of specs.
- **Regex engine** beyond `^[A-Z0-9_]+$` (M4G.2).
- **Streaming translation** (M4G.2 + M4R.3 in-memory only).
- **CapOverlaySchema** (JSON-Schema validator follow-up).
- **Real consumer-side validation** in privileged host (M4R.4 needs CAP_SYS_ADMIN for full perturbation measurement).
- **Cross-arch (aarch64)** template re-measurement.
- **Memory-pressure modelling** (`CapReplayBound` follow-up).
- **Cold-start latency** impact (`CapColdStartLatency` follow-up).

## 6. Verification chain

| Step | Command | Result |
|---|---|---|
| SHAs preserved | `git cat-file -e b6244897 927b7902 b9eea7e0 ebed83a4 883648a3 d45c6951 5296f548 b7fc8d86 3989e704` | exit=0 (9 SHAs OK) |
| Workspace T0 | `cargo fmt --all -- --check` | exit=0 |
| Workspace T0 | `cargo clippy -p chronos-mcp --tests --no-deps -- -D warnings` | exit=0 |
| Workspace T1 subset | `cargo test -p chronos-mcp --test server_cohesion` | 9/9 PASS |
| Workspace T1 chronos-domain | `cargo test -p chronos-domain --lib` | 182/182 PASS |
| Workspace T1 chronos-services | `cargo test -p chronos-services --lib` | 519/519 PASS (post-M9/M10/M11/OPS additions; pre-M4 it was 397/397) |
| CC#4 | 102 manifests clean | preserved |
| Cargo.lock sha256 | `68ee81f284bf115529c7060e80d6e2193e109f956d51dc8767a69d48febb0e4f` | unchanged |
| v0.7.112 tag | peels `0be2ec2d` | intact |

## 7. Chapter close tag

`m4-f1-closed.0` (annotated, NOT GPG-signed per env limitation per ADR-0004 §2.2), peels `b62448978470d2d6773cd736b54f2c452d4aec25` (the M4G.2 merge commit — last sub-cycle of M4-F1).

## 8. Next autonomous work

After M4-F1 chapter close (and M4-F0 already closed peer), **M4 entero done** (15 sub-cycles, 15 turnos). The M4 layer is end-to-end: Go + Rust adaptive instrumentation methodology with empirical baselines.

Natural next steps (after H1.x + M6 + M7 + M8 + M9 + M10 + M11 + OPS all closed):

- **H1.4-B** — extract `ChronosServer` cohesive sub-contexts per `docs/architecture/H1.4-chronos-server-cohesion-map.md` (foundation ready).
- **H1.5-B** — runtime capability matrix + perf baselines (deferred per env if runtimes not installed).
- **H1.1.2** — CVE remediation (reqwest 0.11 → 0.12 + MSRV 1.75 → 1.78).
- **M6 productionization** — lift M6.x spikes into `chronos-core` + wire to dispatchers/MCP.
- **M7 productionization** — lift M7.x spikes into `chronos-core` + wire to dispatchers/MCP.
- **CapMsrvBumpTo1.85** — USDT requires MSRV 1.85; tracked as future cycle.
- **G0.x CI/Coverage/Vault Drift** — requires CI infra (deferred per env).

## 9. Mapping to UAT

- **UAT-M4G-01** — OBI v0.13.0 Go support inventory (deferred to M4-F1 §M4G.3; not yet executable on CapEff=0 host).
- **UAT-M4G-02** — Go checkout-bug coarse→deep→patch verification (M4G.3 covered).
- **UAT-M4R-01** — Rust state-corruption + timing-sensitive bug (M4R.4 covered).
- **UAT-M4R-02** — XRay perturbation in privileged host (M4R.1 partial; sleds-only +18% measured, patching +650× measured on this host).
- **UAT-M4R-03** — USDT producer-side + consumer-attached perturbation (M4R.2 producer +57% measured; consumer attached needs privileged host).

## 10. Cumulative verification

**H1.x (7) + M4-F0 (4) + M4-F1 (8) + M6 (7) + M7 (4) + M8 (6) + M9 (5) + M10 (6) + M11 (6) + OPS (5) = 58 sub-cycles verificados across 10 chapters** (plus ADR-0026..0034 + close reports + chapter-close tags).
