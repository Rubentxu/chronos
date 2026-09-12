# Merge Receipt: m9-04-side-table-key-layout

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-04-side-table-key-layout |
| Branch | main |
| Main SHA | d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc |
| Base SHA | eb2cccd6fdfcd4b00bf980453bc40c606fa69885 |
| Route | local |
| At | 2026-09-12T08:20:42Z |

## Evidence

```
$ git checkout main
$ git merge --ff-only feat/m9-04-side-table-key-layout
Fast-forward (no commit created; -m option ignored)
 crates/chronos-cli/Cargo.toml                      |    1 +
 crates/chronos-cli/src/lib.rs                      |    7 +
 crates/chronos-cli/src/replay.rs                   |    2 +-
 crates/chronos-cli/tests/replay_integration.rs     |  228 ++++
 crates/chronos-services/src/counterexample.rs      |   94 ++
 crates/chronos-store/src/counterexample_storage.rs | 1105 ++++++++++++++++++--
 crates/chronos-store/src/storage.rs                |    7 +-
 .../verify-findings.json                           |  101 ++
 .../m9-04-side-table-key-layout/verify-report.md   |  168 +++
 .../m9-04-side-table-key-layout-design.md          |  175 ++++
 .../m9-04-side-table-key-layout-scoping.md         |  979 +++++++++++++++++
 .../milestones/m9-04-side-table-key-layout-spec.md |   99 ++
 12 files changed, 2900 insertions(+), 66 deletions(-)

$ git push origin main
To github.com:Rubentxu/chronos.git
   eb2cccd..d6b3b8c  main -> main

$ git fetch origin main && test "$SHA" = "$(git rev-parse origin/main)"
VERIFIED: HEAD == origin/main == d6b3b8c51c50d2ce6d0fe4f8c804cf793137f0dc
```

## Receipt

- `git.push` capability: direct push to `origin/main`
- Full local HEAD SHA confirmed at `origin/main` after push
- Merge: fast-forward from base `eb2cccd` to head `d6b3b8c`
- Tree: clean (`git status --porcelain` shows only `.jcode/` which is gitignored)