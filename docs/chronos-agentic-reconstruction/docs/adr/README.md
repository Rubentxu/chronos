# Architecture Decision Records

Accepted for the reconstruction baseline unless superseded.

| ADR | Decision |
|---|---|
| [0001](0001-agent-first-product.md) | Agent-first execution intelligence |
| [0002](0002-execution-log-source-of-truth.md) | ExecutionLog is authoritative |
| [0003](0003-projections-not-primary-storage.md) | Graphs/indexes are projections |
| [0004](0004-no-silent-lies.md) | Explicit uncertainty and gaps |
| [0005](0005-adaptive-instrumentation.md) | Adaptive instrumentation ladder |
| [0006](0006-opentelemetry-reuse-first.md) | Reuse OTel/OBI before custom generic probes |
| [0007](0007-semantic-probe-compiler.md) | LLM requests evidence; deterministic compiler creates probes |
| [0008](0008-breakpoints-internal-only.md) | Breakpoints are internal escalation mechanisms |
| [0009](0009-mcp-thin-driving-adapter.md) | MCP is a thin adapter |
| [0010](0010-sandbox-as-uat.md) | Preserve and evolve sandbox |
| [0011](0011-local-persistence-segments-checkpoints.md) | Segment log persistence + checkpoints |
| [0012](0012-go-and-rust-reference-backends.md) | Go and Rust are near-term reference backends |
| [0013](0013-activegraph-patterns-without-dependency.md) | Adopt event-sourcing patterns without ActiveGraph dependency |
| [0014](0014-m4g2-instrumentation-spec.md) | M4G.2 InstrumentationSpec determinista |
| [0015](0015-m6.1-external-trace-context.md) | M6.1 ExternalTraceContext separado de InvocationId |
| [0016](0016-m6.2-otlp-adapter.md) | M6.2 OTLP adapter de ingesta local |
| [0017](0017-m6.3-otel-correlation.md) | M6.3 correlación no ambigua con evidencia de mutaciones |
| [0018](0018-m6.4-otel-exporter.md) | M6.4 exportación opt-in de eventos compatibles y límites |
| [0019](0019-m6.5-otel-redaction.md) | M6.5 seguridad/redacción y cardinalidad |
| [0020](0020-m6.6-otel-cross-service.md) | M6.6 UAT cross-service con dos servicios y requests concurrentes |
| [0021](0021-m6.7-otel-gates.md) | M6.7 gates de carga/recuperación/errores |
| [0022](0022-m7.1-differential-equivalence.md) | M7.1 criterio de equivalencia semántica y hashes jerárquicos |
| [0023](0023-m7.2-invocation-alignment.md) | M7.2 alineación por invocación/contexto, no sólo timestamps |
| [0024](0024-m7.3-behaviour-fingerprint.md) | M7.3 BehaviourFingerprint como spike medido |
| [0025](0025-m7.4-cost-memory-collision.md) | M7.4 UAT-M7-01/02 y baselines de coste/memoria |
| [0026](0026-m8.1-inventory-foundation.md) | M8.1 preservar e inventariar foundation histórico de shrinking |
| [0027](0027-m9.1-inventory-causal-concurrency.md) | M9.1 inventory + naming convention (vault cycles `cc-m9-NN`) |
| [0028](0028-m9-scoping-causal-concurrency.md) | M9 chapter scoping: build on CausalityIndex pre-existente |
| [0029](0029-m10-scoping-execution-explorer.md) | M10 chapter scoping: consolidar sobre foundation pre-existente |
| [0031](0031-m11-scoping-languages-on-demand.md) | M11 chapter scoping: consolidar sobre 7 adapter crates pre-existentes |
| [0032](0032-ops-scoping-production-ready.md) | OPS chapter scoping: consolidar sobre docs + scripts + workflows pre-existentes |
| [0033](0033-ops-support-telemetry.md) | OPS.4 support runbook + telemetry blueprint + health-check contract |
| [0034](0034-chapter-closures-and-m10-followups.md) | Cross-chapter closures + 4 M10 §6 follow-ups: cumulative AUTO+EXEC decisions |
