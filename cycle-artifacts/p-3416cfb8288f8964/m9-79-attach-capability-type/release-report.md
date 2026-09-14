# Release Report — m9-79-attach-capability-type

## Subject

| Base | Head (verified) | Tag | Tag SHA | Tag peel | CWD | Verified at |
|---|---|---|---|---|---|---|
| `009b75037357d069775ad4d7a0661684e27fc9ca` | `f41abd4580d078a3f5f1255ffd543573a4fa702d` | `v0.7.81` | `c3c69f6abf1cd21c0dbe72e98128043cc95097cd` | `f41abd4580d078a3f5f1255ffd543573a4fa702d` | `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos` | 2026-09-14T06:28:00+02:00 |

## Summary

| Verdict | Mode | Path | Required scenarios | Commands passed | Critical | Warnings |
|---|---|---|---|---|---|---|
| **PASS** | normal | B-direct | 1 (REQ-ATTACH-CAPABILITY-001) | 5/5 | 0 | 0 |

## Files Inventory

| Bucket | Added | Modified | Deleted | Renamed |
|---|---:|---:|---:|---:|
| crates/ | 0 | 1 | 0 | 0 |
| chronos-sandbox/tests/ | 0 | 1 | 0 | 0 |
| docs/manual-ai/ | 0 | 2 | 0 | 0 |

Files (4 in `git diff main..HEAD`, sorted by path):

| Status | Bucket | Path | Change |
|---|---|---|---|
| modified | chronos-sandbox/tests/ | `chronos-sandbox/tests/session_lifecycle.rs` | line 376: assertion literal `ebpf_user` → `ptrace_attach` |
| modified | crates/ | `crates/chronos-services/src/session_lifecycle.rs` | line 198: capability snapshot `probe_type` literal `ebpf_user` → `ptrace_attach` |
| modified | docs/manual-ai/ | `docs/manual-ai/en/08-session-management.md` | attach example now states `probe_type: "ptrace_attach"` |
| modified | docs/manual-ai/ | `docs/manual-ai/es/08-gestion-sesiones.md` | Spanish mirror |

## Behavioral Compliance

| # | Requirement | Production Path | Test | Status | Evidence |
|---|---|---|---|---|---|
| REQ-ATTACH-CAPABILITY-001 | `ChronosSessionLifecycleService::attach` reports `probe_type: "ptrace_attach"` in its capability snapshot | `crates/chronos-services/src/session_lifecycle.rs:198` | `chronos-sandbox/tests/session_lifecycle.rs::test_session_start_attach_to_running_self` (line 376) | **COMPLIANT** | diff at session_lifecycle.rs:198; test passed 1/1 (15.26 s) |

## Production Readiness

| Gate | Status | Findings / N/A reason |
|---|---|---|
| Errors / recovery | PASS | The literal change is total; no fallback path that could regress |
| State / data integrity | PASS | `probe_type` is advisory; downstream readers tolerate the new value |
| Resource cleanup | N/A | No probe lifecycle change in this cycle |
| Concurrency | N/A | No new shared state |
| Migrations / compatibility | PASS | `CapabilitySnapshot::probe_type: Option<String>`; no schema/wire change |
| Security | N/A | No new input surface |
| Performance | PASS | No runtime cost difference between two equal-length string literals |
| Observability / deployability | PASS | Literal appears in JSON envelope; manual updated |

## Code Quality

| Standard | Status | Evidence |
|---|---|---|
| Business code reality (no stub / mock / hardcoded satisfier in changed `src/`) | PASS | One literal in the diff; grep for `TODO`, `FIXME`, `unimplemented!`, `todo!` in the changed files: zero hits |
| Documentation discipline (no issue/task/user/cycle refs in comments; or refs attached to a behavior explanation) | PASS | No new comments; only doc-prose updates in manuals |

## Cross-checks

- CC#4: artifact index SHA-256 consistency (regenerated to fixpoint by `scripts/regen_manifest_index_shas.py` after the artifacts commit lands)
- CC#12: `main_sha == head_sha == remote_tag_peel == f41abd4580d078a3f5f1255ffd543573a4fa702d` (verified on origin)
- CC#15: `apply-checkpoint.json` carries `created_at`, `title`, `summary`
- CC#21: `archive-manifest.md` carries `## Evidence bindings`
- CC#24 / CC#31: `## Cross-checks` present in this report, `verify-report.md` and `archive-manifest.md`
- CC#33: `## Subject` present in `change-entry.md`, `verify-report.md`, `verify-findings.json`
- CC#36 / CC#55 / CC#39: `verify-report.md` carries `Path`, `## Files Inventory`, and the canonical summary table
- CC#42: peel-match recorded in `release-receipt.md`
- CC#50: `apply-checkpoint.json` carries canonical `remote_tag` field
- CC#51: cycle-artifacts folder exists for the m9-79 row
- CC#54: bash meta-check passes
- CC#55: `verify-report.md` carries `## Files Inventory`

## Gates

- T0: `cargo fmt --all -- --check` PASS; `cargo clippy --workspace --all-targets -- -D warnings` PASS (0 warnings, 0 errors)
- T2: `cargo test -p chronos-services --lib --no-fail-fast` → 264 passed / 0 failed / 0 ignored
- T4-smoke (`--test-threads=1`, `CHRONOS_MCP_PATH` exported):
  - `chronos-sandbox/tests/e2e_connectivity::test_mcp_server_starts_and_responds` → 1/1 (5.65 s)
  - `chronos-sandbox/tests/session_lifecycle::test_session_start_attach_to_running_self` → 1/1 (15.26 s)

## Notes

- B-direct cycle. One literal rename at `crates/chronos-services/src/session_lifecycle.rs:198`, matching sandbox assertion at `chronos-sandbox/tests/session_lifecycle.rs:376`, two manual updates (en + es).
- Tag is a patch bump (`v0.7.80` → `v0.7.81`) because no wire protocol or stored schema changed.
- Working tree was clean at merge (`git status --porcelain` empty).
- m10-ms-race-fix stale SDDK ledger entry was reconciled as a separate pre-cycle operation (`cycle supersede --reason external-obsolete`); the closure event was emitted before this cycle started.
- This is the first cycle post-m9-77 that uses the SDDK ledger for the build → verify → release → archive transitions; the previous attach cycles (m9-77, m9-78) were committed before SDDK reconciliation and are backfilled into the same archival sweep via synthesized `release-report.md` files.

## Deviations

None. The cycle is a one-literal rename with matching test assertion and matching manual updates in both languages.