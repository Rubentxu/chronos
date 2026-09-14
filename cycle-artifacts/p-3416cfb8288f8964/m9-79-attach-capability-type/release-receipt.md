# Release Receipt — m9-79-attach-capability-type

| Field | Value |
|---|---|
| Tag | `v0.7.81` |
| Tag SHA (object) | `c3c69f6abf1cd21c0dbe72e98128043cc95097cd` |
| Tag peel (`^{commit}`) | `f41abd4580d078a3f5f1255ffd543573a4fa702d` |
| Merge commit | `f41abd4580d078a3f5f1255ffd543573a4fa702d` |
| Code commit | `917fcb036a56d6051af6429dd09225e7d0da4fe6` |
| Handoff commit | `eba71a12079d576b20a45005404ea2f7ba62e20e` |
| Base | `009b75037357d069775ad4d7a0661684e27fc9ca` |
| Branch | `feat/m9-79-attach-capability-type` |
| Cycle | `p-3416cfb8288f8964/m9-79-attach-capability-type` |
| Path | `B-direct` |
| Released at | 2026-09-14T06:28:00+02:00 |

## Peel match verification

```
git rev-parse v0.7.81         → c3c69f6abf1cd21c0dbe72e98128043cc95097cd
git rev-parse v0.7.81^{commit} → f41abd4580d078a3f5f1255ffd543573a4fa702d
git rev-parse HEAD             → f41abd4580d078a3f5f1255ffd543573a4fa702d
git rev-parse origin/main      → f41abd4580d078a3f5f1255ffd543573a4fa702d
git ls-remote origin v0.7.81   → c3c69f6abf1cd21c0dbe72e98128043cc95097cd refs/tags/v0.7.81
```

HEAD == origin/main == tag peel; remote tag SHA matches local. CC#12 clean.

## Commits since base (3)

| SHA | Title |
|---|---|
| `917fcb0` | feat(m9-79): distinguish ptrace attach capabilities |
| `eba71a1` | docs(handoff): persist m9 attach and SDDK recovery state |
| `f41abd4` | merge: m9-79 attach capability type |

## Gate receipts

| Gate | Receipt | Verdict |
|---|---|---|
| `implementation-complete` | `gate-implementation-complete-35eb8158c41aa3fb-1` | passed |
| `tests-pass` | `gate-tests-pass-b4e15865a16e42dd-1` | passed |
| `policy-compliant` | `gate-policy-compliant-b4e15865a16e42dd-1` | passed |

## Tiers run

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check` | PASS |
| T0 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| T2 | `cargo test -p chronos-services --lib --no-fail-fast` | PASS (264/264) |
| T4-smoke | `cargo test -p chronos-sandbox --test e2e_connectivity -- --test-threads=1` | PASS (1/1) |
| T4-smoke | `cargo test -p chronos-sandbox --test session_lifecycle test_session_start_attach_to_running_self -- --test-threads=1` | PASS (1/1, 15.26 s) |

`CHRONOS_MCP_PATH=/var/home/rubentxu/cargo-targets/debug/chronos-mcp` set explicitly.

## Notes

- B-direct cycle. One literal rename (`ebpf_user` → `ptrace_attach`) at
  `crates/chronos-services/src/session_lifecycle.rs:198`, matching sandbox
  assertion at `chronos-sandbox/tests/session_lifecycle.rs:376`, plus two manual
  updates (en + es). Tag is a patch bump because no wire protocol or stored
  schema changed.
- Working tree was clean at merge (`git status --porcelain` empty).
- m10-ms-race-fix stale SDDK ledger entry was reconciled as a separate pre-cycle
  operation (`cycle supersede --reason external-obsolete`); the closure event
  was emitted before this cycle started.
- Post-release: see follow-up commit for cycle artifacts + vault sync.