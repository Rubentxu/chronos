# Standing Maintenance: Vault Drift Sweep

## Purpose

Capture the lessons from four consecutive sessions (2026-09-12, cycles m9-07
through m9-10) where each session declared "auto-mode exhausted" and the next
session found more drift to clean. The recurring gap was that **code-grep
returns no actionable findings** is not the same as **auto-mode exhausted**;
the audit vault is part of the repo and has its own drift surface.

## Trigger

Run the sweep at the **start of any session that is about to declare
auto-mode exhausted**. Before claiming exhaustion:

1. Code-level sweep (already standard): `grep -rn` for `FIND-M9-*` IDs in
   the codebase, check that all are either open findings or closed via a
   released cycle.
2. **Vault-level sweep (this procedure)**: run the four cross-checks below.

## Cross-checks

### 1. Vault ID uniqueness (closed by m9-09)

```bash
awk -F'|' '/^### /{section=$0; next} /^\| m9/{gsub(/^[ \t]+/, "", $2); print $2}' \
    .sddk-knowledge/p-3416cfb8288f8964/terms/index.md \
    | sort | uniq -d
```

**Expected output (clean):** empty.

**If non-empty:** an ID appears in two `### ` sections (typically one
Active + one Terminated). The duplicate was created when a finding was
terminated in one section but the row in another section was not removed.
Resolution: keep the Terminated row, remove the duplicate from Active.

### 2. findings_closed ↔ Terminated terms cross-check (closed by m9-10)

```python
import json, os

# Collect closures from all apply-checkpoint.json files
closed = set()
for folder in os.listdir('cycle-artifacts/p-3416cfb8288f8964/'):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        continue
    d = json.load(open(ckpt))
    for fid in d.get('findings_closed', []):
        closed.add(fid)
    # findings_introduced.no_action counts as closed by no-action
    for fid in d.get('findings_introduced', {}).get('no_action', []):
        closed.add(fid)

# Collect terminated IDs from terms/index.md
terminated = set()
in_section = False
with open('.sddk-knowledge/p-3416cfb8288f8964/terms/index.md') as f:
    for line in f:
        if '## Terminated terms' in line: in_section = True; continue
        if line.startswith('## '): in_section = False; continue
        if in_section and line.startswith('|'):
            cols = [c.strip() for c in line.split('|')]
            if len(cols) > 2 and cols[1] and cols[1] != 'ID':
                terminated.add(cols[1])

# Report
print(f"In apply-checkpoints (incl. no_action) but NOT in terminated: {closed - terminated}")
print(f"In terminated but NOT in apply-checkpoints: {terminated - closed}")
```

**Expected output (clean):** empty on the left side; right side may show
`m8-*-R*` (pre-vault-reorg cycles that never had an apply-checkpoint in
this checkout).

**If non-empty on the left:** an apply-checkpoint claims a closure that
the vault does not record. Resolution: either the closure is real and
needs a `Terminated terms` row, or the apply-checkpoint is over-claiming.

**If non-empty on the right:** a vault `Terminated terms` row refers to a
cycle whose apply-checkpoint is missing. Resolution: rebuild the
apply-checkpoint from the cycle's pre-existing merge-receipt, release-
receipt, release-report, verify-report (m9-10 pattern). Note that pre-
vault-reorg rows (m8-04-R4, m8-07-R2) cannot be rebuilt because the
source artifacts are not in this repo.

### 3. apply-checkpoint ↔ tag consistency

```python
import json, os
for folder in os.listdir('cycle-artifacts/p-3416cfb8288f8964/'):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        print(f"MISSING: {ckpt}")
        continue
    d = json.load(open(ckpt))
    head = d.get('head_sha')
    peel = d.get('remote_tag_peel')
    match = d.get('peel_match')
    archive_manifest_sha = None  # would require parsing archive-manifest.md
    print(f"{folder}: head={head[:8]} peel={peel[:8] if peel else None} match={match}")
```

**Expected output (clean):** all `peel_match=True`, all `head_sha` equals
`remote_tag_peel`.

**If non-empty:** the cycle was tagged but the local head diverged from
the tag peel (most commonly because a subsequent cycle fast-forwarded past
the tag peel — though chronos convention peels to the fix-commit which is
typically a parent of the docs-commit, so this is usually fine).

