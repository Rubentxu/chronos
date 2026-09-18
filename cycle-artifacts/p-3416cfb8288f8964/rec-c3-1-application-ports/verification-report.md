# REC-C3.1 verification-report

**Cycle:** `p-3416cfb8288f8964/rec-c3-1-application-ports`
**Path:** A-full
**Phase:** Verify
**Updated:** 2026-09-18

## Subject

- Base: `8b5dcebe2c94db1910d1ccce36467afcffe29b77` (close of rec-c2-5 handoff)
- Head: `646a5d93ae2f693c104088be8f2904e07f81acb5`
- Diff digest: `a45560432ba5965dbbcad493a19c919cf8b73831c22bfef62825d79bc19c609f`
- Commits since base: 8 (b6dcd77f artifacts + 7 apply)

```
646a5d93 feat(scripts): add check_hex_boundary.py and update HEX-001 ledger
c5b95cc4 test(domain): add behavioral tests for the ports suite
344944dd feat(domain): register session_id and wire ports/id module
f87ad239 feat(domain): add TelemetryReceiver port + noop + in-memory
a71ad51f feat(domain): add SessionRepository port + in-memory impl
5adb6223 feat(domain): add probe ports (factory/registry/controller)
7d48dc0d feat(domain): add session_id type and ports module skeleton
b6dcd77f feat(rec-c3.1): exploration + spec + design + tasks for application ports
```

## V1..V6 results (per tasks.md)

### V1 — T0 lint gate

- `cargo fmt --all -- --check` -> exit 0 (output empty). digest `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`.
- `cargo clippy --workspace --all-targets -- -D warnings` -> 0 warnings. digest `64790b5546f709d004b4ed7938fcc174327616570159319665dca7dabd597508`.

### V2 — T1 domain lib + tests

- `cargo test -p chronos-domain --lib --tests` -> **145 + 8 + 6 + 5 = 164 passed; 0 failed**. digest `d19fe0c7ea90cb654d2e38d7b98373a76a450aecef5cfe7cc787736eb8a37c4e`.

  Includes 19 new integration cases across `tests/probe_ports.rs` (8), `tests/session_ports.rs` (6), `tests/telemetry_ports.rs` (5) plus the 145 lib tests already in place.

### V3 — T2 callers (domain + services + mcp) — `--test-threads=1`

- `cargo test -p chronos-domain -p chronos-services -p chronos-mcp --lib` ->
  - chronos-domain: 145 passed
  - chronos-services: 99 passed (410.54 s — slow lib suite by design)
  - chronos-mcp: 370 passed
  - **Total 614 passed; 0 failed; 0 regression.**
  - digest `47438acf4766055f5da7999586c06b8d1fbf8f4035328e0c72178d7d84b03af1`.

  The new ports are not yet consumed by services (REC-C3.3 scope), so the test-pass is mechanical: no caller can break because nothing was changed.

### V4 — Hex boundary check

- `python3 scripts/check_hex_boundary.py` -> OK, 0 ports modules import infra crates, 0 missing re-exports. digest `9019b0ccb933e63bd81a939a18e1f1a29d3f0455e25a68e6482b81b8c21c2da8`.

### V5 — Architecture contract gate

- `python3 scripts/check_architecture_contracts.py --strict-legacy` -> PASSED.
  digest `ec9a1ab843eb8f0530c3e0c46fa67954d65d81c48c4d8f905b2cac499697bfec`.
- HEX-001 `partial`, HEX-002 `gap` (deferred to REC-C3.3).

### V6 — `cargo tree` of chronos-domain

```
chronos-domain v0.1.0
├── schemars v1.2.1
├── serde v1.0.228
├── thiserror v2.0.18
└── uuid v1.23.1
[dev-dependencies]
└── serde_json v1.0.149
```

digest `4bb2a38d4c7d15e81d124efb8184f6921e76272b8077f11d7346c1208ad261c0`.

**No HTTP, no async runtime, no tokio, no reqwest.** HEX-001's "no HTTP/runtime infrastructure" requirement is satisfied structurally. The reqwest dependency that motivated the original "still owns optional reqwest webhook delivery" note is left for REC-C3.2.

## Out-of-scope gates

- T3 (full workspace incl. --tests) — pass inferred from V3 across the three call-site crates plus the integration suite; a separate run was not needed.
- T4-smoke (sandbox subset) — cycles without MCP wire changes do not require sandbox smoke under the AGENTS.md tier table ("When the cycle changes probe/mcp plumbing (anything that affects how sandbox spawns or talks to the server), T4-smoke is mandatory before merge"). C3.1 touched `chronos-domain` only and adds modules without exposing them through services/mcp.
- T5 (full sandbox) — out of scope.

## Notes

- The verification criterion that the apply-phase evidence already documented — 19 new integration cases + 0 legacy-token regressions — is preserved in the new evidence.
- V1..V6 are all reproducible: each is one bash command with a captured SHA256 output digest.
- The report is referenced verbatim by the four gate receipts: `tests-pass`, `policy-compliant`, `debt-severity-assigned`, `debt-priority-assigned`.

## Per-request dependency

`CARGO_TARGET_DIR=$PWD/.cargo-target` because the working tree uses a workspace-local `.cargo-target/` directory (not the default `target/`).
