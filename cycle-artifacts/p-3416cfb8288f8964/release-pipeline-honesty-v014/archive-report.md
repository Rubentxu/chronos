# Archive Report — release-pipeline-honesty-v014

status: blocked
cycle_id: p-3416cfb8288f8964/release-pipeline-honesty-v014
path: B-direct
published_subject: { main_sha: 98c4cd2341058872102f458655791ec53f21bd8c, tag: v0.1.4 }
closed_at: NOT CLOSED — `archive.complete` was not applied
ledger_at_report: { event_count: 417, last_hash: sha256:524d744eda0ad3d6733733c705d533767e61f32133f34869277ea8d07c18a40d }

executive_summary: >
  The release half of v014 is done and published: v0.1.4 is pushed, annotated,
  and peels to 98c4cd23 on the remote. The archive half is not. The
  `vault-index-current` gate for `archive.complete` failed against real
  validator output — 60 errors, 30 duplicate node ids and 30 dangling wikilinks
  — so the cycle stays RELEASED/phase=archive instead of being forced to CLOSED.
  None of the 60 errors are attributable to this cycle, and this cycle added no
  vault node at all, but the archive contract makes a vault validation failure
  blocking and no precedent exists in any local ledger for a narrower reading.
  The gate is therefore recorded as failed, with the full evidence in
  archive-manifest.md, and the vault repair is left to a follow-up cycle.

## Gate outcomes

Re-evaluated 2026-09-29T16:16Z with fresh command evidence. Both receipts were
re-issued (suffix `-2`); the outcome is unchanged, but the evidence is now
reproduced by this session rather than inherited.

- { gate: ledger-valid, receipt: gate-ledger-valid-f86f75d06cd14622-2, outcome: passed,
    argv: "sddk ledger verify --format json", exit_code: 0,
    output_digest: "sha256:725b4ba88edd51345087be226b38adaa3d35728cea85a056de3aa82e3ea4a8de" }
- { gate: vault-index-current, receipt: gate-vault-index-current-f86f75d06cd14622-2, outcome: failed,
    argv: "sddk vault validate --vault ~/.sddk-knowledge/p-3416cfb8288f8964", exit_code: 1,
    output_digest: "sha256:ee826e21e75895d7587655e7df6a0b9b360f79077547e25a4f586e1306f15a74",
    nodes: 138, errors: 60 }

The `ledger-valid` digest reproduces the value recorded in the earlier
`-1` receipt, so the previous session's evidence was genuine.

## Blocked-on condition

`sddk vault validate --vault /home/rubentxu/.sddk-knowledge/p-3416cfb8288f8964`
reports `nodes: 138, errors: 60` (exit 1). `sddk vault index` rebuilds without
staleness, so this is content rot in the vault, not a stale index.

Confirmed refusal by the runtime (observed, re-attempted this session):

```
error[ENGINE_GATE_FAILED_WITHOUT_TARGET]: transition archive.complete
gate "vault-index-current" failed without an on_failure target
```

## Root cause, decomposed (measured this session)

The 60 errors are **two** distinct defect classes, not one:

**Class A — 30 `VAULT002` duplicate node ids.** Node ids are derived from the
*filename*, not the H1 title, so every cycle folder's generic
`archive-manifest.md`, `spec.md`, `tasks.md`, `proposal.md` collapses onto one
id. 12 distinct ids collide across 53 files:
`archive-manifest`(16), `spec`(6), `proposal`(6), `specs`(5), `tasks`(4),
`index`(3), `design`(2), `archive-report`, `cycle-spec`, `debt-verify`,
`explore-report`, `verify`.

> **Proven fixable.** On an isolated copy (`target/vault-probe`, never the
> canonical vault), qualifying each duplicate with its parent directory name
> takes the vault from **60 errors to 30**. Verified safe: **zero** files
> contain an inbound `[[...]]` link to any of the 12 colliding ids, so the
> rename breaks no existing edge.

**Class B — 30 `VAULT003` dangling wikilinks** from 9 distinct targets.

