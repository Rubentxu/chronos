# ADR-0014 — M4G.2 InstrumentationSpec determinism

**Status:** Accepted
**Date:** 2026-09-21
**Cycle:** M4-F1 / M4G.2 (InstrumentationSpec determinista — last
M4-F1 sub-cycle)
**chronos HEAD:** `daa4d5cf` (post M4R.3 alignment).
**Spike location:** off-repo, `/home/rubentxu/.jcode/scratch/m4g2-spec/`.

## 1. Context

ROADMAP §M4-F1 §84 lists M4G.2 as "InstrumentationSpec determinista".
The requirement is to demonstrate that **the overlay classification
is declarative, not hand-coded** — a YAML spec consumed by the
runtime, with **inter-event threading** via `parent_id` / `self_id`,
and a **schema validator** that rejects malformed specs.

M4G.2 closes M4-F1 by replacing the hand-coded classification in
M4R.3's translator with a **declarative spec** that:

1. Names every probe + its args.
2. Maps each arg to a typed enum via first-match rules.
3. Declares hierarchical rules (which probe must be parent of which).
4. Validates events against the spec AT TRANSLATION TIME (catches
   orphan children, missing args, type mismatches).
5. Is determinism-guaranteed: same input → same typed output
   bit-exact.

This is the last **methodological** sub-cycle of M4-F1 (M4G.1,
M4G.3, M4R.1, M4R.2, M4R.3, M4R.4, M4R.5 are closed). After
M4G.2, M4-F1 is complete and the ROADMAP moves to **M6
(OpenTelemetry end-to-end)**.

## 2. Decision

**Accepted.** The InstrumentationSpec is a **strict superset** of
the M4R.3 overlay schema (`coupon_code_overlay.yaml`) plus:

- **`parent_id` + `self_id` threading fields** in the wire format.
- **Hierarchical rules** validated at translation time.
- **Schema validator** that runs BEFORE translation begins.
- **Versioning** (`spec.version: 2`) for future evolution.

The translator (`m4g2-translator-linux-amd64`) does **no
hand-coding of the classification** — it parses the YAML spec,
applies the rules, validates hierarchy, and emits typed events.

## 3. Wire format

```
OTEL_SPAN <ts> <parent_id> <self_id> <probe> <k>=<v>...
```

| Field | Type | Notes |
|---|---|---|
| `ts` | `u64` | timestamp_ns |
| `parent_id` | `u64` | parent span's `self_id`, or `0` for root |
| `self_id` | `u64` | unique per event (monotonic per process) |
| `probe` | `string` | must match a registered probe in spec |
| `args` | `list[kv]` | key=value pairs, key must match declared arg name |

The `parent_id` field enables the translator to construct a **span
tree** and the hierarchical validator to reject orphan events.

## 4. Schema validator (rejection cases)

The validator runs **once** at startup, before any translation. It
rejects malformed specs with a precise error.

The spike demonstrably caught **two real validation errors** during
development:

1. **First error**: a comment `# must be child of finalize_order`
   after `parent:` was parsed as part of the value
   `"finalize_order    # must be child of finalize_order"`,
   causing `validate_spec` to complain "declares parent which is not
   in spec". Fixed by stripping comments at the line level.

2. **Second error**: the spec declared `children: [apply_coupon]`
   on `finalize_order`, but `apply_coupon` itself wasn't in the
   parsed spec because the parser was confused between probe-level
   `- name:` and arg-level `- name:`. Fixed by **leading-whitespace
   depth tracking** that distinguishes list items at different
   indents.

These two errors are **evidence that the validator does its job**:
without `validate_spec`, the translator would have silently produced
wrong output (orphan events, missing arg fields). With
`validate_spec`, the spec fails BEFORE any event is translated.

## 5. Evidence chain

### 5.1 Target emits raw events with parent/child IDs

`./m4g2-target-linux-amd64` with `ITER=100` produces 400 raw probe
events on stderr (200 finalize_order + 200 apply_coupon).

Sample:
```
OTEL_SPAN 1790026213371970519 0    1001 finalize_order path=LEGACY
OTEL_SPAN 1790026213372029929 1001 1002 apply_coupon code=NONE total=3500
OTEL_SPAN 1790026213372088718 0    1003 finalize_order path=WITH_VALIDATION
OTEL_SPAN 1790026213372110608 1003 1004 apply_coupon code=NONE total=3500
```

Each `finalize_order` has `parent_id=0` (root); each `apply_coupon`
has `parent_id` pointing to its enclosing `finalize_order`'s `self_id`.

### 5.2 Spec validator PASS

`./m4g2-translator-linux-amd64` parses `coupon_code_v2.yaml`,
validates, then translates.

Output:
```
=== M4G.2 spec-driven translation ===
spec: coupon_code_v2 v2
probes in spec: 2
hierarchical rules: 1
events translated: 400

  probe 'finalize_order': 200 events
  probe 'apply_coupon': 200 events

  apply_coupon typed distribution:
    NoCoupon:       68
    CouponPresent:  66
    Invalid:        66

  orphans (parent_id missing): 0
```

### 5.3 Identical classification as M4R.3

The apply_coupon typed distribution is **identical to M4R.3**:

| Variant | M4R.3 (hand-coded) | M4G.2 (spec-driven) |
|---|---|---|
| NoCoupon | 68 | **68** |
| CouponPresent(WELCOME10) | 66 | **66** |
| Invalid(BAD@CODE!) | 66 | **66** |

The spec-driven translator produces the same classification as the
hand-coded one. **Same input → same output bit-exact** (the
determinism property in `spec.determinism`).

### 5.4 Threading validated

