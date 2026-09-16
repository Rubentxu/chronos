# SANDBOX-S0.2 (part 1) — cache hardening and contract namespace

Two follow-ups identified during the S0.1c review, closed here.

## 1. The QEMU cache is now content-addressed

Before:

```text
cache/
  rootfs/            <- reused whenever the directory existed
  s0-base.cpio.gz    <- same
```

That permitted:

```text
image A -> build cache ; image becomes B ; cache A reused
```

a Silent Lie: the run would report provenance for B while executing A's
filesystem.

Now:

```text
cache/
  <key>/
    rootfs/
    base.cpio.gz
    manifest.json
```

with

```text
key = sha256(schema_version + image_immutable_id + init_script_sha256 + arch)
```

and `manifest.json` recording `schema`, `source_image`, `source_image_id`,
`init_sha256`, `architecture`, `base_sha256`, `cache_key`. Every entry is
validated before reuse; a mismatch is a cache MISS, never a heuristic reuse. If
the image is not present locally the builder refuses (no pulling).

Verified: a fresh run creates the CAS entry and passes; a second run reuses it;
cleanup leaves no per-run artifacts behind.

## 2. The contract namespace is `chronos.execution.*`

```text
chronos.execution.scenario/v1
chronos.execution.result/v1
```

Previously `sddk.sandbox.*`. SDDK is the workflow that governs development; the
contract belongs to the product. Renaming now, before any consumer exists,
avoids ever migrating an external API from `sddk.*` to `chronos.*`.
`chronos.execution.*` was chosen over `chronos.sandbox.*` because the same
contract should serve the future Portable Execution Runtime, which is not a
sandbox.

All four backends still pass with the renamed schema.

## 3. Naming the KERNEL class honestly

QEMU currently boots the **host's** kernel image
(`/usr/lib/modules/<release>/vmlinuz`), with its SHA recorded in provenance.
That is `KERNEL / host-derived guest`, not cross-host kernel reproducibility:

```text
host A -> kernel SHA A
host B -> kernel SHA B
```

A pinned kernel artifact belongs to S0.7 or later. Until then this is stated
rather than implied away.

## Still open in S0.2

Formal contract set: scenario / result / capability / workspace / provenance /
lifecycle, plus removing `collect()` from the `ExecutionEnvironment` draft.
