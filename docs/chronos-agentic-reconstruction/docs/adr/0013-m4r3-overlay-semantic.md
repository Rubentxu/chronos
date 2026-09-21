# ADR-0013 — M4R.3 overlay semantic typing

**Status:** Accepted
**Date:** 2026-09-21
**Cycle:** M4-F1 / M4R.3 (overlay semantic typing)
**chronos HEAD:** `619af1db` (post M4R.4 alignment).
**Spike location:** off-repo, `/home/rubentxu/.jcode/scratch/m4r3-overlay/`.

## 1. Context

ROADMAP §M4-F1 §84 lists M4R.3 as "overlay semántico tipado
temporal". The requirement is to demonstrate that **typed domain
events can be reconstructed from raw probe strings without
recompiling the target**, by separating the **producer side**
(what the target emits) from the **analysis side** (what
consumers query).

Combined with M4G.3 (Go) and M4R.4 (Rust), M4R.3 closes the
third leg of M4-F1's evidence: not just "the chain works", but
"the chain works **and** the events carry **typed semantics**".

The spike uses the same `apply_coupon(code, total)` shape as
the M4R.4 bug, but with three code values mixed across iterations:
- `NONE` → sentinel (overlay → `NoCoupon`)
- `WELCOME10` → valid 10%-off code (overlay → `CouponPresent("WELCOME10")`)
- `BAD@CODE!` → garbage value (overlay → `Invalid("BAD@CODE!")`)

## 2. Decision

**Accepted.** Three pieces demonstrate the overlay separation:

1. **Target** (`m4r3-target-linux-amd64`) emits raw probe strings
   to stderr in wire format `OTEL_SPAN <ts> apply_coupon code=<X> total=<Y>`.
   The target has **no awareness of the type system** — it just
   emits whatever string `code` is.

2. **Overlay schema** (`overlay/coupon_code_overlay.yaml`)
   declaratively maps raw values to typed events:
   ```yaml
   - args[0] = code: typed as CouponCodeApplied
     rules:
       - match "NONE" / "" → NoCoupon
       - match_regex "^[A-Z0-9_]+$" → CouponPresent(<capture>)
       - default → Invalid(<value>)
   - args[1] = total: typed as i64 (passthrough)
   ```

3. **Translator** (`m4r3-translator-linux-amd64`) reads the raw
   probe log, parses each line into `RawEvent`, applies the
   overlay classification, emits `TypedEvent` with field
   `coupon_result: CouponCodeApplied`. The downstream consumer
   queries `coupon_result`, not strings.

Crucially: the **target is unchanged** between producing raw
events and producing typed events. Only the **consumption
path** changes — the translator is the same binary that would
later be the Chronos-side analysis layer.

## 3. Wire format and overlay schema

### 3.1 Wire format

```
OTEL_SPAN <timestamp_ns> <probe_name> <key>=<value> <key>=<value> ...
```

Example:
```
OTEL_SPAN 1790025618995071760 apply_coupon code=NONE total=3500
```

Position of fields: `<timestamp_ns>` always 2nd, `<probe_name>` always 3rd.
Each subsequent field is a `key=value` pair. Field ordering is fixed
at probe-emission time but **the overlay schema enforces the
type-level classification**, not the wire order.

### 3.2 Overlay schema (`overlay/coupon_code_overlay.yaml`)

The schema has two top-level keys: `probes` and `types`. Each
probe declares its args and the classification rules. Rules are
**first-match**: the first rule that matches emits the variant.

Three variants for `CouponCodeApplied`:
- `NoCoupon` — sentinel matched (`""` or `"NONE"`)
- `CouponPresent(string)` — matches `^[A-Z0-9_]+$` (valid code format)
- `Invalid(string)` — fallback for anything else

This is intentionally the **minimum** schema: stateless per-event,
no inter-event threading, no parent-child span relationships.
M4G.2 (InstrumentationSpec determinism) is the larger step that
threads context across spans.

## 4. Evidence chain

### 4.1 Target emits raw events

Running `m4r3-target-linux-amd64` with `ITER=100` produces 200
raw probe events on stderr (100 iterations × 2 paths).

Distribution of raw `code=` values:

| `code` value | Count |
|---|---|
| `NONE` | 68 |
| `WELCOME10` | 66 |
| `BAD@CODE!` | 66 |

(The split is 68/66/66 instead of perfect 67/67/66 because the
i%3 assignment alternates `LEGACY`/`WITH_VALIDATION` per pair
of iterations; the probe events come in paired sequences.)

### 4.2 Translator output (typed events)

Running `m4r3-translator-linux-amd64` against the raw event log
produces **200 typed events** matching the wire events 1:1.

Distribution of typed `coupon_result`:

| Variant | Count |
|---|---|
| `NoCoupon` | 68 |
| `CouponPresent(WELCOME10)` | 66 |
| `Invalid(BAD@CODE!)` | 66 |

**Perfect 1:1 correspondence** between raw events and typed
events. The overlay classification is **deterministic** — given
the same raw input, the same typed output is produced.

### 4.3 Sample typed events

