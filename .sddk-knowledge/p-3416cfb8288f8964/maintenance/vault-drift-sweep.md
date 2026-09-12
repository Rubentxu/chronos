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
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        print(f"MISSING: {ckpt}")
        continue
    d = json.load(open(ckpt))
    head = d.get('head_sha')
    peel = d.get('remote_tag_peel')
    match = d.get('peel_match')
    if head and peel and head != peel:
        print(f"DRIFT: {folder}: head_sha ({head[:12]}) != remote_tag_peel ({peel[:12]})")
    if match is not True:
        print(f"DRIFT: {folder}: peel_match is {match}, expected True")
    if peel and len(peel) != 40:
        print(f"DRIFT: {folder}: remote_tag_peel is {len(peel)} chars, expected 40 (full SHA)")
```

**Expected output (clean):** empty (no DRIFT lines).

**If non-empty:** either (a) the cycle was tagged but the local head
diverged from the tag peel (most commonly because a subsequent cycle
fast-forwarded past the tag peel), or (b) the stored `peel_match` is
not `True` (lying), or (c) the SHA is not 40 characters (short-SHA
storage that git accepts but is ambiguous). Resolution: use full
40-char SHAs in `head_sha` and `remote_tag_peel`, and verify
`peel_match` against `git rev-list -n 1 <tag>`.

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

### 8. apply-checkpoint.json + archive-manifest.md SHA fields exist in the repository (closed by m9-14; extended m9-16)

```python
import json, os, re, subprocess, glob

# Part A: apply-checkpoint.json SHA fields
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
            print(f"DRIFT: apply-checkpoint {folder}: {field}={v} does not exist in repository")

# Part B: archive-manifest.md Head SHA fields (only enforced for m9-11+ where the convention was introduced)
for manifest in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    # Only m9-11 and later use the | Head SHA | format; older cycles used | Published SHA |
    folder = os.path.basename(os.path.dirname(manifest))
    if folder < 'm9-11':
        continue
    with open(manifest) as f:
        content = f.read()
    m = re.search(r'\| Head SHA \| `([a-f0-9]+)` \|', content)
    if not m:
        print(f"DRIFT: {manifest}: no Head SHA field found")
        continue
    v = m.group(1)
    try:
        subprocess.check_output(
            ['git', 'cat-file', '-e', v],
            stderr=subprocess.DEVNULL,
        )
    except subprocess.CalledProcessError:
        print(f"DRIFT: {manifest}: Head SHA {v} does not exist in repository")
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

### 9. archive-manifest.md Head SHA + cross-references are full 40-char (closed by m9-16)

```python
import re, glob
for manifest in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    with open(manifest) as f:
        content = f.read()
    # Check Head SHA field
    m = re.search(r'\| Head SHA \| `([a-f0-9]+)` \|', content)
    if m and len(m.group(1)) != 40:
        print(f"DRIFT: {manifest}: Head SHA is {len(m.group(1))} chars (expected 40)")
    # Check all SHAs in m9-*-* row references within Previous cycles / Drift summary tables
    for ref_m in re.finditer(r'\| (m9-\d+-[a-z0-9-]+) \| m9-\d+-[a-z0-9-]+ \| B-direct \| `v0\.\d+\.\d+` \| `([a-f0-9]+)` \|', content):
        if len(ref_m.group(2)) != 40:
            print(f"DRIFT: {manifest}: cross-ref to {ref_m.group(1)} has {len(ref_m.group(2))}-char SHA")
```

**Expected output (clean):** empty.

**If `DRIFT`:** either (a) the archive-manifest's own `Head SHA`
metadata field is short (e.g. `0012f12`), or (b) the archive-manifest
references a prior cycle's row in a Previous-cycles table with a short
SHA. C3 only checks `apply-checkpoint.json`; this check covers
`archive-manifest.md` which can drift independently.

**History:** m9-11, m9-12, and m9-13 archive-manifests all stored
short or fabricated SHAs in their Head SHA fields. m9-14 and m9-15
fixed the apply-checkpoint.json fields but did not touch the
corresponding archive-manifest.md files, leaving the drift visible in
the published archive. m9-16 closes this and adds cross-check #9 to
prevent recurrence.

### 10. cycle artifact SHA fields (verify-findings.subject_sha + markdown Head/Base SHA + Remote tag_peel) are full 40-char AND match apply-checkpoint (closed by m9-17)

