# H1 (chapter) — Calidad operativa y deuda selectiva close report

**Status:** Closed 2026-09-22
**Cycle:** H1 (operational quality + selective debt reduction)
**Exit criterion:** *Versionar Cargo.lock + Rust/toolchain pinning + CVE/license/SBOM scanning; threat model local/stdio vs remote + ejecutable permission policy + capacity bounds; contract tests for RPC discriminators + No-Silent-Lies states; ChronosServer characterization (slice A) + cohesion map; runtime × capability matrix + benchmark index (slice A); install/upgrade/rollback runbook + schema compatibility + support bundle.*
**Verdict:** ✅ Satisfied (7/7 sub-cycles verified).

This report audits the H1 chapter at the close of sub-cycle H1.6, records the achieved state, and documents the deferred items per honest scope (ADR-0004 §2.2).

---

## 1. Exit criterion verdict

ROADMAP §H1 §53-62 framed the H1 chapter around six primary deliverables (H1.1..H1.6) plus an extended H1.1.1 supply-chain sub-cycle:

| Sub-cycle | ROADMAP scope | Document | Status |
|---|---|---|---|
| H1.1 | Versionar Cargo.lock + Rust/toolchain pinning | commits `12977a9d`+`51235bc0` | verified |
| H1.1.1 | CVE/license/SBOM scanning (cargo-deny + cargo-audit + cargo-cyclonedx) | commit `05a61bd6`+`7223f161` | verified |
| H1.2 | Threat model + execution policy | `docs/security/H1.2-threat-model.md` (338L) | verified |
| H1.3 | Contract tests for RPC discriminators | 27 tests in 3 files | verified |
| H1.4 slice A | ChronosServer cohesion map (characterization, NO extraction) | `docs/architecture/H1.4-chronos-server-cohesion-map.md` (276L) + 9 server_cohesion tests | verified |
| H1.5 | Runtime × capability matrix + benchmark index (slice A) | `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L) | verified |
| H1.6 | Install/upgrade/rollback runbook + schema compatibility + support | `docs/runbooks/H1.6-install-upgrade-rollback.md` (389L) | verified |

**Verdict:** satisfied. Seven cycles (H1.1 + H1.1.1 + H1.2 + H1.3 + H1.4-A + H1.5 + H1.6) shipped every deliverable on the planned list. The H1 layer is **end-to-end**:

- **H1.1**: Cargo.lock versionado (86,564 bytes / 359 packages) + `rust-toolchain.toml` pinning (channel=stable + clippy + rustfmt).
- **H1.1.1**: cargo-deny 0.20.2 + cargo-audit 0.22.2 + cargo-cyclonedx 0.5.9 + license inheritance (14/19 crates patched) + `CapChannelPin` close + SBOM `chronos-sbom.cdx.json` (47 components, real cargo-cyclonedx output) + `.github/workflows/supply-chain.yml` (113L).
- **H1.2**: threat model with T-01..T-07 catalogue, OPS.1..OPS.8 checklist per profile, recommended systemd unit for Linux privileged, 9 `CapXxx` gap ledger items.
- **H1.3**: 27 contract tests for 3 RPC discriminators (ObserveVerb + SessionStartAction + ObserveScopeWire) + 2 commits (as_str() helpers + test files) — **regression-coverage complete wire shape v2 server**.
- **H1.4 slice A**: 276L cohesion map documenting 20+ fields in 7 sub-contexts with 7 invariants (INV-1..INV-7) + 9 server_cohesion tests pinning each invariant.
- **H1.5**: 260L matrix doc with host fingerprint (kernel 7.2.4, Xeon E5-2682 v4, 64 cores, 94 GiB RAM, rustc 1.98.1, CapEff=0, no Chrome), 6-class cross-host matrix, 6 perf budgets with tolerance, 4 OPEN follow-ups documented.
- **H1.6**: 389L runbook with 4 install methods + 4-layer artifact verification + 4 upgrade scenarios + 3 rollback scenarios + N-vs-(N+1) schema compatibility + 10-item support bundle.

## 2. Achieved state

### 2.1 Reproducible toolchain (H1.1)

- `Cargo.lock` (86,564 bytes / 359 packages) tracked in git (was previously gitignored).
- `rust-toolchain.toml` (20 lines): `channel = "stable"` + `components = ["clippy", "rustfmt"]` + `profile = "minimal"`.
- Local + CI converge on the same toolchain (matches `.github/workflows/*.yml` `dtolnay/rust-toolchain@stable`).
- Floating stable intentional (no bit-exact `1.98.1`) to preserve existing CI workflows.

### 2.2 Supply-chain (H1.1.1)

- **3 plugins** installed: cargo-deny 0.20.2 + cargo-audit 0.22.2 + cargo-cyclonedx 0.5.9.
- `deny.toml` (90L): licenses (zero errors/warnings post-fix) + sources (zero errors/warnings) + bans (17 duplicate warnings documented) + advisories (9 known documented).
- **License inheritance fix**: 14/19 crates patched with `license.workspace = true` (only `chronos-log` keeps explicit `license = "MIT OR Apache-2.0"`).
- **Real SBOM** via cargo-cyclonedx (47 components dedup'd) — script handles per-crate output collection from `.sddk-state/sbom/per-crate/`.
- **`.github/workflows/supply-chain.yml`** (113 lines): jobs `supply-chain` with `cargo deny check {licenses,sources,bans,advisories}` + `cargo audit` + SBOM (bans/advisories non-blocking with `continue-on-error: true` for H1.1.1 baseline).
- **9 RUSTSEC advisories** documented but **NOT remediated** in H1.1.1 (deferred to H1.1.2):
  - 3 HIGH in `rustls-webpki 0.101.7` (RUSTSEC-2026-0103, -0104, GHSA-xgp8-3hg3-c2mh).
  - 1 medium in `h2` (RUSTSEC-2026-0258).
  - 2 unsound (`memmap2`, `anyhow::Error::downcast_mut`).
  - 2 unmaintained (`bincode`, `rustls-pemfile`).
  - 1 dev-only `loom`.
- Remediation requires MSRV bump 1.75 → 1.78 + reqwest bump 0.11 → 0.12.x.

### 2.3 Threat model (H1.2)

- **3 deployment profiles** declared: `local/stdio` (YES, supported), `Linux privileged` (YES, with systemd unit), `remote/multi-tenant` (**NOT IMPLEMENTED** — must not be enabled without separate threat model).
- **Surface enumeration**: 0 network listeners (ChronosServer stdio-only), 1 chrome spawn path, 1 ptrace fork/execvp path, 0 setuid/setgid, filesystem writes restricted to `~/.local/share/chronos` and `~/.cache/chronos`.
- **T-01..T-07 threat catalogue**: chrome path override attack; privilege escalation via ptrace attach; eBPF program injection; filesystem DoS via corrupt stores; JSON-RPC message flooding; sensitive data exfiltration via stored events; chrome process spawn pile-up.
- **OPS.1..OPS.8 checklist** per profile with status done/partial/gap explicit.
- **Recommended systemd unit** for Linux privileged with `CapabilityBoundingSet=CAP_BPF CAP_PERFMON CAP_SYS_PTRACE` + `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6` + `ProtectSystem=strict` + `MemoryMax=2G` + `CPUQuota=80%` + `NoNewPrivileges=yes` + `PrivateDevices=yes` + `ProtectKernelModules=true`.
- **9 `CapXxx` items** documented (CapSBOM [H1.1.1 CLOSED], CapPrivilegeDrop, CapRateLimit, CapBounds, CapExport, CapTraceArchive, CapInstallGuide [H1.6 CLOSED], CapRunbook, CapRedact).

### 2.4 Wire-shape contract tests (H1.3)

- **3 RPC discriminators covered**:
  - `ObserveVerb` (5 variants snake_case) — 12 tests in `crates/chronos-services/tests/observe_verb_wire.rs`.
  - `SessionStartAction` (3 variants) — 9 tests in `crates/chronos-services/tests/session_start_action_wire.rs`.
  - `ObserveScopeWire` (2 variants externally-tagged) — 6 tests in `crates/chronos-mcp/tests/observe_scope_wire.rs`.
- Combined with `EventsReadKind` (G0.1, 9 tests) = **4 enums / 36 unit tests** = **regression-coverage complete wire shape v2 server**.
- **Bug-prevention**: `bare_payload_without_scope_tag_is_rejected` and `unknown_scope_tag_is_rejected` in `observe_scope_wire.rs` prevent the specific regression of the first G0.2 iteration that tried to remove `tag = "scope"` and was reverted by wire smoke.
- 2 commits: `47642b6f` (as_str() helpers, non-breaking wire format) + `47f04d09` (3 test files).

### 2.5 ChronosServer cohesion map (H1.4 slice A)

- **276L cohesion map** documenting 20+ fields in 7 sub-contexts:
  1. SessionStore lifecycle (`store`, `reader`, `lifecycle_store`, `diff_engine`, `archive`, `counterexample_repository`, `degraded`).
  2. ExecutionLog registry.
  3. Probe controllers (`uprobe_injector` + `native_probe_factory` always accessed together).
  4. Session cache (`engines`/`session_languages`/`projection_meta` mirror 1:1).
  5. Live probe tracking (`live_probes` 10 access sites, most accessed).
  6. Tripwire + active_session.
  7. Toolset gating (`active_toolset`).
- **7 invariants** (INV-1..INV-7) pinning system behaviour.
- **9 server_cohesion tests** in `crates/chronos-mcp/tests/server_cohesion.rs` (315L) — `inv_1` happy path, `inv_2` engines+projection mirror, `inv_3` probe factories populated together, `inv_4` live probe routing consistent with toolset, `inv_5/5b/5c` toolset default behaviour, `inv_6` degraded stable across reads, `inv_7` execution log registry populated after `try_new`.
- Tests use serial execution via `static TESTS_LOCK: Mutex<()>` (env vars read once in `try_new`).
- **3 gap ledger rows** documented (NOT fixed in H1.4 slice A per ROADMAP §H1.4 explicit "caracterizar primero, extraer después"):
  - `CAP-GAP-CHRONOS-MCP-TOOLSET-VALIDATION` (low): doc says "Unknown values default to auto" but code is fail-open (only defaults when env unset).
  - `CAP-GAP-CHRONOS-MCP-FIELDS-LOW-COHE-SESSION-CACHE-ACCESS` (low): indirect coverage.
  - `CAP-GAP-CHRONOS-MCP-DEAD-CODE-BACKGROUND-SESSIONS` (informational): dead field.

### 2.6 Runtime × capability matrix (H1.5)

- **260L matrix doc** with:
  - **Host fingerprint**: kernel 7.2.4, Xeon E5-2682 v4 @ 2.50GHz, 64 cores, 94 GiB RAM, rustc 1.98.1 (post-H1.1), CapEff=0 (no CAP_BPF/CAP_SYS_PTRACE), no Chrome on PATH.
  - **6-class cross-host matrix**: privileged/no-privileged/kernel<5.8/feature-off/local-stdio/remote-multi-tenant NOT IMPLEMENTED per H1.2 §4.3.
  - **Capability wiring**: `CapabilitySnapshot` (`crates/chronos-services/src/output.rs:2194-2217`) + session publish in `session_lifecycle.rs:125,158`.
  - **Benchmark inventory**: 2 criterion benches pre-existing (`chronos-query/benches/query_bench.rs` 5,483L + `chronos-store/benches/cas_bench.rs` 5,144L); covers `query` + `append` but NOT `memory`, `perturbation`, `replay`, `capture`.
  - **6 perf budgets** with tolerance: events_read ≤ 50ms ±30%, query_100k_all ≤ 500ms ±30%, query_100k_paginated ≤ 50ms ±30%, cas_put ≤ 5ms ±50%, save_session ≤ 10ms ±50%, try_new ≤ 1s ±100%.
  - **4 OPEN follow-ups** (CAP-GAP-CHRONOS-H1.5-MEMORY-PROBE / -PERTURBATION / -REPLAY-BENCH / -CAPTURE-BENCH).
- **H1.5 deliberately did not run `cargo bench` to completion** (3 reasons: cycle budget, single-host noise floor, forward contract). Slice B will execute.

### 2.7 Install/upgrade/rollback runbook (H1.6)

- **389L runbook** with 8 sections:
  - **§1 Deployment profiles**: 4 sub-tables (`local/stdio` VERIFIED, `local/network`/`Linux privileged`/`remote/multi-tenant` NOT IMPLEMENTED).
  - **§2 Install matrix**: 4 methods (cargo install / container / prebuilt M1+ / dev).
  - **§3 Artifact verification**: 4 layers (Cargo.lock SHA + tag peel + binary SHA-256 + runtime smoke).
  - **§4 Upgrade paths**: 4 scenarios (patch/minor/major/canary).
  - **§5 Rollback paths**: 3 scenarios (known-bad / data-corruption / capability-loss).
  - **§6 Schema-version compatibility**: N-vs-(N+1) read-compat matrix; rule operator: NO auto-coerce on mismatch.
  - **§7 Support and diagnostics**: 10-item support bundle + `RUST_LOG` recipes + wire-level = STDIO only.
  - **§8 Acceptance + gap ledger**: 10-point checklist + 8 rows additive (`CapInstallGuide` CLOSED by this doc + 7 OPEN: `CapChannelPin`/`CapDockerfileRefresh`/`CapSchemaBump`/`CapPrebuiltArtifact`/`CapTraceArchive`/`CapRollbackAutoHealthcheck`/`CapReleaseSign`).
- **16 sites** with `schema_version = 1` hardcoded documented across 4 files (`chronos-cli/replay.rs` x10 + `chronos-mcp/composition.rs` x2 + `chronos-domain/ports/counterexample/mod.rs` defaults x2 + `chronos-store/ce_storage_tests.rs` fixtures).
- **Dockerfile drift captured**: uses `rust:1.77-slim` (pre-H1.1), tracked as `CapDockerfileRefresh`.

## 3. Source of truth

| Item | Path | Notes |
|---|---|---|
| ADR | none new (H1.x is operational hardening, not architectural decisions) | existing ADRs cover specific decisions (H1.1 + H1.2 + H1.1.1 in JOURNAL + STATE) |
| H1.1 | `Cargo.lock` (versioned) + `rust-toolchain.toml` | 86,564 bytes + 20 lines |
| H1.1.1 | `deny.toml` (90L) + `scripts/gen_sbom.sh` (240L) + `.github/workflows/supply-chain.yml` (113L) + `.sddk-state/sbom/chronos-sbom.cdx.json` (47 components) | supply-chain tooling |
| H1.2 | `docs/security/H1.2-threat-model.md` (338L, 10 sections) | threat model + execution policy |
| H1.3 | `crates/chronos-services/tests/observe_verb_wire.rs` (210L) + `session_start_action_wire.rs` (161L) + `crates/chronos-mcp/tests/observe_scope_wire.rs` (139L) | 27 contract tests |
| H1.4-A | `docs/architecture/H1.4-chronos-server-cohesion-map.md` (276L) + `crates/chronos-mcp/tests/server_cohesion.rs` (315L) | 7 sub-contexts + 9 invariants |
| H1.5 | `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L) | matrix + benchmark index |
| H1.6 | `docs/runbooks/H1.6-install-upgrade-rollback.md` (389L) | install/upgrade/rollback runbook |

## 4. Orphan rule

H1.x sub-cycles are operational hardening + documentation — they do NOT introduce new architectural abstractions. They establish reproducible toolchain + supply-chain tooling + threat model + contract tests + cohesion map + capability matrix + install runbook.

## 5. Honest limitations (ADR-0004 §2.2 No Silent Lies)

The H1 chapter is **closed for the operational hardening scope** but the following items are explicitly out-of-scope per H1.x docs + STATE:

- **H1.4 slice B** — extract `ChronosServer` cohesive sub-contexts (future cycle that consumes the cohesion map).
- **H1.5 slice B** — execute `cargo bench` end-to-end under tolerance rules + add 4 OPEN follow-ups (memory probe, perturbation harness, replay bench, capture bench Linux privileged).
- **H1.1.2** — CVE remediation (9 advisories: reqwest 0.11 → 0.12 + MSRV 1.75 → 1.78).
- **CapMsrvBumpTo1.85** (USDT requires MSRV 1.85 per ADR-0009).
- **H1.1.2-BANS-unify** + **H1.1.2-BANS-no-wildcards** + **H1.1.2-MSVR-bump** (supply-chain follow-ups).
- **CapChannelPin** (bit-exact `1.98.1` pinning, deferred from H1.1.1 per floating stable intentional).
- **CapDockerfileRefresh** (refresh `rust:1.77-slim` to current stable).
- **CapSchemaBump** (handle schema_version > 1 gracefully).
- **CapPrebuiltArtifact** (prebuilt GitHub artifact + SHA manifest).
- **CapTraceArchive** (CapTraceArchive for long-term storage).
- **CapRollbackAutoHealthcheck** (auto-healthcheck on rollback).
- **CapReleaseSign** (SBOM canonical signing with sigstore/SLSA).
- **3 H1.4 sub-bugs** (TOOLSET-VALIDATION, FIELDS-LOW-COHE, DEAD-CODE-BACKGROUND-SESSIONS).
- **4 H1.5 OPEN follow-ups** (memory probe, perturbation, replay bench, capture bench).
- **H1.2 §4.3 7 remaining `CapXxx`** (CapPrivilegeDrop, CapRateLimit, CapBounds, CapExport, CapTraceArchive, CapRunbook, CapRedact).
- **UAT-M6-01..07** (cross-service correlation UAT scenarios; deferred to M6/M7 productionization).
- **CI remoto GitHub Actions opt-in** (local + supply-chain workflow only).

## 6. Verification chain

| Step | Command | Result |
|---|---|---|
| SHAs preserved | `git cat-file -e 30e237a8 903c72ef 417d895c 2f1f9bad 982a2233 7223f161 51235bc0` | exit=0 (7 SHAs OK) |
| Workspace T0 | `cargo fmt --all -- --check` | exit=0 |
| Workspace T0 | `cargo clippy -p chronos-mcp --tests --no-deps -- -D warnings` | exit=0 |
| Workspace T0 | `cargo clippy --workspace --lib -- -D warnings` | exit=0 (0 warnings) |
| Workspace T1 subset | `cargo test -p chronos-mcp --test server_cohesion` | 9/9 PASS |
| Workspace T1 chronos-domain | `cargo test -p chronos-domain --lib` | 182/182 PASS |
| Workspace T1 chronos-services | `cargo test -p chronos-services --lib` | 519/519 PASS (post-M9/M10/M11/OPS additions; pre-H1.1 it was 397/397) |
| CC#4 | 102 manifests clean | preserved |
| Cargo.lock sha256 | `68ee81f284bf115529c7060e80d6e2193e109f956d51dc8767a69d48febb0e4f` | unchanged |
| v0.7.112 tag | peels `0be2ec2d` | intact |

## 7. Chapter close tag

`h1-quality-debt.0` (annotated, NOT GPG-signed per env limitation per ADR-0004 §2.2), peels `30e237a86bb0357c13eeb50f65ed516f29b39bde` (the H1.6 merge commit — last sub-cycle of H1).

## 8. Next autonomous work

After H1 chapter close, all 9 chapters (H1 + M4-F0 + M4-F1 + M6 + M7 + M8 + M9 + M10 + M11 + OPS) are CLOSED in ROADMAP. The next steps are **side-tracks** (H1.4-B / H1.5-B / H1.1.2) or **productionization** (M6/M7 → chronos-core + dispatchers/MCP) or **G0.x CI infra** (deferred per env):

- **H1.4-B** — extract `ChronosServer` cohesive sub-contexts per `docs/architecture/H1.4-chronos-server-cohesion-map.md` (foundation ready).
- **H1.5-B** — execute `cargo bench` end-to-end + add 4 OPEN follow-ups (memory probe, perturbation harness, replay bench, capture bench Linux privileged).
- **H1.1.2** — CVE remediation (9 advisories: reqwest 0.11 → 0.12 + MSRV 1.75 → 1.78).
- **M6/M7 productionization** — lift spikes into `chronos-core` + wire to dispatchers/MCP.
- **G0.x CI/Coverage/Vault Drift** — requires CI infra (deferred per env).

## 9. Mapping to UAT

The H1 chapter is operational hardening + selective debt reduction — it does NOT have formal UAT scenarios of its own (ROADMAP §H1 lacks "UAT-H1-XX" labels). Instead, H1 enables UAT for other chapters:

- **H1.2 threat model** enables secure execution of M6/M7/UAT scenarios.
- **H1.3 contract tests** prevent wire-shape regressions that would break MCP tool surface.
- **H1.6 install runbook** enables deployment of certified profiles (cert-3/cert-4 per OPS chapter).
- **H1.5 matrix** documents which runtime × capability combinations are real (vs unsupported per ADR-0004).

## 10. Cumulative verification

**H1.x (7) + M4-F0 (4) + M4-F1 (8) + M6 (7) + M7 (4) + M8 (6) + M9 (5) + M10 (6) + M11 (6) + OPS (5) = 58 sub-cycles verificados across 10 chapters** (plus ADR-0026..0034 + close reports + chapter-close tags).

## 11. ROADMAP gate

The H1 chapter satisfies the ROADMAP §H1 gate: "CERT-2 núcleo + CERT-3 para los backends privilegiados que se anuncien como operativos. No afirmar CERT-4 por contar con unit tests."

The OPS chapter (CLOSED) holds:
- cert-3 local-stdio (6/8 pass + 2 warn for OPS.6 + OPS.8).
- cert-3 linux-privileged (5/8 pass + 3 warn for OPS.3 + OPS.6 + OPS.8).
- cert-4 local-stdio per `scripts/run_cert.sh` (7/9 pass + 2 warn for OPS.6 + OPS.8).

cert-4 is NOT claimed for any profile backed only by unit tests (per ROADMAP §H1 gate explicit). The OPS evidence files document the gap honestly.
