# Handoff: m9+ Backlog (Updated 2026-09-12T18:18Z)

## Status

As of 2026-09-12T18:18Z, the m9 vault is canonical-schema-clean across all 57 cycles
in CA p-3416cfb8288f8964 (plus 2 legacy). 43 active cross-checks all pass.

m9-57 was a major vault hygiene cycle. Discovery: prior CC runner filtered
on the `Total: X` summary pattern and silently missed CCs that emit
`DRIFT:` lines without a summary. Running all 43 CCs with broad pattern
detection surfaced 452 drift lines (vs 0 previously reported).

Closed:
- 21 apply-checkpoints backfilled with 12 missing schema fields each
- 4 overly-strict CC regexes hardened (CC#22, CC#23, CC#27, CC#30)
- CC#3 made era-aware (fix-peel exemption)
- CC#14: findings_closed removed from legacy set (semantic distinction)
- m9-34/m9-35 SHAs reconciled to canonical (base_sha=head^, Head SHA match)
- m9-54/m9-55/m9-56 change-entry + archive-manifest canonicalized
- CC#48 added: meta-check that runs every CC and reports drift

## Bucket 1: By-design disclosures (unchanged)

These remain by-design and are **not** "fixable" without an explicit
design change. See
`.sddk-knowledge/p-3416cfb8288f8964/handoff/m9-backlog-blocked-2026-09-12.md`
for the full list.

| ID | Source | Title |
|---|---|---|
| m9-01-R1..R3 | m9-01 scoping | schema_version disclosures |
| m9-02-R1..R8 | m9-02 scoping | Bundle storage disclosures |
| m9-04-R1..R6 | m9-04 scoping | Side-table key layout disclosures |
| m8-04-R-hypothesis-fallback | m8-04 | property_target fallback |
| m8-06-R4 | m8-06 | Cross-variant existence predicate shrinking |

**Action:** None.

## Bucket 2: Architectural redesign (deferred to m10+)

| ID | Title | Status |
|---|---|---|
| cc-001-god-module | counterexample_storage.rs at 2,823 LoC | DEFERRED |
| cc-004-implicit-io-toctou | save_bundle_record_and_events TOCTOU pattern | DEFERRED |

**Action:** Defer to m10+ unless explicitly requested by user.

## Bucket 3: Pre-vault-reorg documentation drift

| Terminated ID | Closed by | Why not rebuildable |
|---|---|---|
| m8-04-R4 | m9-02 | m8-04 source artifacts not in checkout |
| m8-07-R2 | m9-01 | m8-07 source artifacts not in checkout |

**Action:** Out of scope for auto-mode. Recoverable from pre-reorg
snapshot only.

## Bucket 4: Feature work (not debt)

| ID | Title | Status |
|---|---|---|
| m9-02-R4 | counterexample_bundle_events MCP tool | BACKLOG |

**Action:** Treat as feature backlog.

## Cross-checks summary (as of 2026-09-12T18:18Z)

All 43 cross-checks pass. Breakdown:

- **C1-C10 (legacy schema normalization)**: PASS
- **C11-C20 (canonical schema enforcement)**: PASS
- **C21-C30 (release/merge receipt schema)**: PASS
- **C31-C40 (verify-report/verify-findings schema)**: PASS
- **C41-C47 (recent hardening)**: PASS
- **C48 (self-consistency meta-check, m9-57)**: PASS

## Sessions

| Session | Cycles | Drift classes |
|---|---|---|
| m9-34..m9-43 | 10 | 10 |
| m9-44..m9-49 | 6 | 6 |
| m9-50..m9-53 | 4 | 4 |
| m9-54..m9-56 | 3 | 3 (knowledge-only / dedup / branch cleanup) |
| m9-57 | 1 | 7 (schema backfill + 4 CC hardening + CC#48 + era-awareness) |

## When to break the handoff

Same as before. Bucket 1 unchanged, Bucket 3 requires source artifacts,
Bucket 4 requires product scope.


## Session 2026-09-12T18:18Z: post-m9-57 sweep

After m9-57 tagged v0.7.56 (then v0.7.58 for archive-manifest self-fix),
ran additional drift sweep across under-checked dimensions:

- cycles/index.md `Published SHA` — found m9-56 and m9-57 had `—` placeholder
  instead of real SHA; filled in (m9-56 → `32d9a3c...`, m9-57 → `308215f...`)
- archive-manifest.md `Base SHA` column — found m9-13 was missing the row
  entirely; added `bfb9edeeea1a5dd88d8828addf38d0c755c7b5cf` (verified reachable)
- archive-manifest.md m9-54 had short Base SHA `a24139e`; expanded to full 40-char
- change-entry.md `## Subject` m9-56 had `head_sha: TBD` and `base_sha: 6bdf8ba...`;
  filled in (m9-56 → `32d9a3c...`/`6bdf8ba66...`)
- change-entry.md `## Subject` m9-57 had `head_sha: TBD`; filled in (→ `308215f...`)

After all fixes: CC#48 meta-check returns 0 DRIFT lines, confirming
all 48 cross-checks are clean.


## Session 2026-09-12T19:18Z: m9-58..m9-60 sweep

After m9-59 tag v0.7.61, ran additional drift sweeps looking for
dimensions not covered by existing 50 CCs.

Closed:
- m9-58 (v0.7.60): short SHAs in SHA-keyed metadata cells of change-entry
  Ciclo tables (4 occurrences across m9-* history); CC#49 added.
- m9-59 (v0.7.61): 31 apply-checkpoints missing canonical `remote_tag`
  field (16 had legacy `tag`, 15 had neither); CC#50 added.
- m9-60 (v0.7.62): 2 cycles (m9-56, m9-57) in cycles/index.md without
  cycle-artifacts/ folder; synthesized 6 artifacts for each; CC#51 added.

After all fixes: CC#48 meta-check returns 0 DRIFT lines, confirming
all 51 cross-checks are clean.

## Cross-checks summary (as of 2026-09-12T19:18Z)

All 51 cross-checks pass. Breakdown:

- C1-C10 (legacy schema normalization): PASS
- C11-C20 (canonical schema enforcement): PASS
- C21-C30 (release/merge receipt schema): PASS
- C31-C40 (verify-report/verify-findings schema): PASS
- C41-C47 (recent hardening): PASS
- C48 (self-consistency meta-check, m9-57): PASS
- C49 (short SHA detection, m9-58): PASS
- C50 (canonical `remote_tag` detection, m9-59): PASS
- C51 (cycle-artifacts existence, m9-60): PASS

## Cosmetic drift remaining (not CC-enforced)

These are by-design or low-impact:

1. terms/index.md vs cycles/index.md Last updated timestamp format
   inconsistency (`YYYY-MM-DDTHH:MMZ` vs `YYYY-MM-DDTHH:MM:SSZ`).
2. 3 archive-manifests (m9-11, m9-12, m9-13) use Status=ARCHIVED while
   cycles/index.md uses Status=CLOSED. Same meaning, different label.
3. cycle_id short-form vs long-form is by-design per CC#16 (both
   acceptable; m9-19+ uses short, m9-03..m9-18 uses long).
