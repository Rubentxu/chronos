# Reconstrucción Agentic — documento de transición

> **Estado:** activo. Este documento enlaza la nueva documentación que sustituye el roadmap histórico de `01_`, `02_` y `03_`.

La visión "microscopio táctico" se mantiene, pero la arquitectura se refina:

- `ExecutionLog` persistente/append-only en lugar de ring buffer destructivo como verdad;
- proyecciones replayables para call/state/causality;
- evidencia con procedencia y gaps explícitos;
- OpenTelemetry/OBI/runtime instrumentation reutilizados antes de crear probes propias;
- instrumentación semántica temporal y adaptativa;
- Go y Rust como backends de referencia cercanos;
- `chronos-sandbox` preservado como UAT;
- breakpoints relegados a mecanismo interno de escalado;
- MCP simplificado y extraído de la lógica de aplicación.

Empieza en:

- `../reconstruction/00_RECONSTRUCTION_OVERVIEW.md`
- `../roadmap/ROADMAP.md`
- `../roadmap/MILESTONE_ACCEPTANCE.md`
- `../migration/LEGACY_DOCS_DISPOSITION.md`