### 4. SHA-256 consistency in archive-manifest Artifact index

```bash
for manifest in .sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md; do
  awk '/^## Artifact index/,0' "$manifest" \
    | awk '/^\| / && $4 ~ /^[a-f0-9]{64}$/ {gsub(/`/, "", $2); gsub(/`/, "", $4); print $2 "|" $4}' \
    | while IFS='|' read path sha; do
        [ -f "$path" ] && [ "$(sha256sum "$path" | cut -d' ' -f1)" != "$sha" ] \
          && echo "DRIFT: $manifest :: $path"
      done
done
```

**Expected output (clean):** empty.

**If non-empty:** the SHA-256 listed in the Artifact-index block of an
archive-manifest no longer matches the actual file. Resolution:
recompute the SHA-256 and update the manifest. Note: do **not** confuse
this with the SHA-256 listed in the "Evidence bindings" block of older
manifests (m9-01, m9-02) which use the literal word "pending" as a
placeholder before the SHA is captured; those are intentionally not in
the Artifact-index format.

### 5. cycles/index.md metadata consistency (closed by m9-11)

```bash
actual=$(awk -F'|' '/^\| (m6 |m[7-9])/{c++} END{print c+0}' .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
declared=$(awk -F'|' '/Total cycles/{gsub(/[ \t]+/, "", $3); print $3}' .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
[ "$actual" = "$declared" ] && echo "OK: $actual == $declared" || echo "DRIFT: actual=$actual declared=$declared"
```

**Expected output (clean):** `OK: <n> == <n>`.

**If `DRIFT`:** the `Total cycles` metadata field in `cycles/index.md`
diverges from the actual data-row count. Resolution: bump the field to
the actual count, update `Last updated`. Resolution is mechanical
(1-character edit). **History:** introduced when m9-07..m9-10 added
rows without bumping the counter; first caught in the m9-11 cycle by
this very procedure (drift of 4 cycles, 26 actual vs 22 declared).

### 6. terms/index.md "Last archive" ↔ cycles/index.md most-recent-cycle consistency (closed by m9-12)

```bash
last_archive_in_terms=$(awk -F'|' '/Last archive/{gsub(/[ \t]+/, "", $3); print $3}' \
  .sddk-knowledge/p-3416cfb8288f8964/terms/index.md)
last_closed_cycle=$(awk -F'|' '/^\| m[0-9]+/{gsub(/[ \t]+/, "", $3); last=$3} END {print last}' \
  .sddk-knowledge/p-3416cfb8288f8964/cycles/index.md)
[ "$last_archive_in_terms" = "$last_closed_cycle" ] \
  && echo "OK: $last_archive_in_terms == $last_closed_cycle" \
  || echo "DRIFT: terms=$last_archive_in_terms cycles=$last_closed_cycle"
```

**Expected output (clean):** `OK: <cycle-id> == <cycle-id>`.

**If `DRIFT`:** the `Last archive` field in `terms/index.md` is pointing
at a previous cycle (or even further behind), but a more recent cycle
has been recorded in `cycles/index.md`. This happens when an archive
cycle adds itself to `cycles/index.md` but forgets to bump the
`Last archive` pointer in `terms/index.md`.

Resolution: bump `Last archive` to the most-recent closed cycle in
`cycles/index.md`, and bump `Last updated` to the current cycle time.

**History:** m9-11 added itself to `cycles/index.md` but did not bump
`terms/index.md`'s `Last archive` (was pointing at m9-10, should have
been m9-11). m9-12 closes this drift and adds cross-check #6 to prevent
recurrence.

### 7. change-entry.md Head/Base SHA ↔ apply-checkpoint.json head_sha/base_sha consistency (closed by m9-13)

