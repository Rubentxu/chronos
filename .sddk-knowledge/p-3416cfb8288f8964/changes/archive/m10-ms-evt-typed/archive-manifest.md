# Archive Manifest — m10-ms-evt-typed

## Summary

B-direct slice per ADR-0003: typed event filter wire (parse-time
rejection of unknown event types) and single-owner snake_case mapping in
`EventType::from_snake_case` (21 variants). OCP closed on `EventType`:
adding a variant now requires only the enum.

## Identification

| Field | Value |
|---|---|
| Date | 2026-09-14 |
| Path | B-direct |
| Base SHA | `9078ec3136faf07704ff4845d29a901348744988` |
| Head SHA | `0998aa734541e6675ed1645745e2b26060a8a645` |
| Main merge SHA | `0998aa734541e6675ed1645745e2b26060a8a645` |
| Tag | `v0.7.102` |
| Tag peel | `0998aa734541e6675ed1645745e2b26060a8a645` |

## Evidence bindings

- `crates/chronos-domain/src/trace/event.rs`: `from_snake_case`.
- verify-report.md verdict PASS; ADR acceptance grep == 0.
