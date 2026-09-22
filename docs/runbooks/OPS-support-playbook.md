# OPS support playbook

**Cycle:** OPS / OPS.4 (support runbook + telemetry blueprint)
**Precedence:** `docs/security/H1.2-threat-model.md` §10 (operational guidance); `docs/runbooks/H1.6-install-upgrade-rollback.md` (install/upgrade/rollback)
**Status:** actionable runbook (covers the 5 most common failure modes; not exhaustive)
**Audience:** operators deploying Chronos in `local/stdio` or `linux-privileged` profile.

This playbook documents **what to do when something fails in production**. It is intentionally narrow: 5 cases cover ~80% of real failures. Anything outside this list requires investigation + a new playbook entry.

> **Per ADR-0004 (No Silent Lies)**: every failure mode includes concrete observable signals. If you see the signal, the playbook applies. If you don't, do NOT assume the playbook applies.

---

## §1 Quick reference

| Case | First observable signal | First action |
|---|---|---|
| **[§2] Binary does not start** | `cargo run` exits immediately with non-zero; systemd unit fails | Check §2 |
| **[§3] Capture lost / missing events** | `events_log_read` returns empty / `counterexample` tools timeout | Check §3 |
| **[§4] MCP timeout** | `tools/call` returns `-32603 InternalError` after >30s | Check §4 |
| **[§5] Schema migration fails** | Startup panic containing `SchemaMigrationError` | Check §5 |
| **[§6] OOM kill** | Kernel kills process with `oom-kill` in dmesg; systemd `OOMKilled` | Check §6 |

For each case: **signal → diagnosis → mitigation → escalation**.

---

## §2 Binary does not start

### §2.1 Signals

- `cargo run -p chronos-services` exits immediately with non-zero exit code.
- systemd unit enters `failed` state within 5 seconds.
- Process produces log line `panicked at ...` followed by backtrace.
- Process produces log line `error: ` but no further output.

### §2.2 Diagnosis

1. **Run with `RUST_BACKTRACE=1`** to get full backtrace if panicking:
   ```bash
   RUST_BACKTRACE=1 cargo run -p chronos-services
   ```
2. **Check redb lock**: another instance may be holding the store.
   ```bash
   ls -la ~/.local/share/chronos/store.redb.lock 2>/dev/null
   # If present: another process owns the store; stop it before restarting.
   ```
3. **Check schema_version mismatch**: log line `SchemaMismatchError { ... }`.
4. **Check missing capabilities** (linux-privileged profile only):
   ```bash
   grep -E "^Cap" /proc/self/status
   # Need: CAP_BPF, CAP_SYS_PTRACE (see H1.2 §6.2)
   ```

### §2.3 Mitigation

- **Lock contention**: `kill <other_pid>`; restart the failed service.
- **Schema mismatch**: see §5.
- **Missing capabilities**: restart under a privileged user (or grant capabilities; see H1.2 §6.2).
- **Panic from unknown cause**: collect full backtrace + repro on a non-production host before re-deploying.

### §2.4 Escalation

If none of §2.3 applies: open an incident with:
- Full `RUST_BACKTRACE=1` output.
- `git rev-parse HEAD` of the deployed binary.
- `~/.local/share/chronos/store.redb` size + last-modified timestamp.
- Kernel version + eBPF support (`uname -r`, `ls /sys/kernel/debug/tracing/`).

---

## §3 Capture lost / missing events

### §3.1 Signals

- `events_log_read` returns `{"events": [], "next_cursor": null}` for an active session.
- `counterexample_shrink` returns `-32603 InternalError: probe not responding`.
- `chronos-mcp` log line `ebpf: lost N events in M ms` (rate-limited; check for repeated entries).
- Telemetry exporter shows flat event-rate histogram.

### §3.2 Diagnosis

1. **Verify probe is attached**:
   ```bash
   # MCP: tools/call observe_list (filters session_id)
   # CLI: chronos probe list
   ```
2. **Check kernel eBPF support** (linux-privileged):
   ```bash
   ls /sys/kernel/debug/tracing/
   # If permission denied: kernel not configured for tracing OR user lacks CAP_SYS_ADMIN.
   ```
3. **Check ring buffer pressure** (linux-privileged):
   - Symptom: events dropped under high load.
   - Check kernel log: `dmesg | grep -i "trace"`.
4. **Check redb writer thread**:
   - `chronos-mcp` log line `redb_writer: queue full, dropping N events` indicates writer can't keep up.

### §3.3 Mitigation

- **Probe not attached**: re-attach via `chronos observe create`.
- **eBPF not supported**: deploy to a host with kernel ≥ 5.4 (or fallback to local/stdio profile).
- **Ring buffer pressure**: reduce probe count; increase `events_log_buffer_size` in config.
- **redb writer saturated**: this is a known throughput bottleneck (tracked as `CapRedbWriterBackpressure`). Workaround: reduce concurrent sessions.

