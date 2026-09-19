# Merge receipt — rec-c3-1-application-ports

## Git state at release

- Branch: `main`
- Head (cycle end): `33b4f790` (final receipts commit)
- Pre-cycle base: `8b5dcebe` (REC-C2.5 close-out handoff commit)
- Tag: `v0.1.1` → `33b4f790^{commit}`
- Predecessor tag: `v0.1.0` (REC-C2.5-era workspace version)

## Commits in this cycle

```
8b5dcebe docs(cycle): update session handoff for 2026-09-18 close   (base, inherited from rec-c2.5 cycle)
b6dcd77f feat(rec-c3.1): exploration + spec + design + tasks for application ports
7d48dc0d feat(domain): add session_id type and ports module skeleton
5adb6223 feat(domain): add probe ports (factory/registry/controller)
a71ad51f feat(domain): add SessionRepository port + in-memory impl
f87ad239 feat(domain): add TelemetryReceiver port + noop + in-memory
344944dd feat(domain): register session_id and wire ports/id module
c5b95cc4 test(domain): add behavioral tests for the ports suite
646a5d93 feat(scripts): add check_hex_boundary.py and update HEX-001 ledger
88a24223 chore(release): bump workspace version 0.1.0 -> 0.1.1 for rec-c3-1 release
871cddf5 docs(rec-c3-1): add apply-progress, debt-report, verification-report artifacts
```

Note: base commit `8b5dcebe` belongs to the rec-c2.5 cycle (handoff doc).
The actual c3.1 work starts at `b6dcd77f`.

## Branch discipline

- Single branch: `main`.
- No chained PRs.
- No delete-rewrite.
- 9 commits reviewable independently.
- A-full path; no `feat/*` side branches created during the cycle.

## Out-of-scope drift

- No new forbidden dependency edges in `Cargo.toml` (HEX-001).
- No new uses of legacy tokens (`EventBus`, `snapshot_raw`, `drain_fired`, `TraceAdapter`, `UnsupportedOperation`).
- `chronos-domain/Cargo.toml` not modified for `chronos-log` (cyclic dep is honored; defer to REC-C3.3).

## Hashes

- Cycle diff digest: `a45560432ba5965dbbcad493a19c919cf8b73831c22bfef62825d79bc19c609f`.
- Base SHA: `8b5dcebe2c94db1910d1ccce36467afcffe29b77`.
- Head SHA: `871cddf50b118b9f33ac7e8a3cc6c98dfb30c67d` (`871cddf5`).


## Canonical SHA fields (added by CIH-C)


| Head SHA | `33b4f79004b7634a9e88b7919f8b42e854c420e8` |

| Base SHA | `unknown` |

| Branch | `main` |

| Date | `2026-09-19T00:00:00Z` |