| Target | Cause | Status |
|---|---|---|
| `ADR-0002-capability-aware-discovery` (9 links) | File exists as `adrs/0002-capability-aware-discovery.md`; links use the `ADR-` prefix the protocol mandates | **Rename fixes all 9** (proved on copy: 30 → 21) |
| `m0-01-live-pagination/proposal` | Path-style link; real file is `cycles/m0-01-live-pagination/proposal.md` | Rewrite to bare id |
| `.../m2-native-live-probe-frame-capture/proposal` | Same path-vs-id mismatch | Rewrite to bare id |
| `capabilities-discovery/REQ-CapabilitySnapshotToolAvailability` | Same path-vs-id mismatch | Rewrite to bare id |
| `capabilities-discovery`, `m0-01-live-pagination`, `m2-native-frame-log-durable`, `m2-native-live-probe-frame-capture`, `m2-01-function-frame-exploration`, `m2-function-level-capture/m2-01/proposal` | **No file exists.** `m2-01/` holds only `apply-checkpoint.json` + `spec-gate-evidence.json` | Genuinely missing nodes |

> **A warning for whoever repairs this.** Naively rewriting the path-style
> links by bare basename is **unsafe**: `proposal` and `archive-manifest` each
> exist in many directories, so a basename-only rewrite would silently repoint
> edges at the wrong file. Repair Class B by explicit per-target mapping, never
> by blanket search-and-replace.

## Not claimed

- This cycle is **not** archived and **not** CLOSED.
- No vault node exists for v014; none was written.
- The ledger was not closed; the closing event does not exist because the
  transition never applied.
- **I did not repair the vault.** It is outside my authority: `sddk knowledge
  import` refuses with `actor_kind System not admitted on surface
  knowledge_graph_vault (admitted: [Human])`, and `sddk permission check`
  returns `allowed: false` — "agent rubentxu is not declared in the permission
  registry" (default-deny). Note this is *narrower* than "the vault CLI is
  closed to agents": `sddk vault index` runs without refusal. The binding
  constraint is the permission registry, not a blanket ban.

## Follow-ups

- **Vault repair (Human-admitted, P1 `bl-bl-01M3PXC6ZV000387DTRTP6AS00`)**:
  the two-class plan above reduces 60 → 0 in three steps, in this order:
  1. Rename the 12 colliding ids, qualified by parent directory (proven
     60 → 30, no inbound links broken).
  2. Rename `adrs/0002-capability-aware-discovery.md` to the protocol form
     `ADR-0002-…` (proven 30 → 21). Note there are **two** ADRs numbered
     `0002`; the other is `0002-execution-log-source-of-truth.md` and also
     needs the `ADR-` prefix, with its number disambiguated.
  3. Rewrite the 3 path-vs-id links by explicit mapping, then create the
     genuinely-absent nodes or repoint them.
- **pipelinek cache staleness**: P1 `bl-bl-01M3PWNDVW000387DS8ZH6DX00`.
  `--rerun` is already mandatory in `AGENTS.md`; no further action in-repo.
- **Cycle inventory artifact**: still missing for v014.
- **Withdrawn**: an earlier claim that "CI runs no project tests and would stay
  green on a failing tree" was **false** and is retracted. `ci.yml` runs on
  every push/PR to `main` and executes fmt, clippy, build, unit tests and the
  mandatory integration surface `cargo test --workspace --tests --exclude
  chronos-e2e`. Verified empirically, not just read: the skip list
  `.sddk-state/test-buckets/cargo-skip.txt` is **0 bytes**, so the CI-constructed
  `SKIP_ARGS` expands to the empty string; `chronos-sandbox` is **not** among the
  excludes (only `chronos-e2e` is); and
  `cargo test -p chronos-sandbox --test execution_log_read_e2e -- --list`
  enumerates both `execution_log_read_over_the_wire` and
  `execution_log_read_serves_a_real_log_over_the_wire`. The mutated test is an
  integration target under `chronos-sandbox/tests/`, so CI **would have failed**.
  No CI change is warranted.
