# Change Entry — m8-07-hypothesis-reconstruction-fidelity

## Ciclo

| Campo | Valor |
|---|---|
| Cycle ID | `m8-07-hypothesis-reconstruction-fidelity` |
| Path | A-min |
| Status | CLOSED |
| Base SHA | `148f009a4a957ae67ac62a28bcd04e49d44db5f2` |
| Head SHA | `456824693e59a97e8167f39f770dee02ba7ed2a4` |

## Commits

| SHA | Subject |
|---|---|
| `1248994` | feat(m8-07): HypothesisInputWire + target_hypothesis in CounterexampleBundleRecord |
| `fff77da` | feat(m8-07): hypothesis_input_to_wire/from_wire + save/shrink plumbing |
| `2e346ce` | feat(m8-07): reconstruct_hypothesis uses persisted target_hypothesis |
| `4568246` | feat(m8-07): sandbox infrastructure + ce12 replay test |

## Scope

Persisted `target_hypothesis` (as `HypothesisInputWire`) alongside counterexample bundles.
Replay reconstructs from persisted wire instead of synthetic reconstruction.

## Disclosure R2 — CLOSED by m9-01

**R2 — Unknown future wire fields are dropped** (`docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md:291-297`)

> If a future chronos-services version adds a field to `HypothesisInput`, the wire
> mirror will silently drop it on persist. Mitigated by the m9+ plan to add
> `schema_version: u32` to `CounterexampleBundleSummary`.

**Closure:** m9-01 implemented the exact mitigation. `schema_version` was added to
both `CounterexampleBundleRecord` and `CounterexampleBundleSummary`
(`counterexample_storage.rs:171-207`). The loader guard on `schema_version >
CURRENT` protects against unknown future wire fields. Unknown `HypothesisInput`
fields dropped on persist are now caught by the bundle-level version guard.

**Evidence:** m9-01 cycle, `counterexample_storage.rs:234-235, 286-299`.
**Closed by:** m9-01-schema-versioning · SHA `2594814...` · tag `v0.6.0`

## Other disclosures (closed in cycle)

| ID | Título | Estado |
|---|---|---|
| D1 | HypothesisInputWire stringified enums | closed_in_cycle |
| D2 | serde(default) on target_hypothesis for backward compat | closed_in_cycle |
| D3 | reconstruct_hypothesis priority: persisted first, fallback second | closed_in_cycle |
| D4 | ReplayerReport defined locally in tools.rs | closed_in_cycle |
| D5 | CHRONOS_DB_PATH env var for shared store | closed_in_cycle |

## Artefactos

| Kind | Path |
|---|---|
| Apply checkpoint | `sddk/changes/m8-07-hypothesis-reconstruction-fidelity/apply-checkpoint.json` |
| Scoping doc | `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-scoping.md` |
| Merge doc | `docs/milestones/m8-07-hypothesis-reconstruction-fidelity-merge.md` |
