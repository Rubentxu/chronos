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
