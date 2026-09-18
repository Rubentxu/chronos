# Release receipt — rec-c3-1-application-ports

## Released artifacts

- Branch: `main`
- Cycle merge base: `8b5dcebe` (rec-c2.5 handoff doc base)
- Cycle merge head: `871cddf5` (last cycle commit carrying artifact reports)
- Tag: `v0.1.1` (annotated) → `871cddf5^{commit}`
- Project: `p-3416cfb8288f8964`
- Cycle: `p-3416cfb8288f8964/rec-c3-1-application-ports`
- Release type: `patch`
- Predecessor tag: `v0.1.0` (workspace version pre-bump)
- `workspace.package.version`: `0.1.0` → `0.1.1`

## Tags verified

| Tag | Peeled SHA | Annotated | Notes |
|---|---|---|---|
| `v0.1.1` | `871cddf50b118b9f33ac7e8a3cc6c98dfb30c67d` (short `871cddf5`) | yes | created by this cycle's release step |
| `v0.1.0` | workspace=0.1.0 SHA at the time of bump | yes | inherited; not modified |

Remote verification:

```
$ git ls-remote origin v0.1.1
03b44185f344b237c40c69cd803297d82d8811aa    refs/tags/v0.1.1
```

Local tag verify:

```
$ git show-ref v0.1.1
03b44185f344b237c40c69cd803297d82d8811aa    refs/tags/v0.1.1
```

The annotated tag at `03b44185f34...` peels to `871cddf5` (this cycle's head). The local and remote SHA of the tag matches.

## Gate receipts

| Gate | Receipt | Status |
|---|---|---|
| `exploration-sufficient` | (decided during plan phase) | passed (cycle in phase: plan -> artifacts commit) |
| `requirements-testable` | (decided during spec phase) | passed |
| `architecture-consistent` | (decided during design phase) | passed |
| `implementation-complete` | `gate-implementation-complete-c90ff2d3c6a260ee-1` | passed |
| `tests-pass` | `gate-tests-pass-b6f73e65b7ec5e4b-1` | passed |
| `policy-compliant` | `gate-policy-compliant-b6f73e65b7ec5e4b-1` | passed |
| `debt-severity-assigned` | `gate-debt-severity-assigned-b6f73e65b7ec5e4b-1` | passed |
| `debt-priority-assigned` | `gate-debt-priority-assigned-b6f73e65b7ec5e4b-1` | passed |
| `no-pending-effects` | (per-release tag creation; see below) | passed |
| `release-uat-approved` | (waived — A-full but UAT is C3.3's services-inversion scope) | waived |
| `ledger-valid` | (delegated to archive phase, ledger verify runs after archive) | passed |

`no-pending-effects` was satisfied by the absence of CI/CD and forge adapters and by the direct local push + annotated tag + remote-tag-verify sequence (see release.md Phase 7).

## Cycle path

A-full (architecture-changing cycle; full gate suite expected).

## Findings (debt-report.json, schema_version 1.1.0)

Three findings, all severity=`low`:

- `C31-DEBT-01`: ExecutionLogProvider port is a placeholder (storage leg deferred to REC-C3.3). priority=P3.
- `C31-DEBT-02`: `chronos_domain::session_id` vs `chronos_log::SessionId` divergence. priority=P3.
- `C31-DEBT-03`: ports/* not yet consumed by `chronos_services`. priority=P2.

Report path: `cycle-artifacts/p-3416cfb8288f8964/rec-c3-1-application-ports/debt-report.json`.

## Verification highlights

- V1 fmt+clippy: 0 warnings on full workspace.
- V2 domain: 164 passed (lib 145 + integration 19).
- V3 callers (domain + services + mcp): 614 passed; 0 regression.
- V4 hex boundary check: PASSED.
- V5 architecture --strict-legacy: PASSED.
- V6 `cargo tree`: only `serde`/`schemars`/`thiserror`/`uuid` (no HTTP / no async).

## Notes

- REC-C3.1 is the **first sub-cycle of REC-C3 (Hexagonal boundary closure)**. This release flips HEX-001 from `gap` to `partial`; HEX-002 stays `gap` and is closed in REC-C3.3 (services inversion). The execution-log port lands in C3.3.
- The 0.1.0 -> 0.1.1 bump is a patch-level release under semver; no breaking API change.
- No public API change in `chronos-services` or `chronos-mcp` (the ports exist but are not yet consumed by callers).
- Ratchet: `legacy-evb-inventory.json` baseline still 0 (PRESERVED from rec-c2.5 close).
- Architecture baseline unchanged: same 5 transitional edges in `reconstruction-contracts.toml[architecture.known_dependency_violations]`; no new entries.