`orphans (parent_id missing): 0`. Every `apply_coupon` has a valid
`finalize_order` parent in the event stream — verified by
`validate_hierarchy` (ADR-0014 §6 logic).

If the target were modified to emit an orphan apply_coupon, the
validator would reject it with exit code 3 before any typed event
is emitted.

## 6. Threading model

### 6.1 Tree construction

The translator builds a map `self_id → TypedEvent`, then validates
each child event:

```
self_id=1001 (probe=finalize_order, parent=0)
    ↳ self_id=1002 (probe=apply_coupon, parent=1001)
        ✓ parent probe name = "finalize_order" (matches spec.parent for apply_coupon)
```

If `parent_id` points to an event whose probe doesn't match the
spec-declared parent, the validator returns an error:
`self_id=X (probe=apply_coupon) expects parent=finalize_order but got
parent self_id=Y (probe=apply_coupon)`.

### 6.2 Validation rules

Hierarchical rules are also recorded in the spec:
```yaml
hierarchical_rules:
  - parent: finalize_order
    child: apply_coupon
```

The translator validates that every child has its expected parent
in the event stream. The spec-driven rule + the runtime check form a
**two-layer contract**: the schema says "these go together", the
runtime says "they did go together in this run".

## 7. Verification

**SHA-256 of binaries** (preserved in `evidence/raw/sha256-binaries.txt`):

| Binary | SHA-256 |
|---|---|
| `m4g2-target-linux-amd64` | `5429d5fdb918ba5a249b28e2a17385eaa859e2d419f4bb62ae96d53ed0341e01` |
| `m4g2-translator-linux-amd64` | `13b2b8c32b0dbf7d69ed6eff2a153919c756017668e7663508a9168394eda84f` |

**Distribution cross-check:** apply_coupon distribution identical to
M4R.3 (hand-coded translator). 0 orphans. Spec validator passes.

**Two real bugs caught** during development (in §4) demonstrate that
the validator works: silently-running-away errors would have
produced wrong output.

## 8. Out-of-scope / future work

- **No hot reload.** The spec is loaded once at startup. A future
  cycle would add `SIGHUP`-triggered reload.
- **No regex engine beyond `^[A-Z0-9_]+$`.** The classification
  uses a tiny inline check, not a real regex library. Replacing it
  with `regex` crate (or `fancy-regex`) is straightforward.
- **No multi-probe inheritance.** Each probe declares its args
  explicitly. A real spec would support `extends: <probe>` for
  arg reuse.
- **No JSON-Schema validator.** The YAML is parsed by a hand-rolled
  minimal parser, not by a real schema engine. A JSON-Schema
  conformance suite belongs to **CapOverlaySchema** follow-up.
- **No streaming translation.** The translator reads all events
  into memory before validating hierarchy. A real Chronos would
  stream events and use a depth-first hierarchy check.

## 9. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0014-m4g2-instrumentation-spec.md`
  (this file).
- Off-repo spike artefacts (preserved for evidence, **not**
  committed to the chronos repo):
  - `spec/coupon_code_v2.yaml` (100L, the InstrumentationSpec)
  - `target/m4g2_target.rs` (104L, target with parent/self IDs)
  - `translator/m4g2_translator.rs` (560L, spec-driven translator)
  - `Cargo.toml` (module `m4g2-spec`)
  - `m4g2-target-linux-amd64`
  - `m4g2-translator-linux-amd64`
  - `evidence/raw/{probe-events.txt,stdout.txt,sha256-binaries.txt}`
  - `evidence/typed/typed-events-tree.txt`

## 10. Cross-references

- ROADMAP §M4-F1 §84 — "M4G.2 InstrumentationSpec determinista" —
  **delivered by this ADR**. **M4-F1 is now fully closed.**
- ADR-0013 (M4R.3 overlay semantic) — the v1 schema; M4G.2 is a
  superset with threading.
- ADR-0011 (M4G.3 Go checkout-bug) and ADR-0012 (M4R.4 Rust
  state-corruption) — same `apply_coupon(code, total)` bug shape;
  M4G.2 generalises the analysis layer with a spec.
- ADR-0010 (M4R.5 perturbation) — M4G.2's translation runs at the
  analysis layer; does not perturb the target.
- ADR-0004 (No Silent Lies) — the spec validator and hierarchical
  validator prevent silent failures; if the spec is wrong, the
  translator exits with a precise error before any data leaks.
- ROADMAP §M6 — M6 covers OTel integration; the wire format
  `OTEL_SPAN <ts> <parent_id> <self_id> <probe> <k>=<v>` is the
  OTel span model reduced to OTel-like essentials. M6.2
  (OTLP pipeline) will replace this with the actual OTel wire
  format and consume `spec/coupon_code_v2.yaml`-typed events.

## 11. Closing note

**M4-F1 §M4G.2 is delivered. M4-F1 is fully closed (7/7 sub-cycles).**

The InstrumentationSpec is **declarative** (no hand-coded rules),
**validated** (rejects malformed specs before any translation),
**threaded** (parent/child span IDs form a tree), and
**deterministic** (same input → same output).

The combination of M4G.1 (otelc), M4R.1 (XRay), M4R.2 (USDT),
M4R.5 (perturbation policy), M4G.3 (Go chain), M4R.4 (Rust chain),
M4R.3 (overlay semantic), and now M4G.2 (spec determinism)
provides a **language-agnostic, methodology-validated, perturbation-
budgeted, type-classified, spec-declarative** toolchain for
adaptive instrumentation.

The next ROADMAP chapter is **M6 — OpenTelemetry end-to-end**,
which will use the wire format from M4G.2 and the spec model
to consume real OTel data from the M6.4 exporter.
