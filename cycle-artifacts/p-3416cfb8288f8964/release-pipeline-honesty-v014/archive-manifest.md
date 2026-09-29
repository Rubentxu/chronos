# Archive Manifest — release-pipeline-honesty-v014 (v0.1.4)

status: **BLOCKED — archive.complete NOT applied**
cycle_id: p-3416cfb8288f8964/release-pipeline-honesty-v014
project_id: p-3416cfb8288f8964
workspace_id: w-361237634265a0a7d986676e
path: B-direct
published_subject: { main_sha: 98c4cd2341058872102f458655791ec53f21bd8c, tag: v0.1.4, tag_peel: 98c4cd2341058872102f458655791ec53f21bd8c }
cycle_status_before_archive: { status: RELEASED, phase: archive, updated_at: 2026-09-29T15:31:07.028992696Z, lease: none }

## Why this cycle is not CLOSED

The release itself is complete and published. The **archive** transition is not,
because the `vault-index-current` gate for `archive.complete` failed against real
observed vault validation output. The runtime refuses to advance with a failed
gate; this was confirmed by an attempted transition, which the engine rejected
with `ENGINE_GATE_FAILED_WITHOUT_TARGET`.

Attempted and refused (observed, not assumed):

```
sddk cycle transition --cycle p-3416cfb8288f8964/release-pipeline-honesty-v014 \
  --transition archive.complete \
  --artifact archive-manifest=<this file> \
  --gate-receipt gate-ledger-valid-f86f75d06cd14622-1 \
  --gate-receipt gate-vault-index-current-f86f75d06cd14622-1

error[ENGINE_GATE_FAILED_WITHOUT_TARGET]: transition archive.complete
gate "vault-index-current" failed without an on_failure target
```

## Gates evaluated for archive.complete

| Gate | Outcome | Receipt | Basis |
|---|---|---|---|
| `ledger-valid` | passed | `gate-ledger-valid-f86f75d06cd14622-1` | `sddk ledger verify --root . --scope . --format json` exit 0, 417 events, last_hash `sha256:524d744e…`, output_digest `sha256:725b4ba8…` |
| `vault-index-current` | **failed** | `gate-vault-index-current-f86f75d06cd14622-1` | `sddk vault validate` reports 60 errors |

## vault-index-current evidence (observed)

Command:

```
sddk vault validate --vault /home/rubentxu/.sddk-knowledge/p-3416cfb8288f8964
```

Observed result: `nodes: 138, backlinks: 62, errors: 60, warnings: 0`.

Error breakdown:

| Code | Count | Meaning |
|---|---|---|
| `VAULT002` | 30 | duplicate node id `archive-manifest` reused across many `changes/archive/*/archive-manifest.md` |
| `VAULT003` | 30 | wikilinks pointing at missing target nodes |

Index staleness is **not** the problem. `sddk vault index` rebuilt cleanly
(`inserted: 137, updated: 1, deleted: 0`) and a second `vault validate` reported
the identical 60 errors, so the FTS index is current.

## Attribution: these errors are pre-existing and not caused by this cycle

- Zero of the 60 diagnostics mention `v014` or `release-pipeline-honesty`
  (verified by filtering the validator output for both strings).
- The colliding/missing nodes belong to cycles from the m0-01..m8-07 era and to
  `ci-vault-drift-verify-findings-restoration` / `sandbox-stale-binary-dependency-closure`.
- This cycle contributed **no** vault node at all: no
  `changes/archive/*v014*` directory exists in the vault.

Missing VAULT003 targets (10 distinct):

`ADR-0002-capability-aware-discovery`, `capabilities-discovery`,
`capabilities-discovery/REQ-CapabilitySnapshotToolAvailability`,
`m0-01-live-pagination`, `m0-01-live-pagination/proposal`,
`m2-01-function-frame-exploration`, `m2-function-level-capture/m2-01/proposal`,
`m2-function-level-capture/m2-native-live-probe-frame-capture/proposal`,
`m2-native-frame-log-durable`, `m2-native-live-probe-frame-capture`.

## Why a failed gate was recorded anyway

The archive phase contract states: "Vault validation or ledger validation fails
→ `blocked`". There is no documented carve-out that lets a cycle close on a
locally-clean vault while the validator reports errors, and no prior
`vault-index-current` receipt exists in any local ledger to establish a
precedent. Recording this gate as `passed` would have required either ignoring
the validator output or silently narrowing the gate to index staleness only.
Neither is honest, so the gate is recorded as `failed` and the cycle stays open
in `archive` until the vault is genuinely repaired.

## Ledger contract

- `archive.complete` ledger row: `lifecycle.cycle.transition.archive`
- Expected artifact on success: this file
- Final ledger evidence is deliberately **absent**: it does not exist until
  `archive.complete` appends the closing event.
- Ledger state at manifest time: 417 events,
  `sha256:524d744eda0ad3d6733733c705d533767e61f32133f34869277ea8d07c18a40d`.

## Artifact index

