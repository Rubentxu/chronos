# Portable Runtime — Architecture Notes

Status: FUTURE / SUPPORTING NOTES

## Control plane vs execution plane

A future portable runtime should preserve one semantic Chronos API while allowing execution placement to vary.

```text
Control Plane
  Agent API / MCP
  session intent
  project/workspace mapping
  capability request
  evidence queries

Execution Plane
  subject process
  probes/tracers
  ExecutionLog producer
  runtime dependencies
```

The Verification Lab is the first place where this split can be proven safely.

## Transport boundary candidates

Do not choose a protocol yet. The seam should first be exercised in-process/local-VM.

Future requirements likely include:

- capability negotiation;
- execution start/stop/cancel;
- live or resumable event transfer;
- artifact upload/download;
- runtime identity/provenance;
- health/liveness;
- durable reconnect where appropriate.

Protocol selection is deferred until local execution-plane separation exists.

## Workspace identity

Avoid protocol coupling to host absolute paths. Prefer logical workspace identity plus explicit materialization metadata.

Potential future materialization mechanisms:

- bind mounts;
- virtiofs;
- git revision + patch bundle;
- content-addressed archive;
- remote cache/artifact store.

## Security direction

Portable execution increases the importance of:

- least privilege;
- explicit network policy;
- read-only inputs where possible;
- disposable runtimes;
- authenticated remote workers;
- no implicit secret forwarding;
- explicit credential/input mounts;
- evidence redaction policy.

No remote-worker implementation should precede a dedicated security ADR.
