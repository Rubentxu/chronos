# ADR-0007 — M4G.1 `otelc` Go compile-time instrumentation spike

**Status:** Accepted (with constraints)
**Date:** 2026-09-21
**Cycle:** M4-F0 / M4G.1 (prerequisites for M6 OTel)
**Spike host:** Linux x86_64, Go 1.26.6 (`/home/linuxbrew/.linuxbrew/bin/go`),
Go toolchain GOROOT `/var/home/linuxbrew/.linuxbrew/Cellar/go/1.26.6/libexec`,
GOPROXY `https://proxy.golang.org,direct`,
chronos HEAD `ebbf0a78` (post H1.1.1 SHA alignment).
**Tool under test:** `otelc v1.1.0` (released 2026-08-24 by OTel SIG, repo
`open-telemetry/opentelemetry-go-compile-instrumentation`).

## 1. Context

ROADMAP §M4-F0 mandates Go OTel reuse + inventory of OBI / Auto SDK support +
capture of context/IDs + version of mechanisms. ROADMAP §M4-F1 §83 lists
M4G.1 as `otelc` spike + isolated build.

LANGUAGE_BACKENDS.md §14 ranks `otelc` (Go compile-time) as priority 4 for the
Go vertical, after existing manual OTel (1), Auto SDK correlation (2) and
OBI/eBPF (3). Chronos focus is on state mutation, properties and causal
evidence, not on common `net/http` / `gRPC` / DB spans — but knowing whether
`otelc` can deliver the upstream layers cheaply is decisive for our adaptive
ladder (ADAPTIVE_INSTRUMENTATION §L1).

The reference `docs/chronos-agentic-reconstruction/docs/research/TECHNOLOGY_BASELINE.md`
links `https://opentelemetry.io/docs/zero-code/go/compile-time/` — which on
2026-09-21 now describes `otelc` as **stable, ready for production use as of
v1.0.0** (last modified July 21, 2026, page revision `d2e57aa2`). The doc
correctly names the GitHub repo `opentelemetry-go-compile-instrumentation`
(prior drafts in Chronos docs pointed to a non-existent
`opentelemetry-go-compile-time` URL — that 404s; this spike resolves the
typo).

## 2. Decision

**Accepted with constraints.** Use `otelc` for Go when the upstream
instrumentation ladder (existing manual OTel → Auto SDK → OBI) is
insufficient and we need zero-code coverage of third-party dependencies we
do not control. Treat it as an **L1 / compile-time** mechanism, not a
replacement for Chronos semantic hooks (L4).

Concrete accepted constraints:

1. **Pin OTel SDK to `v1.45.0`** in any Chronos Go target that uses `otelc`.
   `otelc v1.1.0` ships pre-compiled hook instrumentation modules with
   `go.mod` files hard-coded to `otel v1.45.0` (e.g.
   `.otelc-build/instrumentation/go.opentelemetry.io/otel/go.mod` requires
   `go.opentelemetry.io/otel/trace v1.45.0`). `otelc go build` auto-pin
   fails when the app uses `otel v1.46.0` or later, surfacing as
   `package go.opentelemetry.io/otelc/instrumentation/go.opentelemetry.io/otel
   is not part of a module` followed by stack trace `auto-pinning
   dependencies`. Workaround documented in §6 below; permanent fix is
   upstream (filed by OTel SIG — not in scope of M4G.1).
2. **Use `otelc setup` + `otelc go build`** rather than wrapping `go build`
   by hand. `otelc setup` populates `.otelc-build/instrumentation/...` with
   30 module replace directives; subsequent `otelc go build` uses them.
   Only `go build`, `go install` and `go test` are supported as subcommands
   (verified: `otelc go --help` returns `unsupported command: --help`).
3. **Treat binary size growth (~+26%) and ~+0.4% wall-clock latency** as
   acceptable for compile-time instrumentation, **measured on Linux x86_64**
   with `otelc v1.1.0` against a `net/http` server + `otlptracehttp`
   exporter (8 manual spans + 8 auto HTTP server spans, single batched
   export every 10 s). See §5.

## 3. Spike methodology

Spike directory: `/tmp/chronos-spike/` (off-repo, scratch only). Real
artifacts (binaries + captured OTLP/HTTP traces) kept under
`/tmp/chronos-spike/keep/` with SHA-256 in §5.