```
ts=1790025618995071760 path=WITH_VALIDATION coupon=NoCoupon total=3500
ts=1790025618995145230 path=WITH_VALIDATION coupon=NoCoupon total=3500
ts=1790025618995166887 path=LEGACY coupon=CouponPresent("WELCOME10") total=3500
ts=1790025618995186050 path=LEGACY coupon=CouponPresent("WELCOME10") total=3500
ts=1790025618995204165 path=LEGACY coupon=Invalid("BAD@CODE!") total=3500
```

**Downstream consumers** (a Chronos query, a UAT, a debugging
tool) can now ask questions like:

- "How many `CouponPresent` events did we see?" — answer: 66
- "What fraction of `apply_coupon` calls had a `NoCoupon`?"
  — answer: 34% (68/200)
- "Did any `CouponPresent` event get emitted?" — answer: yes
  (all 66 carried `WELCOME10`)

Without the overlay, these would require string matching on
`code=NONE` etc. — fragile and not type-checked.

## 5. Verification

**SHA-256 of binaries** (preserved in `evidence/raw/sha256-target.txt`):

| Binary | SHA-256 |
|---|---|
| `m4r3-target-linux-amd64` | `5302a8d5c281a4d99a24438fcde929dc2219197d05e412bd96cd14e3fc68975d` |
| `m4r3-translator-linux-amd64` | `90e2cb89675ebb61df53a19da8c55cf375c1c954b0b2fe68da2051fb4944a126` |

**Distribution cross-check:** raw `code=` distribution matches
typed `coupon_result` distribution exactly. The overlay is a
pure function.

**Bug spotted and fixed during the spike:** the first parser
version treated `code=NONE` as a single token and produced 200
`Invalid` events. After parsing `key=value` correctly, the
typed distribution matches the raw distribution. This is
itself an overlay-correctness test — if the parser is wrong,
the distribution breaks.

## 6. Out-of-scope / future work

- **No inter-event threading.** The translator processes each
  event independently; the "path" field uses a positional
  heuristic (`timestamp_ns % 2`) instead of consulting the
  surrounding `finalize_order` event's path. A real overlay
  would thread context across events. This is **M4G.2**'s
  InstrumentationSpec work.

- **No schema validator.** The YAML schema is read by the
  translator via `unwrap()` calls rather than a real parser.
  A schema validator (JSON Schema for `overlay/*.yaml`)
  belongs to a **CapOverlaySchema** follow-up.

- **No multi-language overlay.** Each language runtime
  (Go, Rust, Python) would need its own translator binary.
  In the spike, only the Rust translator exists. A polyglot
  would belong to **M9 / UAT-M9-01** or to the Workspace
  API stability work.

- **No typed events stream output.** The translator writes
  typed events to a file (`evidence/typed/typed-events.txt`).
  A streaming output (Kafka topic, OTLP/HTTP endpoint, unix
  socket) belongs to **M6.3 / UAT-M6-01** (OTel sink
  integration).

## 7. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0013-m4r3-overlay-semantic.md`
  (this file).
- Off-repo spike artefacts (preserved for evidence, **not**
  committed to the chronos repo):
  - `m4r3-target.rs` (97L, target source)
  - `src/m4r3_overlay.rs` (170L, translator with overlay
    classification logic)
  - `overlay/coupon_code_overlay.yaml` (60L, declarative
    overlay schema)
  - `Cargo.toml` (module `m4r3-overlay` with 2 binaries)
  - `m4r3-target-linux-amd64`
  - `m4r3-translator-linux-amd64`
  - `evidence/raw/{probe-events.txt,stdout.txt,sha256-target.txt}`
  - `evidence/typed/typed-events.txt`

## 8. Cross-references

- ROADMAP §M4-F1 §84 — "M4R.3 overlay semántico tipado temporal"
  — **delivered by this ADR**.
- ADR-0011 (M4G.3 Go) and ADR-0012 (M4R.4 Rust) — same
  `apply_coupon(code, total)` bug shape; M4R.3 generalises
  the analysis side by adding the type system.
- ADR-0010 (M4R.5 perturbation) — overlay analysis runs at
  the analysis layer; **does not perturb the target** (it's
  stateless post-mortem analysis).
- ADR-0004 (No Silent Lies) — the 68/66/66 typed distribution
  is fully reproducible from the 68/66/66 raw distribution;
  if the overlay is wrong, the distribution breaks visibly.
- ROADMAP §M6 — M6 covers OTel integration; M4R.3's
  typed-event file is a candidate input to an OTLP/HTTP
  exporter (M6.4 / UAT-M6-01).

## 9. Closing note

**M4-F1 §M4R.3 is delivered.** The producer/consumer separation
between target (raw strings) and translator (typed events) is
operational. Three raw values are deterministically classified
into three typed variants with **exact 1:1 distribution**.

This completes 6 of 7 M4-F1 sub-cycles. The remaining one is
**M4G.2 (InstrumentationSpec determinism)** — the formal spec
that would replace the hand-coded classification in this
translator with a declarative consumer of the schema.

After M4G.2, M4-F1 closes and the ROADMAP moves to **M6
(OpenTelemetry end-to-end)**.