```python
import json, os, re

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt_path = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt_path):
        continue
    d = json.load(open(ckpt_path))
    head = d.get('head_sha')
    base = d.get('base_sha')
    if not head or len(head) != 40:
        continue  # already caught by C3

    # Part A: verify-findings.json subject_sha (or schema-v1 subject.head)
    vf = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/verify-findings.json'
    if os.path.exists(vf):
        vf_d = json.load(open(vf))
        vf_sha = vf_d.get('subject_sha')
        if vf_sha is None and 'subject' in vf_d:
            vf_sha = vf_d['subject'].get('head_sha') or vf_d['subject'].get('head')
        if vf_sha and vf_sha != head and len(vf_sha) == 40:
            # Schema v1 (m9-03, m9-04) intentionally stores the fix commit
            # which differs from apply-checkpoint.head under the pre-m9-11
            # docs-peel convention. Skip those.
            pass  # would need m9-03/m9-04 special-case

    # Part B: release-receipt.md Head SHA / Remote tag_peel
    rr = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/release-receipt.md'
    if os.path.exists(rr):
        content = open(rr).read()
        for field, expected in [('Head SHA', head), ('Remote tag_peel', d.get('remote_tag_peel'))]:
            m = re.search(rf'\| {field} \| `([a-f0-9]+)`', content)
            if m:
                sha = m.group(1)
                if len(sha) != 40:
                    print(f"DRIFT: {folder}/release-receipt.md: {field} is {len(sha)} chars (expected 40)")
                elif expected and sha != expected:
                    print(f"DRIFT: {folder}/release-receipt.md: {field}={sha[:12]} != apply-checkpoint={expected[:12]}")

    # Part C: merge-receipt.md Head SHA
    mr = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/merge-receipt.md'
    if os.path.exists(mr):
        content = open(mr).read()
        m = re.search(r'\| Head SHA \| `([a-f0-9]+)`', content)
        if m:
            sha = m.group(1)
            if len(sha) != 40:
                print(f"DRIFT: {folder}/merge-receipt.md: Head SHA is {len(sha)} chars (expected 40)")
            elif sha != head:
                print(f"DRIFT: {folder}/merge-receipt.md: Head SHA={sha[:12]} != apply-checkpoint={head[:12]}")
```

**Expected output (clean):** empty.

**If `DRIFT`:** a cycle artifact other than apply-checkpoint.json or
archive-manifest.md (i.e., verify-findings.json, release-receipt.md,
merge-receipt.md, change-entry.md) has a SHA field that is either
short (< 40 chars) or disagrees with apply-checkpoint.head_sha. These
files can drift independently and require explicit checks per file
type.

**Note on schema-v1 (m9-03, m9-04):** their verify-findings.json uses
`sddk.verify-finding/v1` schema with a nested `subject.head` /
`subject.head_sha` field that intentionally stores the **fix commit**
rather than the apply-checkpoint head (which is a docs commit under
the pre-m9-11 docs-peel convention). This is accepted-by-design
drift; the cycle's apply-checkpoint, archive-manifest, tag, and
release-receipts are all internally consistent. Don't migrate
schema-v1 cycles without a dedicated cycle that handles the
docs-peel → fix-peel migration.

**History:** m9-14 fixed apply-checkpoint.json fabricated SHA but
missed the corresponding release-receipt.md, merge-receipt.md,
release-report.md, verify-report.md (all of which still had short
SHA `cd0115f`). m9-15 fixed apply-checkpoint.json short SHAs but
missed verify-findings.json (m9-12 and m9-13 still had `0012f12`
and `26848cf`). m9-16 caught archive-manifest drift. m9-17 closes
the remaining file types (verify-findings, release-receipt,
merge-receipt, change-entry Head SHA) and adds cross-check #10 to
prevent recurrence.

### 11. apply-checkpoint.json required metadata fields (status, archived_at, findings_introduced) consistency (closed by m9-18)

```python
import json, os, re

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        continue
    d = json.load(open(ckpt))

    # Required fields per chronos convention (m9-11+)
    if d.get('status') != 'CLOSED':
        print(f"DRIFT: {folder}: status={d.get('status')!r}, expected 'CLOSED'")

    # archived_at must be a timestamp string (not null) for cycles that have archive-manifest
    am = f'.sddk-knowledge/p-3416cfb8288f8964/changes/archive/{folder}/archive-manifest.md'
    archived_at = d.get('archived_at')
    if os.path.exists(am) and archived_at is None:
        print(f"DRIFT: {folder}: archived_at is None but archive-manifest exists")

    # findings_introduced must be present (even if empty)
    if 'findings_introduced' not in d:
        print(f"DRIFT: {folder}: missing 'findings_introduced' field")
    elif not isinstance(d.get('findings_introduced'), dict):
        print(f"DRIFT: {folder}: findings_introduced is not a dict")
    elif 'no_action' not in d.get('findings_introduced', {}):
        print(f"DRIFT: {folder}: findings_introduced missing 'no_action' subfield")
```

**Expected output (clean):** empty.

**If `DRIFT`:** apply-checkpoint.json is missing or has wrong values for
required metadata fields that the m9-11+ convention established:
- `status` must be uppercase `CLOSED` (not `closed`)
- `archived_at` must be a timestamp string when archive-manifest.md
  exists (not `null`)