### §3.4 Escalation

If capture loss persists at >5% event rate: open an incident with:
- `events_lost_per_minute` metric.
- `chronos-mcp` version + commit SHA.
- Kernel version + eBPF program count.

---

## §4 MCP timeout

### §4.1 Signals

- `tools/call` returns `-32603 InternalError` after >30 seconds.
- `chronos-mcp` log line `mcp: tool '<name>' exceeded timeout`.
- Client (agent) reports `request timeout` after 60 seconds (typical agent retry budget).

### §4.2 Diagnosis

1. **Identify slow tool**: log line includes tool name. Common offenders:
   - `events_log_read` with large `limit` parameter.
   - `counterexample_shrink` with large `max_rounds`.
   - `events_cursor_paginate` with deep pagination.
2. **Check lock contention**: another tool call may hold the redb writer lock.
3. **Check system load**:
   ```bash
   uptime
   # If load > CPU count: reduce concurrent sessions.
   ```

### §4.3 Mitigation

- **Reduce `limit` / `max_rounds`**: split the request into smaller chunks.
- **Backoff + retry**: agent should retry with exponential backoff (1s, 2s, 4s, 8s — max 4 retries).
- **Stuck lock**: as last resort, restart `chronos-mcp` (it releases locks on shutdown).

### §4.4 Escalation

If MCP timeout >50% of requests over 10 minutes: open an incident with:
- Tool histogram (p50/p95/p99 latency per tool).
- `chronos-mcp` version + commit SHA.
- Concurrent session count.

---

## §5 Schema migration fails

### §5.1 Signals

- Startup panic containing `SchemaMigrationError`.
- Log line `Migration: cannot migrate schema_version X to Y`.
- redb corruption: `redb: database is corrupted` panic.

### §5.2 Diagnosis

1. **Identify current schema_version**:
   ```bash
   # Stored as a redb table key.
   # CLI (planned): chronos admin schema version
   ```
2. **Identify deployed binary's schema_version**:
   - Check release notes for the deployed binary.
3. **Compare**: if current > deployed, you rolled forward; if current < deployed, you rolled back without backup.

### §5.3 Mitigation

- **Forward migration (current < deployed)**: deploy intermediate binary versions one at a time.
  - Chronos schema migrations are forward-only (H1.6 §4.2); backwards requires restoring from backup.
- **Backward (current > deployed)**: STOP. Do not run. Restore from backup (see H1.6 §5).
- **Corruption**: STOP. Do not run. Open incident immediately.

### §5.4 Escalation

Schema migration is a one-way door. ANY schema-related failure requires:
- The `~/.local/share/chronos/store.redb` file (preserve for forensics).
- The `chronos-mcp` version + commit SHA.
- The deployed binary version + commit SHA.
- A backup from BEFORE the migration attempt.

---

## §6 OOM kill

### §6.1 Signals

- Kernel log: `oom-kill: ... chronos-mcp ...` in `dmesg`.
- systemd unit enters `failed` with status `OOMKilled`.
- Process exit code `137` (= 128 + SIGKILL signal 9).

### §6.2 Diagnosis

1. **Check memory usage at time of kill**:
   ```bash
   journalctl -u chronos-mcp | grep -i "memory\|rss\|alloc"
   ```
2. **Identify leak**:
   - Common culprit: `events_log_read` with `limit: u64::MAX` (allocates unbounded Vec).
   - Another: `counterexample_shrink` with `max_rounds: u64::MAX`.

### §6.3 Mitigation

- **Immediate**: set memory limit via systemd `MemoryMax=` to prevent total OOM.
- **Short-term**: add per-tool memory budgets (tracked as `CapPerToolMemoryBudget`).
- **Long-term**: fix the unbounded allocation; require explicit pagination.

### §6.4 Escalation

If OOM repeats >3 times in 24 hours with no code change: open an incident with:
- Memory growth curve (if telemetry enabled).
- Recent query patterns (tool name + parameter histogram).
- `chronos-mcp` version + commit SHA.

---

## §7 Out of scope (handled elsewhere)

- **Install/upgrade/rollback**: see `docs/runbooks/H1.6-install-upgrade-rollback.md`.
- **Threat model + OPS.1..OPS.8**: see `docs/security/H1.2-threat-model.md`.
- **Supply chain + SBOM**: see `docs/security/H1.1.1-supply-chain-baseline.md`.
- **Capability matrix + perf budgets**: see `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md`.

---

## §8 References

- ADR-0004 — Silent Lie Prohibition (every signal must be observable; every mitigation must be actionable).
- ADR-0032 — OPS chapter scoping.
- ADR-0033 — OPS.4 support + telemetry (this playbook + `OPS-telemetry-blueprint.md`).
- H1.2 §10 — operational guidance (this playbook extends §10).
- H1.6 — install/upgrade/rollback runbook (cross-referenced from §5).
