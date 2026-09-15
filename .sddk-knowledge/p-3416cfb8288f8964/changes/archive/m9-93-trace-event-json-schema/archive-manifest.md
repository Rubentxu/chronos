# Archive Manifest — m9-93-trace-event-json-schema

## Identification

| Field | Value |
|---|---|
| Cycle | m9-93-trace-event-json-schema |
| Path | A-min (single-crate refactor + 1-line DTO cleanup) |
| Branch | feat/m9-93-trace-event-json-schema |
| Date | 2026-09-14 |
| Base SHA | 1b8164d209b521e341b6cbc433000d3a4f57aede |
| Head SHA | `60db8b58bdcf9bb55549dad1e43c9d381b45939c` |
| Merge SHA | 1642ad3de923e202d5652b61b9837d4abe425317 |
| Source SHA | 2e2b7bda983a39d8e730162e2d6ffd3e55a03296 |
| Cascade SHA | 6203f9a4df18e1831093dfe9c74b17df7c9c154f |
| Remote tag | v0.7.95 |
| Tag peel SHA | 60db8b58bdcf9bb55549dad1e43c9d381b45939c |

## Summary

A-min Rust cycle. Closes FIND-M9-91-TRACE-EVENT-NO-JSON-SCHEMA by
adding `schemars::JsonSchema` derive to 12 chronos-domain types
(TraceEvent + 11 transitive dependencies) and dropping the m9-91
`#[schemars(skip)]` workaround on
`CounterexampleBundleEventsOutputDto::returned_events`.

After m9-93, the `counterexample_bundle_events` MCP tool publishes a
JSON Schema where `returned_events` is fully visible as an array of
`TraceEvent` objects. MCP clients can introspect the full event
stream shape.

## Drift delta

None. m9-93 is a Rust-only cycle; no vault CC drift introduced or
closed.

## Files changed

| Bucket | Files | Lines | Notes |
|---|---|---|---|
| Cargo.toml (workspace) | 1 modified | +1 | schemars features ["uuid1"] |
| chronos-domain | 4 modified | ~65 | +12 derives + 2 unit tests |
| chronos-services | 1 modified | ~30 | dropped schemars(skip), +1 unit test, updated doc |
| m9-93 cycle artifacts | 7 added | — | apply-checkpoint + 6 receipts/reports |
| m9-93 knowledge artifacts | 5 added | — | proposal + spec + tasks + exploration + change-entry |
| cycles/index.md | 1 modified | +2 | m9-93 row + Total 92 → 93 |
| terms/index.md | 1 modified | +1 | Last archive = m9-93 |

**Total**: 6 source files modified + 12 cycle artifacts + 2 index files.

## Cross-checks

- `bash scripts/check_vault_drift.sh`: PASS (48 python CCs + 7 bash CCs all clean).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean.
- `cargo fmt --all -- --check`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace --lib -- --test-threads=1`: 742 pass.
- `cargo test -p chronos-sandbox --test counterexample_tools -- --test-threads=1`: 12/12 pass.
- `cargo test -p chronos-sandbox --test e2e_connectivity -- --test-threads=1`: 1/1 pass.
- `apply-checkpoint.peel_match == true`.
- `apply-checkpoint.head_sha == release-receipt.Head SHA == 60db8b58`.
- `Remote tag` v0.7.95 peel: `60db8b58` (cycle-artifacts commit; tag pre-created at cycle-artifacts per CC#42 workaround).
- `apply-checkpoint.status == "CLOSED"`.
- `apply-checkpoint.archive_status == "complete"`.
- `apply-checkpoint.findings_introduced.no_action == []` (cc#19-compliant).
- `cycles/index.md` Total cycles = 93 (matches actual folder count).
- `terms/index.md` Last archive = m9-93-trace-event-json-schema.
- 3 new unit tests added (2 chronos-domain, 1 chronos-services).
- No regressions in 742 lib tests.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| apply-checkpoint | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/apply-checkpoint.json` | `0e0be74228a996e2a6881a4dc89ea554bce6e55f8941a28e739c3aba01a09857` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/implementation-receipt.md` | `9ba4569a098233f21d3f2ffad5e2363a12d3a775b9fc27abf8a904824a618551` |
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/merge-receipt.md` | `5dd8aec3c7601070f7f96248a87578e9e24d757a0fe0a399936b4235355a61f5` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/release-receipt.md` | `6e5960250a05064fb7787ee350dd454df5c51580f7535a2345d99406ec03d96d` |
| release-report | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/release-report.md` | `857a4520b19b599cce952e5833c2a74c04655df233c560a1a03c146df4349685` |
| verify-findings | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/verify-findings.json` | `a0be3050a7c628c27e60d472b21725a5f07da00faa56a6a372aced8dc9387aa0` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/m9-93-trace-event-json-schema/verify-report.md` | `8a8f39528964b0b9d53c32998b5092e41de8467b738245e07e2d5a9dc3286fcb` |
| proposal | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-93-trace-event-json-schema/proposal.md` | `8382243062d1a1d87946329d4c4517e30cd7520a07686ff093def2b48a786953` |
| spec | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-93-trace-event-json-schema/spec.md` | `0a8b258b5bb2ddf206f4a39bda7b7fd9da9812630b6ee4c87456750673bd9e2f` |
| tasks | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-93-trace-event-json-schema/tasks.md` | `4e2cb74c80304be9df4186fdca792002c88732b90a2f124f4e73d72a8097d4f3` |
| exploration-report | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-93-trace-event-json-schema/exploration-report.md` | `3600c8d27acb78921810916f1beff96de82969d2c3e33c0d5823a38aa1285f04` |
| change-entry | `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-93-trace-event-json-schema/change-entry.md` | `92ba2f45d2ff884ced1dc37cec73223c41c2911e3771de0003e4809ccc65d606` |

## Evidence bindings

The Artifact index above provides the SHA-256 binding between each
released artifact and the file content at archive time. Readers can
verify each binding with:

```bash
sha256sum <path>  # compare against the SHA-256 listed in the index
```

**Commit provenance:**

| Artifact | Bound to |
|---|---|
| Source commit (Rust) | `2e2b7bda983a39d8e730162e2d6ffd3e55a03296` (m9-93: add JsonSchema derive) |
| Cycle artifacts commit | `60db8b58bdcf9bb55549dad1e43c9d381b45939c` (m9-93: cycle artifacts + knowledge files) |
| Cascade commit | `6203f9a4df18e1831093dfe9c74b17df7c9c154f` (m9-93: align artifacts to v0.7.95 HEAD) |
| Merge commit | `1642ad3de923e202d5652b61b9837d4abe425317` (--no-ff merge into main) |
| Tag v0.7.95 | `60db8b58` (pre-created at cycle-artifacts per CC#42 workaround) |