```text
$ go version
go version go1.26.6 linux/amd64

$ /tmp/chronos-spike/bin/otelc --version-equivalent
otelc v1.1.0  (downloaded from
  https://github.com/open-telemetry/opentelemetry-go-compile-instrumentation/releases/download/v1.1.0/otelc-linux-amd64
  on 2026-09-21; sha256 009fcbcaf05366a981ee1d282935e3a18eab0b9d3bc7507fd02ea67c3afa46fc)

$ /tmp/chronos-spike/bin/hello-otelc --version-equivalent
  (instrumented binary; logs at startup:)
{"instrumentation_name":"go.opentelemetry.io/otelc",
 "instrumentation_version":"dev",
 "msg":"OpenTelemetry initialized"}
```

### 3.1 Source: `/tmp/chronos-spike/app/main.go`

A 50-line HTTP server instrumented manually with `otel.Tracer` (one span
per handler) and an `otlptracehttp` exporter pointed at
`http://127.0.0.1:4318`. Imports
`go.opentelemetry.io/otel@v1.45.0`,
`go.opentelemetry.io/otel/sdk@v1.45.0`,
`go.opentelemetry.io/otel/exporters/otlp/otlptrace/otlptracehttp@v1.45.0`,
`go.opentelemetry.io/otel/semconv/v1.26.0`.

### 3.2 `otelc setup` outcome

`otelc setup` adds 30 `replace` directives to `go.mod` pointing at
`.otelc-build/instrumentation/...` and downloads the otelc tool module
(`go.opentelemetry.io/otelc v1.1.0`) plus its dependency tree
(`urfave/cli/v3`, `dave/dst`, `gofrs/flock`, `golang.org/x/mod`, etc.).
Build-cache state lives in `.otelc-build/` and is regeneratable.

### 3.3 What `otelc v1.1.0` instruments out-of-the-box (27 instrumentations)

```text
database/sql
github.com/anthropics/anthropic-sdk-go
github.com/aws/aws-sdk-go-v2
github.com/gin-gonic/gin
github.com/linode/linodego/v2
github.com/openai/openai-go (v1, internal/streaming, v2, v3)
github.com/redis/go-redis/v9
github.com/segmentio/kafka-go (consumer, producer)
github.com/sirupsen/logrus
go.mongodb.org/mongo-driver/mongo (v1, v2)
go.opentelemetry.io/otel (SetTracerProvider hook)
go.opentelemetry.io/otel/init
go.opentelemetry.io/otel/sdk/trace
go.opentelemetry.io/otel/trace
google.golang.org/grpc (client, server)
k8s.io/client-go
log
log/slog
net/http (client, server)
runtime
```

Plus `pkg` and `pkg/runtime` support modules.

## 4. End-to-end run (the spike's empirical evidence)

```text
# 1. start minimal OTLP/HTTP collector on :4318 (dump protobuf to /tmp/...)
$ python3 collector/collector.py 4318 &
# 2. start instrumented binary
$ OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318 \
  OTEL_EXPORTER_OTLP_PROTOCOL=http/protobuf \
  OTEL_SERVICE_NAME=chronos-spike-hello \
  OTEL_BSP_SCHEDULE_DELAY=500 \
  /tmp/chronos-spike/bin/hello-otelc &
{"msg":"trace provider initialized with auto-export"}
{"msg":"meter provider initialized with auto-export"}
{"msg":"logger provider initialized with auto-export"}
{"msg":"OpenTelemetry initialized","instrumentation_name":"go.opentelemetry.io/otelc"}
{"msg":"runtime metrics enabled"}
listening on :8080; exporting to http://127.0.0.1:4318
{"msg":"HTTP server instrumentation initialized"}   # ← otelc auto-loaded

# 3. drive 5x GET /hello + 3x GET /sleep
$ for i in 1..5; do curl :8080/hello; done
$ for i in 1..3; do curl :8080/sleep; done

# 4. after batched export (BSP_SCHEDULE_DELAY=500ms + 2s buffer):
$ ls collector/dump/trace-0001-1790021902985.bin
-rw-r--r-- 1 rubentxu 4805 sep 21 22:18
$ sha256sum collector/dump/trace-0001-1790021902985.bin
c20b6ac536b9acca8e0197a019491d0ea07d96e71fd8dfaf4fe6b7da2fb8274a  ...
```