| Kind | Path | SHA-256 |
|---|---|---|
| implementation-receipt | `implementation-receipt.json` | `39022fe221dab69f17865784b47953f34bc4dfb3bb0f97a7ab457e224e7c5d19` |
| merge-receipt | `merge-receipt.md` | `a24c798932141ef0175e9bad4b65a7a5a4bde560a83a5cf9d40479c9a47d8a07` |
| release-receipt | `release-receipt.md` | `c68b9f75ab37a1c406b5e1239233ffc6b0eaf8fc600b4a63c1e9f63eb037d2e4` |
| release-report | `release-report.md` | `f0cb84e9b9c21d96799b03a602c827b9affde6fbd7d5b6a3eb6005e8da5b80ec` |
| verify-report | `verify-report.md` | `e534ca720d816fc356aa8c995e907905a386a63e7b5b796661a557704d6a06ba` |
| verify-findings | `verify-findings.json` | `33ce8d53552894daa57e80b3612ab88e4a757ca9deb1c238b76537c83ba201ea` |

All paths are relative to
`cycle-artifacts/p-3416cfb8288f8964/release-pipeline-honesty-v014/`.

## Release evidence bound into this manifest

- Tag `v0.1.4` is annotated (tag object `d617c17799d0b65ed6ad1af3e6694a8e98c79c1a`)
  and peels to `98c4cd2341058872102f458655791ec53f21bd8c`.
- `release_converged: true`; remote main and remote tag peel were verified
  independently after the push.
- `main` has since advanced to `84c42bd97cc7e951d4662232b725a2cd98b8c6b3` (docs
  only). The tag stays at `98c4cd23` **on purpose**. Do not move or recreate it.

## Known limitations carried forward

1. `pipelinek` can report SUCCESS from a stale cache. Every pipeline claim in
   this cycle used `--rerun`. Tracked as backlog item
   `bl-bl-01M3PWNDVW000387DS8ZH6DX00`.
2. No real macOS runner was observed. Platform matrix remains unverified.
3. The `v0.1.4` tag is annotated but **unsigned**.
4. UAT was **not executed**. `release-uat-approved` passed by documented policy
   skip (no `uat.toml`; patch releases default to `skip`), not by a test run.
5. The cycle inventory artifact was never created.
6. The vault has 60 pre-existing errors blocking `archive.complete` (this file).

## Root cause of the 60 errors (diagnosed, not yet fixed)

The two error classes are one problem: **node ids are derived from markdown
filenames, so every canonical filename collides vault-wide, and links written
against directory names resolve to nothing.**

SDDK indexes 138 nodes, which is exactly the count of `.md` files in the vault.
The vault also holds 62 directories, and those are never indexed as nodes.

`VAULT002` — 30 duplicate ids, from files named by *role* rather than by cycle:

| Duplicate id | Files |
|---|---|
| `archive-manifest` | 6 under `changes/archive/m0-02…m8-07*/` |
| `specs` | 4 under `changes/archive/m0-02…m0-06*/` |
| `spec` | 5 (incl. `specs/capabilities/spec.md`, `cycles/m0-truth-first-foundation/spec.md`) |
| `proposal` | 3 |
| `tasks` | 3 |
| `design` | 2 |
| `index` | 2 (`cycles/index.md`, `terms/index.md`) |
| `archive-report`, `cycle-spec`, `debt-verify`, `explore-report`, `verify` | 1 each |

`VAULT003` — 30 dangling wikilinks, because links were written against
**directory** names. Those directories do exist and hold real content; there is
simply no node to point at:

- `specs/capabilities-discovery/` exists and contains `spec.md` plus 9
  `REQ-*.md` files. `[[capabilities-discovery]]` has no node.
- `cycles/m2-function-level-capture/m2-native-frame-log-durable/` exists with
  `cycle-spec.md`, `apply-checkpoint.json`, `archive-manifest.json`, `release/`.
- `adrs/0002-capability-aware-discovery.md` exists, yet
  `ADR-0002-capability-aware-discovery` does not resolve — the id scheme is not
  the filename.
- `sddk vault search` for `capabilities-discovery`, `m0-01-live-pagination`,
  `m2-native-frame-log-durable` and `m2-native-live-probe-frame-capture` all
  return no node.

**Consequence:** fixing this is a rename/reindex migration over the whole vault,
not a patch. Every one of the 30 colliding ids must become cycle-scoped (for
example `archive-manifest--m0-02-snapshots-cumulative`), and 30 wikilinks must be
retargeted to whatever the new scheme produces.

## Why this was not done in this cycle

`~/.sddk-knowledge/p-3416cfb8288f8964` is a **derived artifact that lives
outside the repository**. It is not under git, `sddk knowledge verify` reports
`registry_present: false` with 178 untracked files, and therefore nothing in it
can be regenerated from source. A mass rename there has:

- no version control and no rollback,
- 28 files that reference `archive-manifest` and would need retargeting,
- a naming decision (cycle-scoped ids vs. frontmatter ids vs. directory-indexed
  nodes) that changes how every future cycle writes its vault node.

A verifiable copy was taken before stopping:
`~/.jcode/scratch/vault-backup-20260929-174612` (163 files, 992K).

The naming scheme is a design decision about the knowledge vault, not a cleanup
task inside a release cycle, so it is left to its own cycle with the operator's
awareness. Recorded as backlog `bl-bl-01M3PXC6ZV000387DTRTP6AS00` (P1).

## Next executable step

Decide the vault identity scheme, then repair it in a dedicated cycle. Target:
`sddk vault validate` → `errors: 0`. After that, re-evaluate
`vault-index-current` and apply `archive.complete` for v014.