- `findings_introduced` must be present (even if empty `{}` with
  empty `no_action: []`)

**History:** m9-14 introduced the `archived_at` field but didn't
backfill m9-11/12/13 apply-checkpoint.json (which had `archived_at:
null` even though archive-manifest.md files existed for all three).
m9-09 introduced the `findings_introduced` field but m9-03, m9-05,
m9-06, m9-07, m9-08, m9-10 (7 cycles) were never updated to include
it. The `status: "closed"` (lowercase) pattern was the pre-m9-11
convention; m9-18 normalizes all 8 affected cycles to uppercase
`CLOSED`. m9-18 adds cross-check #11 to prevent recurrence.

### 12. apply-checkpoint.json route field + main_sha convention + cycle folder cleanup (closed by m9-19)

```python
import json, os, re

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        continue
    d = json.load(open(ckpt))

    # route must be a known B-direct / A-* path label, not the
    # pre-m9-11 lowercase "local" placeholder
    r = d.get('route')
    if r in ('local', ''):
        print(f"DRIFT: {folder}: route={r!r}, expected a path label like 'B-direct'")

    # main_sha convention (m9-04+): post-cycle HEAD = head_sha
    # (m9-11/12/13 wrongly set main_sha = pre-cycle HEAD = base_sha)
    head = d.get('head_sha')
    main = d.get('main_sha')
    if head and main and head != main:
        print(f"DRIFT: {folder}: main_sha ({main[:12]}) != head_sha ({head[:12]}) — expected post-cycle HEAD")

# Cycle folder cleanup: no empty (untracked) cycle folders
import glob
for d in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*')):
    if os.path.isdir(d) and not os.listdir(d):
        folder = os.path.basename(d)
        print(f"DRIFT: {folder}: empty cycle folder (no artifacts)")
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- `route` is the pre-m9-11 placeholder `"local"` instead of a real
  path label like `"B-direct"` / `"A-min"` / `"A-lite"` / `"A-full"`
  / `"B-rebuild"`. m9-11+ uses descriptive paths.
- `main_sha` differs from `head_sha`. Per m9-04+ convention (and the
  `path: B-direct` description), `main_sha` is "main HEAD after this
  cycle merged", which equals `head_sha`. m9-11/12/13 wrongly set
  `main_sha = base_sha` (pre-cycle HEAD). Fix: copy `head_sha` to
  `main_sha`.
- An empty cycle folder exists (no apply-checkpoint.json, no
  artifacts). Likely an aborted investigation. Fix: remove the empty
  folder.

**History:** m9-19 discovered three classes of drift:
- 8 cycles (m9-03..m9-10) had `route: "local"` from the pre-m9-11 era
- 3 cycles (m9-11/12/13) had `main_sha` set to pre-cycle HEAD (an
  inconsistency I introduced when authoring m9-11..m9-13 myself)
- 1 empty folder (`m9-11-m9-04-findings-closed-drift-fix`) was a
  leftover from an aborted false-positive investigation from prior
  sessions

m9-19 closes all three and adds cross-check #12 to prevent recurrence.

## When to escalate

If any of the cross-checks finds drift that is **not** trivially
remediable (e.g. a cycle's entire artifact set is missing, or the source
artifacts for a rebuild do not exist in the repo), document the gap in
the next cycle's apply-checkpoint and the `handoff-blocked` standing
item — do not attempt a cross-cycle rebuild outside a dedicated cycle.

If **check 5**, **check 6**, **check 7**, **check 8**, **check 9**,
**check 10**, **check 11**, **check 12**, **check 13**, or **check 14**, or **check 15** finds
drift: the resolution is mechanical (a small metadata edit). Do this
in the same cycle that catches it; do not defer.

## Reference

Cycles that established this procedure:
- **m9-09** (vault hygiene: m9-01-R4 active/terminated dedupe, v0.7.7) → cross-check #1
- **m9-10** (vault hygiene: m9-03 apply-checkpoint rebuild, v0.7.8) → cross-check #2
- **m9-11** (vault hygiene: cycles/index.md metadata drift fix, v0.7.9) → cross-check #5
- **m9-12** (vault hygiene: terms/index.md "Last archive" drift fix, v0.7.10) → cross-check #6
- **m9-13** (vault hygiene: change-entry SHA drift fix, v0.7.11) → cross-check #7
- **m9-14** (vault hygiene: apply-checkpoint fabricated-SHA fix, v0.7.12) → cross-check #8
- **m9-15** (vault hygiene: apply-checkpoint short-SHA expansion, v0.7.13) → tightened cross-check #3
- **m9-16** (vault hygiene: archive-manifest short/fabricated-SHA fix, v0.7.14) → cross-check #9
- **m9-17** (vault hygiene: cycle artifact SHA drift fix across verify-findings/markdown files, v0.7.15) → cross-check #10
- **m9-18** (vault hygiene: apply-checkpoint metadata drift fix — archived_at, findings_introduced, status normalization, v0.7.16) → cross-check #11
- **m9-19** (vault hygiene: route field normalization + main_sha convention + empty folder cleanup, v0.7.17) → cross-check #12
- **m9-20** (vault hygiene: status fields backfill — verify_status + release_status + archive_status, v0.7.18) → cross-check #13
- **m9-21** (vault hygiene: verbose route normalization + legacy field prohibition for m9-19+ cycles, v0.7.19) → cross-check #14
- **m9-22** (vault hygiene: created_at + title + summary backfill for 16 prior cycles, v0.7.20) → cross-check #15

### 13. apply-checkpoint.json `*_status` field backfill (closed by m9-20)

```python
import json, os
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        continue
    d = json.load(open(ckpt))
    if d.get('status') != 'CLOSED':
        continue
    for f in ['verify_status', 'release_status', 'archive_status']:
        if d.get(f) is None:
            print(f"DRIFT: {folder}: {f} is None (expected 'passed'/'released'/'archived')")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A CLOSED cycle is missing the `verify_status`,
