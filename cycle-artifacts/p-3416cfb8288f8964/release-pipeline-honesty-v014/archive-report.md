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

- { gate: ledger-valid, receipt: gate-ledger-valid-f86f75d06cd14622-1, outcome: passed }
- { gate: vault-index-current, receipt: gate-vault-index-current-f86f75d06cd14622-1, outcome: failed }

## Blocked-on condition

`sddk vault validate --vault /home/rubentxu/.sddk-knowledge/p-3416cfb8288f8964`
reports `errors: 60` (VAULT002 x30 duplicate ids, VAULT003 x30 missing targets).
`sddk vault index` rebuilds without staleness, so this is content rot in the
vault, not a stale index.

Confirmed refusal by the runtime (observed):

```
error[ENGINE_GATE_FAILED_WITHOUT_TARGET]: transition archive.complete
gate "vault-index-current" failed without an on_failure target
```

## Not claimed

- This cycle is **not** archived and **not** CLOSED.
- No vault node exists for v014; none was written.
- The ledger was not closed; the closing event does not exist because the
  transition never applied.

## Follow-ups

- **Vault repair cycle**: namespace the 30 colliding `archive-manifest` node ids
  per cycle; create or correct the 10 missing wikilink targets. Blocks
  `archive.complete` for this cycle and any future one.
- **pipelinek cache staleness**: backlog `bl-bl-01M3PWNDVW000387DS8ZH6DX00`.
  Consider forcing `--rerun` in AGENTS.md so no session can read a stale SUCCESS.
- **Cycle inventory artifact**: still missing for v014.