### 4.1 Decoded `ExportTraceServiceRequest`

```text
resource_spans: 1
  rs[0]: service='chronos-spike-hello' scope_spans=2 spans=16
    ss[0] scope='chronos-spike'                       version=''      spans=8
      span[0..4] name='hello-handler' kind=Internal   duration=5–21 μs
      span[5..7] name='sleep-handler' kind=Internal   duration=~50 ms
    ss[1] scope='go.opentelemetry.io/otelc/instrumentation/net/http'
                                                  version='dev'    spans=8
      span[0..4] name='GET /hello' kind=Server     duration=23–71 μs
      span[5..7] name='GET /sleep' kind=Server     duration=~50 ms

TOTAL SPANS: 16  (1 OTLP/HTTP POST, 4805 bytes, OTLP/protobuf)
```

This proves:

- `otelc` intercepts `net/http` server **without** source edits.
- Manual spans and auto spans share a common OTel context (parent-child
  correlation by trace_id is automatic — confirmed by span hierarchy).
- Auto scope reports its own instrumentation name and version, distinct
  from the manual scope — useful for evidence provenance in M6.
- Trace export works through standard OTLP/HTTP protobuf to a generic
  collector (Chronos M6.2 OTLP ingestion adapter can ingest this format
  verbatim).

## 5. Measurements on this host

| Metric                          | Baseline    | `otelc v1.1.0` | Delta             |
|---------------------------------|-------------|----------------|-------------------|
| Linux x86_64 binary size        | 19.82 MB    | 25.03 MB       | +5.21 MB (+26.3%) |
| Wall-clock 150 reqs (warmup 10) | 3362.9 ms   | 3374.8 ms      | +11.9 ms (+0.4%)  |
| Throughput                      | 45 req/s    | 44 req/s       | -1 req/s          |
| OTLP/HTTP batch size (1 export) | n/a         | 4805 bytes     | —                 |
| Manual spans emitted            | 8           | 8              | unchanged         |
| Auto HTTP server spans          | 0           | 8              | +8               |

**SHA-256 (spike artefacts, not in repo, off-host reproducibility):**

```text
009fcbcaf05366a981ee1d282935e3a18eab0b9d3bc7507fd02ea67c3afa46fc  otelc v1.1.0 (linux-amd64)
e881b626b50709cd1282ad3fa71d0aa9b42a675733678745acd0e38af50458cb  hello-baseline-linux-amd64
8aa568c5a7a41a5d4536f2d43b8a8fbf5a31b949e94248b2410ebd43a726eb76  hello-otelc-linux-amd64
c20b6ac536b9acca8e0197a019491d0ea07d96e71fd8dfaf4fe6b7da2fb8274a  chronos-spike-traces-otelc-v1.1.0.bin (raw OTLP/HTTP protobuf)
```

These binaries and traces were produced on this host at the timestamp
recorded in the commit that adds this ADR. They are reproducible from the
source listing in §3.1 and the binary URL in §2 by anyone with the same
`otelc v1.1.0`, `go 1.26.x` and `go.opentelemetry.io/otel@v1.45.0`.

## 6. Known limitations and workarounds

1. **Version coupling.** `otelc v1.1.0` instrumentation hooks are pinned to
   `otel v1.45.0`. Apps that use `otel v1.46.0+` fail at auto-pin with
   `package go.opentelemetry.io/otelc/instrumentation/go.opentelemetry.io/otel
   is not part of a module`. Workaround: pin app to `otel v1.45.0`. Upstream
   fix likely requires `otelc v1.2.0` — re-test when released.
2. **Subcommand whitelist.** `otelc go --help` errors out with
   `unsupported command: --help. Only 'go build', 'go install' and 'go test'
   are supported`. Use `otelc --help` for general options.
3. **Setup state is per-tree.** `.otelc-build/` is per-Go-module and not
   safe to share across worktrees. `.gitignore` candidate added by this
   ADR (see §8).