```python
import json, os, re
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    ce = f'.sddk-knowledge/p-3416cfb8288f8964/changes/{folder}/change-entry.md'
    if not os.path.exists(ckpt) or not os.path.exists(ce): continue
    d = json.load(open(ckpt))
    with open(ce) as f: content = f.read()
    head_m = re.search(r'\| Head SHA \| `([a-f0-9]+)`', content)
    base_m = re.search(r'\| Base SHA \| `([a-f0-9]+)', content)
    if head_m and d.get('head_sha') and not d['head_sha'].startswith(head_m.group(1)):
        print(f"DRIFT: {folder}: head SHA {head_m.group(1)} not a prefix of apply-checkpoint head_sha {d['head_sha'][:12]}")
    if base_m and d.get('base_sha') and not d['base_sha'].startswith(base_m.group(1)):
        print(f"DRIFT: {folder}: base SHA {base_m.group(1)} not a prefix of apply-checkpoint base_sha {d['base_sha'][:12]}")
```

**Expected output (clean):** empty.

**If `DRIFT`:** the change-entry's `Base SHA` or `Head SHA` field
disagrees with the corresponding field in `apply-checkpoint.json`. This
is usually a copy-paste error in the change-entry. Resolution: update
the change-entry to match the apply-checkpoint.

**History:** m9-11's change-entry was authored with `Base SHA = cd0115f`
(the fix commit), but the apply-checkpoint correctly had
`base_sha = 6120e98` (the parent of the fix). m9-13 catches this
drift and adds cross-check #7 to prevent recurrence.

### 8. apply-checkpoint.json SHA fields exist in the repository (closed by m9-14)

```python
import json, os, subprocess
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        continue
    d = json.load(open(ckpt))
    for field in ['head_sha', 'base_sha', 'main_sha', 'remote_tag_peel']:
        v = d.get(field)
        if not v:
            continue
        try:
            subprocess.check_output(
                ['git', 'cat-file', '-e', v],
                stderr=subprocess.DEVNULL,
            )
        except subprocess.CalledProcessError:
            print(f"DRIFT: {folder}: {field}={v} does not exist in repository")
```

**Expected output (clean):** empty.

**If `DRIFT`:** an SHA in the apply-checkpoint is not a real commit in
the repository. This is a fabrication error: the agent writing the
checkpoint guessed an SHA instead of looking it up via
`git rev-list -n 1 <short-sha>` or `git rev-parse <ref>`. The fix is
mechanical: replace the fabricated SHA with the real one, then verify
`peel_match` is still correct by re-checking the tag.

**History:** m9-11's `apply-checkpoint.json` was written with
`head_sha = cd0115f8c93bddcae06e5a57f4e7e91d3a4fbb33` — a 40-character
hex string that looks plausible but does not exist in the repository.
The real m9-11 fix commit is `cd0115fd8f942058cde109c72a975cab7ea7473c`
(verified via `git rev-list -n 1 cd0115f`). m9-14 catches this and adds
cross-check #8 to prevent recurrence. Note that `peel_match: true` in
m9-11 was also a lie: `remote_tag_peel` did not equal the actual tag
peel; the corrected values match `git rev-list -n 1 v0.7.9`.

## When to escalate

If any of the cross-checks finds drift that is **not** trivially
remediable (e.g. a cycle's entire artifact set is missing, or the source
artifacts for a rebuild do not exist in the repo), document the gap in
the next cycle's apply-checkpoint and the `handoff-blocked` standing
item — do not attempt a cross-cycle rebuild outside a dedicated cycle.

If **check 5**, **check 6**, **check 7**, or **check 8** finds drift: the
resolution is mechanical (a small metadata edit). Do this in the same
cycle that catches it; do not defer.

## Reference

Cycles that established this procedure:
- **m9-09** (vault hygiene: m9-01-R4 active/terminated dedupe, v0.7.7) → cross-check #1
- **m9-10** (vault hygiene: m9-03 apply-checkpoint rebuild, v0.7.8) → cross-check #2
- **m9-11** (vault hygiene: cycles/index.md metadata drift fix, v0.7.9) → cross-check #5
- **m9-12** (vault hygiene: terms/index.md "Last archive" drift fix, v0.7.10) → cross-check #6
- **m9-13** (vault hygiene: change-entry SHA drift fix, v0.7.11) → cross-check #7
- **m9-14** (vault hygiene: apply-checkpoint fabricated-SHA fix, v0.7.12) → cross-check #8

Each closed a one-line drift that the prior session's "exhausted"
verdict missed. The lesson is that **vault drift is a first-class
maintenance surface**, not a side effect of code work.