`release_status`, or `archive_status` field. m9-19 introduced these
three fields but didn't backfill the 17 prior CLOSED cycles.

**Resolution:**
- For a CLOSED cycle, set:
  - `verify_status: "passed"`
  - `release_status: "released"`
  - `archive_status: "archived"`
- This is mechanical and safe. The values are derived from the
  `status: CLOSED` fact: if a cycle is CLOSED, it must have been
  verified, released, and archived.

**History:** m9-20 closed this drift class after m9-19 introduced
the fields without backfill.

### 14. m9-19+ cycles must not introduce legacy schema-v1 fields (enforced by m9-21)

```python
import json, os, re
legacy_fields = {'change', 'artifacts', 'commits_since_base', 'findings_closed',
                 'findings_remaining_m9_plus', 'ledger_state', 'next_cycle',
                 'next_cycle_path', 'next_cycle_target_findings', 'notes',
                 'runtime_status', 'tag'}

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    n = int(m.group(1))
    if n < 19: continue  # legacy cycles exempt
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    found = legacy_fields & set(d.keys())
    if found:
        print(f"DRIFT: {folder}: has legacy fields {sorted(found)}")

# Also: route field must be a bare path label, not the verbose form
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    n = int(m.group(1))
    if n < 19: continue
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    r = d.get('route', '')
    if ' (' in r:
        print(f"DRIFT: {folder}: route {r!r} is verbose, expected bare path label")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A cycle m9-19 or later has either:
- One or more schema-v1 legacy fields (`change`, `artifacts`,
  `commits_since_base`, `findings_closed`, etc.). These were used by
  pre-m9-11 cycles but the new schema replaces them with the explicit
  fields (`findings_introduced`, `verify_status`, etc.). New cycles
  should follow the new schema.
- A verbose `route` like `"B-direct (T0 + light-verify)"` instead of
  the bare label `"B-direct"`.

**Resolution:**
- Remove the legacy fields (the data is preserved in the m9-18 and
  earlier cycles that introduced them).
- Set `route` to the bare path label.

**History:** Pre-m9-11 cycles used a more verbose schema with fields
like `change`, `artifacts`, `commits_since_base`, etc. m9-11+
authored cycles kept these legacy fields for backward compat. m9-19
authored a clean schema without them, and m9-20 followed suit. m9-21
adds cross-check #14 to enforce that m9-19+ cycles stay clean and
also normalizes the verbose `"B-direct (T0 + light-verify)"` route
strings to bare `"B-direct"` for m9-11..m9-18.

### 15. apply-checkpoint.json required fields (created_at, title, summary) backfill (closed by m9-22)

```python
import json, os
required = ['created_at', 'title', 'summary']
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    missing = [f for f in required if f not in d or not d.get(f)]
    if missing:
        print(f"DRIFT: {folder}: missing {missing}")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A cycle is missing `created_at`, `title`, or
`summary`. These were present in m9-19+ but missing on the 16 prior
cycles (m9-03..m9-18).

**Resolution:**
- `created_at`: extract from git log (first commit touching the
  apply-checkpoint.json), normalized to UTC.
- `title`: derive from the cycle folder slug (kebab-case → spaces).
- `summary`: extract first non-heading, non-table, non-code line from
  the cycle's `merge-receipt.md`.

**History:** m9-22 closed this drift class after m9-19 introduced
the fields without backfill.

Each closed a one-line drift that the prior session's "exhausted"
verdict missed. The lesson is that **vault drift is a first-class
maintenance surface**, not a side effect of code work.