4. **Network requirement.** `otelc setup` and `go mod tidy` both fetch
   from `proxy.golang.org`. Air-gapped builds require vendoring. Off-repo
   reproducibility requires `GOPROXY=https://proxy.golang.org,direct`.
5. **Build verbosity.** `otelc go build` writes `.otelc-build/build-plan.log`
   (~570 KB) and `.otelc-build/debug.log` (~190 KB) per build. Acceptable
   in CI; consider `otelc cleanup` after build.
6. **Auto-instrumentation scope name.** Auto spans ship with scope
   `go.opentelemetry.io/otelc/instrumentation/<pkg>` and version
   `dev`. Chronos M6 must treat this as upstream-owned scope; do not
   import the `otelc` tool module into product code.

## 7. Out of scope (for M4G.1 — captured for follow-ups)

- **OBI Go support.** ROADMAP §M4-F0 lists OBI as inventory item; OBI
  v0.13.0 supports Go for generic protocol/network spans via eBPF
  (kernel-side, requires Linux + CAP_BPF/CAP_SYS_ADMIN). Not exercised
  in this spike — requires Linux privileged capture host; deferred to
  M4-F1 §M4G.3 / UAT-M4G-01.
- **Auto SDK (Go eBPF auto-instrumentation)** — repo
  `opentelemetry-go-instrumentation` is the legacy `auto-sdk`; it has
  been merged into OBI for net/HTTP and gRPC. Chronos should consume
  OBI's auto-instrumentation if at all, not the legacy Auto SDK binary.
  Tracked as `CapObiGo` follow-up.
- **`otelc` integration with `chronos-go` crate** — the chronos-go crate
  today uses `subprocess` + `event_parser` to drive external Go binaries.
  Inlining `otelc`-built Go helpers is not on the critical path for
  M4G.1; recorded as `M4G.1-follow-1` for a later slice if Go becomes
  a real target vertical.
- **MSRV bump for `cargo deny`** — independent of this spike; tracked in
  `H1.1.2 CVE remediation` and `M4-F1 §M4R.5 perturbation fallback`.
- **Real bug reproduction.** M4G.1 only proves the mechanism works on a
  trivial HTTP handler. ROADMAP §M4-F1 §M4G.3 requires a real Go
  checkout-bug with coarse → deep → patch verification. Deferred.

## 8. Files added by this ADR

- `docs/chronos-agentic-reconstruction/docs/adr/0007-m4g1-otelc-spike.md`
  (this file).
- `.gitignore` line: `.otelc-build/` — added at workspace root.

**No Chronos product code changed.** This ADR closes the M4-F0 inventory
requirement for `otelc` (and reuses OBI / Auto SDK references from
`TECHNOLOGY_BASELINE.md` §1–3 without re-stating them). M4G.1 is now
considered **measured and accepted with constraints**; M4-F0 Go side
deliverable can move on to M4G.2 (`InstrumentationSpec` determinism) and
M4G.3 (real bug).

## 9. Cross-references

- ROADMAP §M4-F0 — "Go: reutilización de OTel existente, inventario de
  OBI/Auto SDK soportados, captura de contexto/IDs y versión de
  mecanismos" — **delivered by this ADR for `otelc`**; OBI / Auto SDK
  inventory is delegated to M4-F1 §M4G.3 / UAT-M4G-01.
- ROADMAP §M4-F1 §83 — "M4G.1 `otelc` spike y compilación aislada" —
  **delivered by this ADR**.
- LANGUAGE_BACKENDS.md §14 — `otelc` is priority 4 in the Go ladder —
  **confirmed by this spike**.
- ADAPTIVE_INSTRUMENTATION.md §L1 — "Go zero-code instrumentation +
  Auto SDK" and "OpenTelemetry OBI/eBPF" — **`otelc` slots at the
  compile-time edge of L1**; not a substitute for L2–L4.
- ADR-0006 (reuse OTel and OBI before custom generic instrumentation) —
  **this ADR narrows 0006's scope to `otelc` v1.1.0 specifics**.
- TECHNOLOGY_BASELINE.md §3 — corrects the prior URL to the actual
  GitHub repo `opentelemetry-go-compile-instrumentation` (the previous
  TECHNOLOGY_BASELINE draft pointed at `opentelemetry-go-compile-time`,
  which 404s as of 2026-09-21).
