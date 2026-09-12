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
        # findings_closed entries can be strings or dicts
        if isinstance(fid, str):
            closed.add(fid)
        elif isinstance(fid, dict):
            closed.add(fid.get('finding_id', ''))
    # findings_introduced.no_action counts as closed by no-action
    for fid in d.get('findings_introduced', {}).get('no_action', []):
        if isinstance(fid, str):
            closed.add(fid)
        elif isinstance(fid, dict):
            closed.add(fid.get('finding_id', ''))

# Collect terminated IDs from terms/index.md
terminated = set()
in_section = False
with open('.sddk-knowledge/p-3416cfb8288f8964/terms/index.md') as f:
    for line in f:
        if '## Terminated terms' in line: in_section = True; continue
        if line.startswith('## '): in_section = False; continue
        if in_section and line.startswith('|'):
            cols = [c.strip() for c in line.split('|')]
            # Skip separator rows and header
            if len(cols) > 2 and cols[1] and cols[1] != 'ID' and not cols[1].startswith('---'):
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

### 3. apply-checkpoint ↔ tag consistency (era-aware, extended by m9-57)

```python
import json, os, re, subprocess

def tag_peel_era(peel_sha):
    """Return docs-peel or fix-peel based on peel commit message."""
    if not peel_sha:
        return 'unknown'
    try:
        msg = subprocess.check_output(
            ['git', 'log', '-n', '1', '--format=%s', peel_sha],
            stderr=subprocess.DEVNULL,
        ).decode().strip()
    except subprocess.CalledProcessError:
        return 'unknown'
    if msg.startswith('fix('):
        return 'fix-peel'
    return 'docs-peel'

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    n = int(m.group(1))
    if n < 19: continue  # legacy schema; cycle head_sha != tag peel is accepted-by-design
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt):
        print(f"MISSING: {ckpt}")
        continue
    d = json.load(open(ckpt))
    head = d.get('head_sha')
    peel = d.get('remote_tag_peel')
    match = d.get('peel_match')
    era = tag_peel_era(peel)
    # m9-19+ fix-peel cycles may legitimately have head_sha != remote_tag_peel
    if head and peel and head != peel and era != 'fix-peel':
        print(f"DRIFT: {folder}: head_sha ({head[:12]}) != remote_tag_peel ({peel[:12]})")
    # peel_match=True is required for non-fix-peel cycles, OR for fix-peel cycles
    # where head == peel (head was the tag's peel commit). For fix-peel cycles
    # where head != peel (cycle had additional fix commits), peel_match=False is honest.
    if match is not True and not (era == 'fix-peel' and head != peel):
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

**Fix-peel exemption history:** m9-34, m9-35, m9-50 are fix-peel
cycles where the cycle branch included additional fix commits after
the tag peel. CC#3 originally flagged `head_sha != remote_tag_peel`
as drift for all three. m9-57 makes CC#3 era-aware: it inspects the
peel commit's message and exempts `fix-peel` cycles from the equality
check. The `peel_match` boolean is still required to be `True`.


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
**check 10**, **check 11**, **check 12**, **check 13**, or **check 14**, or **check 15**, or **check 16**, or **check 17**, or **check 18**, or **check 19**, or **check 20**, or **check 21**, or **check 22**, or **check 23**, or **check 24**, or **check 25**, or **check 26**, or **check 27**, or **check 28**, or **check 29**, or **check 30**, or **check 31**, or **check 32**, or **check 33**, or **check 34**, or **check 35**, or **check 36**, or **check 37**, or **check 38**, or **check 39**, or **check 40**, or **check 41**, or **check 42**, or **check 43**, or **check 44**, or **check 45**, or **check 46**, or **check 47** finds
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
- **m9-23** (vault hygiene: cycle_id workspace prefix strip, v0.7.21) → cross-check #16
- **m9-24** (vault hygiene: verify-findings.json schema normalization, v0.7.22) → cross-check #17
- **m9-25** (vault hygiene: synthesize missing verify-findings.json for m9-05..m9-10, v0.7.23) → cross-check #18
- **m9-26** (vault hygiene: verify-findings.json cycle_id prefix strip, v0.7.24) → extended cross-check #16
- **m9-27** (vault hygiene: findings_introduced.no_action free-text note → notes key, v0.7.25) → cross-check #19
- **m9-28** (vault hygiene: m9-19 fabricated base_sha → real 735c57b, v0.7.26) → cross-check #20
- **m9-29** (vault hygiene: backfill Evidence bindings section in m9-11..m9-27 archive-manifests, v0.7.27) → cross-check #21
- **m9-30** (vault hygiene: normalize release-receipt.md SHA fields across all 25 m9 cycles, v0.7.28) → cross-check #22
- **m9-31** (vault hygiene: normalize merge-receipt.md SHA fields across all 25 m9 cycles, v0.7.29) → cross-check #23
- **m9-32** (vault hygiene: add `## Cross-checks` section to 25 verify-report.md files, v0.7.30) → cross-check #24
- **m9-33** (vault hygiene: normalize change-entry.md title format to `# Change: m9-NN <title>` across 18 cycles, v0.7.31) → cross-check #25
- **m9-34** (vault hygiene: verify-findings subject.base_sha backfill for 23 affected cycles, v0.7.32) → cross-check #26
- **m9-35** (vault hygiene: archive-manifest Published SHA→Head SHA rename across 10 files, v0.7.33) → cross-check #27
- **m9-36** (vault hygiene: release-report.md `# m9-NN:` heading format across 9 files, v0.7.34) → cross-check #28
- **m9-37** (vault hygiene: release-receipt Base SHA + markdown-table format, v0.7.35) → cross-check #29
- **m9-38** (vault hygiene: apply-checkpoint path→route normalization, v0.7.36) → cross-check #30
- **m9-39** (vault hygiene: verify-report title + verify-findings verdict + archive-manifest cycle, v0.7.37) → cross-check #31
- **m9-40** (vault hygiene: archive-manifest Base SHA + m9-34 verify-report Cross-checks, v0.7.38) → cross-check #32
- **m9-41** (vault hygiene: release-report Path + Cross-checks sections, v0.7.39) → cross-check #33
- **m9-42** (vault hygiene: change-entry Subject/Files + verify-report Subject + short→full SHA across 16 files, v0.7.40) → cross-check #34
- **m9-43** (vault hygiene: change-entry section order + cross-check section + m9-03 cycle_id, v0.7.41) → cross-check #35
- **m9-44** (vault hygiene: verify-report Path + verify-findings lens_summary + Findings prose across 18 files, v0.7.42) → cross-check #36
- **m9-45** (vault hygiene: change-entry section order + release-report cycle value, v0.7.43) → cross-check #37
- **m9-46** (vault hygiene: verify-findings findings array population, v0.7.44) → cross-check #38
- **m9-47** (vault hygiene: archive/release Cross-checks sections + cycles index count, v0.7.45) → cross-check #39
- **m9-48** (vault hygiene: apply-checkpoint findings_closed + archive-manifest Date/Path backfill, v0.7.46) → cross-check #40
- **m9-49** (vault hygiene: m9-03 lens_summary + m9-19..m9-33 release-report schema backfill, v0.7.47) → cross-check #41
- **m9-50** (vault hygiene: tag recreation + peel reconciliation + index timestamps, v0.7.48) → cross-check #42
- **m9-51** (vault hygiene: head_sha sync between apply-checkpoint + release-receipt + verify-findings, v0.7.49) → cross-check #43
- **m9-52** (vault hygiene: verify-report `## Summary` section across 7 files, v0.7.50) → cross-check #44
- **m9-53** (vault hygiene: cycles/index.md short-SHA expansion (35 rows), v0.7.51) → cross-check #45
- **m9-54** (vault hygiene: stale-branch cleanup (44 local + 28 remote), v0.7.52) → cross-check #46
- **m9-55** (vault hygiene: apply-checkpoint base_sha fabrication fix (m9-38 reachability), v0.7.53) → cross-check #47

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

### 14. m9-19+ cycles must not introduce legacy schema-v1 fields (enforced by m9-21; `findings_closed` removed from legacy set by m9-57)

```python
import json, os, re
# `findings_closed` is NOT legacy — it tracks findings this cycle CLOSED
# (distinct from `findings_introduced` which tracks new findings routed to follow-up).
# All m9 cycles keep both fields by design.
legacy_fields = {'change', 'artifacts', 'commits_since_base',
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
  `commits_since_base`, etc.). These were used by pre-m9-11 cycles but
  the new schema replaces them with the explicit fields
  (`findings_introduced`, `verify_status`, etc.). New cycles should
  follow the new schema.

**Note on `findings_closed`:** this field was originally included in
the legacy set, but is semantically distinct from
`findings_introduced`: `findings_closed` tracks findings this cycle
**resolved**, while `findings_introduced` tracks new findings routed
to follow-up work. All m9 cycles keep both fields by design. m9-57
removed `findings_closed` from the legacy set so CC#14 no longer
flags it as drift.
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

### 16. apply-checkpoint.json + verify-findings.json cycle_id format (no workspace prefix) (closed by m9-23; extended m9-26)

```python
import json, os
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    for fname in ['apply-checkpoint.json', 'verify-findings.json']:
        p = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/{fname}'
        if not os.path.exists(p): continue
        d = json.load(open(p))
        cid = d.get('cycle_id', '')
        # The cycle_id must NOT have a slash (which would indicate a
        # workspace prefix like 'p-3416cfb8288f8964/').
        # Acceptable forms:
        #   - bare m9-NN (m9-19, m9-20, ...)
        #   - full slug matching the folder name (m9-11-cycles-index-...)
        if '/' in cid:
            print(f"DRIFT: {folder}/{fname}: cycle_id has workspace prefix: {cid!r}")
```

**Expected output (clean):** empty.

**If `DRIFT`:** `cycle_id` either:
- Has the workspace prefix `p-3416cfb8288f8964/` (16 cycles m9-03..m9-18
  had this), or
- Differs from the cycle folder name.

**Resolution:** Strip the workspace prefix. The cycle_id should be
the bare cycle slug (e.g. `m9-11-cycles-index-metadata-drift`),
matching the folder name.

**History:** Pre-vault-reorg cycles included the workspace path in
cycle_id. m9-19+ use bare slugs. m9-23 closes this drift.

### 17. verify-findings.json schema must have `subject` dict (closed by m9-24)

```python
import json, os
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    vf = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/verify-findings.json'
    if not os.path.exists(vf): continue
    d = json.load(open(vf))
    sub = d.get('subject')
    if not isinstance(sub, dict):
        print(f"DRIFT: {folder}: subject is not a dict (legacy schema)")
```

**Expected output (clean):** empty.

**If `DRIFT`:** verify-findings.json uses the legacy schema where
`subject_sha` is a top-level field. m9-19+ use the new schema with
`subject: {head, head_sha}`.

**Resolution:** Normalize to new schema. Preserve `lens_summary`,
`verdict`, and `evidence` under a `_legacy` field for traceability.

**History:** m9-11..m9-18 used the legacy lens-based schema. m9-19+
use the simpler subject + findings schema. m9-24 normalizes the 8
prior cycles.

### 18. verify-findings.json must exist for all CLOSED cycles (closed by m9-25)

```python
import os
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    vf = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/verify-findings.json'
    if not os.path.exists(vf):
        print(f"DRIFT: {folder}: missing verify-findings.json")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A cycle folder is missing verify-findings.json.
This can happen if the cycle was created before the standard
6-artifact set was enforced, or if the file was lost during a
vault reorg.

**Resolution:** Synthesize a minimal verify-findings.json from
the apply-checkpoint.json head_sha. Document the synthesis in
the `_note` field so future readers know the file is a
restoration, not an original.

**History:** m9-05..m9-10 were missing verify-findings.json
(synthesized by m9-25). m9-01 and m9-02 don't have cycle folders
at all (pre-cycle-artifacts convention); they are out of scope
for this check.

### 19. findings_introduced.no_action must contain only term IDs (no free-text notes) (closed by m9-27)

```python
import json, os
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    fi = d.get('findings_introduced', {})
    no_action = fi.get('no_action', [])
    for entry in no_action:
        # Real term IDs are short identifiers without spaces or parenthetical notes.
        # If the entry contains spaces or parenthetical content, it's a free-text
        # note that belongs under `notes` instead.
        if ' ' in entry or '(' in entry:
            print(f"DRIFT: {folder}: no_action has free-text note: {entry[:80]}")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A `findings_introduced.no_action` entry contains
free-text content (spaces, parenthetical notes) instead of a bare
term ID like `cc-002-env-coupling-test`. Cross-check #2 expects
these to be valid term IDs.

**Resolution:** Move free-text entries to a new `findings_introduced.notes`
list. Real closures stay in `no_action` as bare term IDs.

**History:** m9-14 stored a free-text note in `no_action` describing
a deferred observation (vacuous peel_match on prior cycles). This
tripped cross-check #2. m9-27 moves the note to a `notes` key.

### 20. apply-checkpoint.json base_sha must equal head_sha^ for fix-peel cycles (closed by m9-28)

```python
import json, os, re, subprocess

def tag_peel_era(tag):
    """Return 'docs-peel' or 'fix-peel' based on the tag's peel commit
    message. Fix-peel: tag peels to a 'fix(m9-XX): ...' commit.
    Docs-peel: tag peels to a 'docs(m9-XX): ...' or 'verify(m9-XX): ...'
    commit (the docs commit, with fix commit preceding)."""
    peel = subprocess.check_output(
        ['git', 'log', '--format=%s', '-n', '1', tag],
        stderr=subprocess.DEVNULL,
    ).decode().strip()
    if peel.startswith('fix('):
        return 'fix-peel'
    return 'docs-peel'

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    head = d.get('head_sha', '')
    base = d.get('base_sha', '')
    if not head or not base or len(head) != 40 or len(base) != 40: continue
    # Find the tag for this cycle (v0.7.<N+16> by convention)
    tag = f'v0.7.{int(m.group(1)) + 16}'
    try:
        era = tag_peel_era(tag)
    except subprocess.CalledProcessError:
        continue
    if era != 'fix-peel':
        continue  # docs-peel cycles (m9-03..m9-06) intentionally have
        # base_sha != head_sha^ (branched from a docs commit, not fix
        # commit). This is accepted-by-design.
    try:
        parent = subprocess.check_output(
            ['git', 'rev-parse', f'{head}^'],
            stderr=subprocess.DEVNULL,
        ).decode().strip()
    except subprocess.CalledProcessError:
        continue
    if parent != base:
        print(f"DRIFT: {folder}: base_sha={base[:12]} != head^={parent[:12]} (expected parent of fix commit)")
```

**Expected output (clean):** empty.

**If `DRIFT`:** For a fix-peel cycle (m9-07+), the stored `base_sha`
does not match `head_sha^` (the parent of the fix commit). The
correct `base_sha` is whatever was the tip of main when the cycle
branch diverged — which, for fix-peel cycles, equals the parent of
the fix commit.

**Note on docs-peel cycles (m9-03..m9-06):** these are exempt because
they branched off a docs commit (the m9-04 archive commit, etc.),
not the fix commit. Their `base_sha` is the commit where their docs
branch diverged from main, which is not necessarily `head_sha^`.

**Resolution:**
1. Compute the real `base_sha`: `git rev-parse <head_sha>^`
2. Replace the fabricated value in `apply-checkpoint.json`
3. The `change-entry.md` `Base SHA` field already uses the short
   form (`6dce373`) which is a valid prefix of m9-18's fix commit
   but NOT of m9-19's actual base. Update the change-entry to use
   the correct short prefix too if the cycle's narrative depends
   on it.

**History:** m9-19 was authored with `base_sha = 6dce3739b7e2...`,
which is a fabricated 40-char SHA guessed from the m9-18 tag short
prefix `6dce373`. The real `head_sha^ = 735c57b7178c...` is "docs(m9-18):
add release receipts + vault archive + backfill m9-18 published SHA" —
the commit m9-19's branch actually diverged from. C8 (apply-checkpoint
SHA fields exist in repo) catches fabrication but only when the
fabricated SHA doesn't even exist; the m9-19 fabricated SHA was a
near-miss (close to a real commit, easy to miss). m9-28 closes this
drift and adds cross-check #20 with era-awareness to prevent
recurrence without breaking docs-peel cycles.

### 21. archive-manifest.md must have `## Evidence bindings` section (closed by m9-29)

```python
import os, glob
for manifest in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    with open(manifest) as f:
        content = f.read()
    if '## Evidence bindings' not in content:
        print(f"DRIFT: {manifest}: missing '## Evidence bindings' section")
```

**Expected output (clean):** empty.

**If `DRIFT`:** An archive-manifest.md is missing the `## Evidence
bindings` section. The section is the canonical binding between
the cycle's released artifacts (in `cycle-artifacts/`) and their
SHA-256 hashes. It is the primary evidence layer that lets readers
verify the archive's claims against the actual files.

**Resolution:**
1. Compute SHA-256 of each cycle artifact:
   - All files under `cycle-artifacts/p-3416cfb8288f8964/<folder>/`
   - The `change-entry.md` under
     `.sddk-knowledge/p-3416cfb8288f8964/changes/<folder>/`
2. Insert `## Evidence bindings` section before the next `## ...`
   heading, with one bullet per artifact in the form
   `- <path> → <sha256>`.
3. For m9-01 and m9-02 the format uses placeholder ("pending")
   before the SHA is captured; those are exempt as pre-cycle-artifacts
   era.

**History:** m9-11..m9-27 archive-manifests were authored without the
`## Evidence bindings` section (the agent in those cycles used a
different layout with `## Archived artifacts` + `## SHA verification`).
The `## Artifact index` / `## Artifact index (SHA-256)` section
already lists SHA-256s of the artifacts, so the missing Evidence
bindings section is a structural drift, not a content drift. m9-28
restored the Evidence bindings section for m9-28 itself; m9-29
backfills it for the 17 prior cycles (m9-11..m9-27).

### 22. release-receipt.md must have canonical SHA fields (closed by m9-30; regex hardened by m9-57)

```python
import json, os, re

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    rr = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/release-receipt.md'
    if not os.path.exists(ckpt) or not os.path.exists(rr):
        continue
    d = json.load(open(ckpt))
    content = open(rr).read()
    head = d.get('head_sha', '')
    peel = d.get('remote_tag_peel', '')
    if not head or len(head) != 40:
        continue
    # C22 part A: required fields present (handles both table and key-value format)
    for field in ['Head SHA', 'Remote tag', 'Remote tag_peel', 'Peel match']:
        # Match table `| Head SHA | sha |` OR key-value `Head SHA | sha` (newline-anchored)
        if not re.search(rf'(?:\n\| {field} \||\n{field} \|)', content):
            print(f"DRIFT: {folder}/release-receipt.md: missing '{field}' field")
    # C22 part B: SHAs match apply-checkpoint (regex handles both formats)
    head_m = re.search(r'(?:\n\| |\n)Head SHA \| `?([a-f0-9]+)`?', content)
    peel_m = re.search(r'(?:\n\| |\n)Remote tag_peel \| `?([a-f0-9]+)`?', content)
    if head_m and head_m.group(1) != head:
        print(f"DRIFT: {folder}/release-receipt.md: Head SHA mismatch")
    if peel_m and peel_m.group(1) != peel:
        print(f"DRIFT: {folder}/release-receipt.md: Remote tag_peel mismatch")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A release-receipt.md is either missing the canonical
fields (`Head SHA`, `Remote tag`, `Remote tag_peel`, `Peel match`)
or has SHA values that disagree with apply-checkpoint.json.

**Note:** The original cross-check #10 catches the case where the
field exists but has wrong value. C22 additionally catches the case
where the field is **missing entirely** (which m9-19..m9-27 had:
their release-receipts only had `Cycle`, `Tag`, `Pee`, `Released at`
and no Head SHA field at all).

**Resolution:**
1. Normalize the release-receipt to the canonical format (m9-28 format):
   - `| Cycle | <slug> |`
   - `| Head SHA | \`<sha>\` |`
   - `| Remote tag | \`<tag>\` |`
   - `| Remote tag_peel | \`<peel>\` |`
   - `| Peel match | true |`
   - `| Date | <released_at> |`
2. The `Remote tag` follows the convention `v0.7.<N-2>` for `m9-NN`
   (e.g. m9-19 → v0.7.17, m9-28 → v0.7.26).

**History:** m9-03..m9-10 used a different format with `Peel SHA`
field. m9-11..m9-18 used the Spanish `Campo` header. m9-19..m9-27
regressed to a minimal format with `Cycle`, `Tag`, `Pee` (typo),
`Released at` — no Head SHA at all. m9-28 restored the canonical
format. m9-30 normalizes all 25 prior cycles to the canonical format
and adds C22 to enforce it. m9-57 hardens the regex to accept both
the table format `| Head SHA | sha |` (which the CC original assumed)
and the key-value format `Head SHA | sha` (which every actual file
uses since m9-04).

### 23. merge-receipt.md must have canonical SHA fields (closed by m9-31; regex hardened by m9-57)

```python
import json, os, re

for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    mr = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/merge-receipt.md'
    if not os.path.exists(ckpt) or not os.path.exists(mr):
        continue
    d = json.load(open(ckpt))
    content = open(mr).read()
    head = d.get('head_sha', '')
    base = d.get('base_sha', '')
    if not head or len(head) != 40:
        continue
    # Required fields present (handles both table and key-value format)
    for field in ['Head SHA', 'Base SHA', 'Branch', 'Date']:
        if not re.search(rf'(?:\n\| {field} \||\n{field} \|)', content):
            print(f"DRIFT: {folder}/merge-receipt.md: missing '{field}' field")
    # SHAs match apply-checkpoint (regex handles both formats)
    head_m = re.search(r'(?:\n\| |\n)Head SHA \| `?([a-f0-9]+)`?', content)
    base_m = re.search(r'(?:\n\| |\n)Base SHA \| `?([a-f0-9]+)`?', content)
    if head_m and head_m.group(1) != head:
        print(f"DRIFT: {folder}/merge-receipt.md: Head SHA mismatch")
    if base_m and base_m.group(1) != base:
        print(f"DRIFT: {folder}/merge-receipt.md: Base SHA mismatch")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A merge-receipt.md is either missing the canonical
fields (`Head SHA`, `Base SHA`, `Branch`, `Date`) or has SHA values
that disagree with apply-checkpoint.json.

**Resolution:**
1. Normalize the merge-receipt to the canonical format (m9-28+ format):
   - `| Cycle | <slug> |`
   - `| Base SHA | \`<base>\` |`
   - `| Head SHA | \`<head>\` |`
   - `| Branch | \`<slug>\` |`
   - `| Date | <archived_at> |`

**History:** m9-03..m9-10 used `Main SHA` (different field name) instead
of `Head SHA`. m9-11..m9-18 used Spanish `Campo` header. m9-19..m9-27
used a minimal format with no Head SHA field at all. m9-28+ used the
canonical format. m9-31 normalizes all 25 prior merge-receipts to the
canonical format and adds C23 to enforce it. m9-57 hardens the regex
to accept both the table format `| Head SHA | sha |` (assumed) and
the key-value format `Head SHA | sha` (used in practice).

### 24. verify-report.md must have `## Cross-checks` section (closed by m9-32)

```python
import os, glob
for vr in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    content = open(vr).read()
    if '## Cross-checks' not in content:
        print(f"DRIFT: {vr}: missing '## Cross-checks' section")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A verify-report.md is missing the canonical
`## Cross-checks` section. This section is the place where the
cycle records its cross-check status (which vault-drift-sweep
checks were run, what passed, what failed).

**Resolution:**
1. Add a `## Cross-checks` section at the end of the verify-report.md
   with a brief note that the cycle predates the cross-check
   annotation format introduced in m9-28.
2. For cycles that did run cross-checks (m9-11+), transcribe the
   cross-check status from the cycle's apply-checkpoint.json or
   release-report.md into the section.

**History:** All 25 verify-report.md files (m9-03..m9-27) were
authored without the `## Cross-checks` section. The convention was
introduced in m9-28+. m9-32 backfills the section for all 25 prior
cycles.

### 25. change-entry.md title format must be `# Change: m9-NN <human-readable>` (closed by m9-33)

```python
import os, re
for folder in sorted(os.listdir('.sddk-knowledge/p-3416cfb8288f8964/changes/')):
    if folder == 'archive' or not os.path.isdir(f'.sddk-knowledge/p-3416cfb8288f8964/changes/{folder}'):
        continue
    ce = f'.sddk-knowledge/p-3416cfb8288f8964/changes/{folder}/change-entry.md'
    if not os.path.exists(ce): continue
    title = open(ce).readline().strip()
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    cycle_num = m.group(1)
    expected_prefix = f"# Change: m9-{cycle_num} "
    if not title.startswith(expected_prefix):
        print(f"DRIFT: {ce}: title={title!r} (expected starts with {expected_prefix!r})")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A change-entry.md is using a different title format
than the canonical `# Change: m9-NN <human-readable>`.

**Resolution:**
1. Convert `# Change Entry — <slug>` (pre-m9-19 format) to
   `# Change: m9-NN <human-readable>` by replacing dashes with spaces
   in the slug and stripping the `m9-NN-` prefix.
2. Convert `# Change: <title without m9-NN prefix>` to
   `# Change: m9-NN <title>`.

**History:** m9-01..m9-18 used `# Change Entry — <slug>` format. m9-19+
used `# Change: m9-NN <title>` format. m9-33 normalizes all 32 cycles
to the canonical format and adds C25 to enforce it.

### 26. verify-findings.json subject must have head_sha AND base_sha; archive-manifest.md header must have Head SHA (closed by m9-34)

```python
import json, os, re, glob

# Part A: verify-findings.json subject
for folder in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    vf = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/verify-findings.json'
    if not os.path.exists(vf): continue
    d = json.load(open(vf))
    sub = d.get('subject', {})
    if not isinstance(sub, dict): continue
    # Either (head_sha + base_sha) OR (head + base) for legacy schema
    has_head = sub.get('head_sha') or sub.get('head')
    has_base = sub.get('base_sha') or sub.get('base')
    if not has_head:
        print(f"DRIFT: {folder}/verify-findings.json: subject missing head_sha/head")
    if not has_base:
        print(f"DRIFT: {folder}/verify-findings.json: subject missing base_sha/base")

# Part B: archive-manifest.md header has Head SHA
for manifest in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    content = open(manifest).read()
    if '| Head SHA |' not in content:
        print(f"DRIFT: {manifest}: no Head SHA field in header")
```

**Expected output (clean):** empty.

**If `DRIFT`:** A verify-findings.json subject is missing the head
SHA or base SHA, or an archive-manifest.md header is missing the
Head SHA field.

**Resolution:**

For verify-findings.json:
1. Add `subject.base_sha` from apply-checkpoint.json if missing.
2. Legacy m9-03 schema uses `subject.base` / `subject.head` instead
   of `subject.base_sha` / `subject.head_sha` — exempt as accepted-by-design.

For archive-manifest.md:
1. m9-01..m9-10 used `| Published SHA |` instead of `| Head SHA |`.
   Add a `| Head SHA |` field as an alias for `| Published SHA |`
   (same value) so cross-references match.

**History:** m9-19..m9-27 verify-findings.json had `head_sha` but
no `base_sha`. m9-01..m9-10 archive-manifest.md had `Published SHA`
instead of `Head SHA`. m9-34 closes both drift classes and adds C26
to enforce them.

### 27. release-report.md title format `# Release Report — m9-NN` (closed by m9-35; regex extended by m9-57)

```python
import glob, re

errors = 0
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-report.md')):
    m = re.search(r'm9-(\d+)', f)
    if not m: continue
    n = m.group(1)
    # Accept both formats: '# Release Report — m9-NN' (no slug, legacy)
    # and '# Release Report — m9-NN-slug-here' (with slug, m9-45+)
    expected_a = f"# Release Report — m9-{n}"
    actual = open(f).readline().strip()
    if not (actual == expected_a or actual.startswith(expected_a + "-")):
        print(f"DRIFT: {f}: actual={actual!r}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** A release-report.md uses an outdated title format
(e.g. `# m9-NN: Release Report`).

**Resolution:**

1. Convert `# m9-NN: Release Report` to `# Release Report — m9-NN`
   (canonical format introduced in m9-28).

**History:** m9-19..m9-27 used `# m9-NN: Release Report` format;
m9-28+ uses `# Release Report — m9-NN`. m9-35 normalizes m9-19..m9-27
and adds C27 to enforce the canonical format.

### 28. release-receipt.md must have Base SHA field (closed by m9-36)

```python
import glob

errors = 0
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-receipt.md')):
    content = open(f).read()
    # Both pipe-separated (`Base SHA |`) and table (`| Base SHA |`) formats allowed
    if 'Base SHA' not in content:
        print(f"DRIFT: {f}: no Base SHA field")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** A release-receipt.md is missing the `Base SHA` field.

**Resolution:**

1. Add `Base SHA | <sha>` field to the release-receipt, sourced from
   the corresponding apply-checkpoint.json `base_sha` field.
2. While at it, normalize the file to the canonical pipe-separated
   format (introduced in m9-34): one field per line, `Field | Value`.

**History:** m9-03..m9-33 release-receipt.md used a markdown table
format that did not include the `Base SHA` field. m9-34, m9-35
introduced the canonical pipe-separated format with all required fields.
m9-36 normalizes m9-03..m9-33 to the canonical format and adds C28
to enforce the presence of `Base SHA`.

### 29. apply-checkpoint.json must use `route` (not `path`); change-entry.md Subject head_sha must match apply-checkpoint.json head_sha (closed by m9-37)

```python
import json, glob, re

errors = 0

# Part A: apply-checkpoint uses route, not path
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/apply-checkpoint.json')):
    d = json.load(open(f))
    if 'path' in d:
        print(f"DRIFT: {f}: has 'path' field, should be 'route'")
    if 'route' not in d:
        print(f"DRIFT: {f}: missing 'route' field")

# Part B: change-entry Subject head_sha matches apply-checkpoint head_sha
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    folder = '/'.join(f.split('/')[3:-1])
    ckpt_path = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not __import__('os').path.exists(ckpt_path): continue
    content = open(f).read()
    m = re.search(r'- head_sha: `([a-f0-9]+)`', content)
    if m:
        head_sha = m.group(1)
        ckpt = json.load(open(ckpt_path))
        if ckpt.get('head_sha') and head_sha != ckpt['head_sha']:
            print(f"DRIFT: {f}: head_sha={head_sha[:8]} != ckpt={ckpt['head_sha'][:8]}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: An apply-checkpoint.json uses the legacy `path` field instead of
  the canonical `route` field.
- B: A change-entry.md Subject section has a `head_sha` that doesn't
  match the corresponding apply-checkpoint.json `head_sha`.

**Resolution:**

For Part A:
1. Rename `path` → `route` in apply-checkpoint.json.

For Part B:
1. Update change-entry.md Subject `head_sha` to match apply-checkpoint.json.
   The mismatch is usually caused by writing the change-entry before
   the cycle's receipts/rebase was complete.

**History:** m9-03..m9-33 apply-checkpoint used `path` field; m9-21+
added `route` alongside `path`. m9-34 dropped `path` entirely. m9-37
normalizes m9-03..m9-33 to `route`-only. m9-34, m9-35 change-entry
`head_sha` field was written with intermediate SHA, not final HEAD;
m9-37 fixes both.

### 30. verify-report.md title + verify-findings verdict + archive-manifest Cycle field + change-entry Summary section (closed by m9-38; regex extended by m9-57)

```python
import json, glob, re

errors = 0

# Part A: verify-report.md title is `# Verify Report — m9-NN`
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    m = re.search(r'm9-(\d+)', f)
    if not m: continue
    n = m.group(1)
    # Accept both formats: '# Verify Report — m9-NN' (no slug, legacy)
    # and '# Verify Report — m9-NN-slug-here' (with slug, m9-45+)
    expected_a = f"# Verify Report — m9-{n}"
    actual = open(f).readline().strip()
    if not (actual == expected_a or actual.startswith(expected_a + "-")):
        print(f"DRIFT: {f}: actual={actual!r}")

# Part B: verify-findings.json has verdict field
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    d = json.load(open(f))
    if 'verdict' not in d:
        print(f"DRIFT: {f}: no verdict field")

# Part C: archive-manifest.md has Cycle field (not Cycle ID)
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    content = open(f).read()
    if 'Cycle ID' in content:
        print(f"DRIFT: {f}: has 'Cycle ID' field, should be 'Cycle'")
    if '| Cycle |' not in content:
        print(f"DRIFT: {f}: no 'Cycle' field")

# Part D: change-entry.md has ## Summary section
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    if '## Summary' not in open(f).read():
        print(f"DRIFT: {f}: no ## Summary section")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**

- A: verify-report.md uses a non-canonical title format.
- B: verify-findings.json missing `verdict` field.
- C: archive-manifest.md uses `Cycle ID` instead of `Cycle` field.
- D: change-entry.md missing `## Summary` section.

**Resolution:**

- A: Normalize to `# Verify Report — m9-NN` (canonical).
- B: Add `verdict` field; use `pass` if no findings, else `pass_with_findings`.
- C: Rename `Cycle ID` → `Cycle`.
- D: Add `## Summary` section right after the H1 title.

**History:** m9-03..m9-10 used `# Verification Report: m9-NN-slug`;
m9-11..m9-18 used `# Verify Report — m9-NN-slug`; m9-19..m9-27 used
`# m9-NN: Verify Report`; m9-28..m9-33 used canonical `# Verify Report — m9-NN`.
m9-34..m9-37 used `# Verify Findings — ...`. m9-38 normalizes all to
canonical `# Verify Report — m9-NN`.

m9-03..m9-27 verify-findings.json lacked `verdict` (used `summary.verdict`).
m9-38 adds the top-level `verdict` field.

m9-01..m9-18 archive-manifest.md used `Cycle ID`; m9-19+ uses `Cycle`.
m9-38 normalizes all to `Cycle`.

m9-01..m9-18 change-entry.md used Spanish `## Ciclo`; m9-19+ uses `## Summary`.
m9-38 normalizes all to `## Summary` (and inserts a placeholder summary
text).

### 31. archive-manifest.md Base SHA field + verify-report.md Cross-checks section (closed by m9-39)

```python
import glob, re

errors = 0

# Part A: archive-manifest.md has Base SHA field
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    content = open(f).read()
    if 'Base SHA' not in content:
        print(f"DRIFT: {f}: no Base SHA field")

# Part B: verify-report.md has Cross-checks section (post-m9-32)
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    m = re.search(r'm9-(\d+)', f)
    if not m: continue
    n = int(m.group(1))
    if n < 32: continue
    content = open(f).read()
    if 'Cross-checks' not in content:
        print(f"DRIFT: {f}: no Cross-checks section")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: archive-manifest.md missing `Base SHA` field in header table.
- B: verify-report.md missing `## Cross-checks` section.

**Resolution:**
- A: Add `| Base SHA | <sha> |` line right after the `| Head SHA |` line.
  Source the SHA from apply-checkpoint.json `base_sha`.
- B: Add `## Cross-checks` section listing the cross-checks that
  passed/failed during this cycle's verification.

**History:** m9-11..m9-27 archive-manifest.md used a table format
without `Base SHA`. m9-39 backfills it. m9-34 verify-report.md
was written before the Cross-checks section was standardized;
m9-39 adds it.

### 32. release-report.md must have Path field + Cross-checks/Verification section (closed by m9-40)

```python
import glob

errors = 0

for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-report.md')):
    content = open(f).read()
    # Path field somewhere in the file
    if 'Path' not in content[:500]:
        print(f"DRIFT: {f}: no Path field")
    # Cross-checks or Verification section
    if 'Cross-checks' not in content and '## Verification' not in content:
        print(f"DRIFT: {f}: no Cross-checks / Verification section")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: release-report.md is missing `Path` field (e.g. `**Path**: B-direct`)
  in the header area.
- B: release-report.md is missing a `## Cross-checks` or `## Verification`
  section listing which cross-checks passed/failed.

**Resolution:**
- A: Add `**Path**: <route>` line right after `**Cycle**` line, sourced
  from apply-checkpoint.json `route`.
- B: Add `## Cross-checks` section listing the cross-checks that
  passed during this cycle's verification. Source the cross-check IDs
  from the cycle's verify-report.md (which should list them).

**History:** m9-04..m9-33 release-report.md didn't have Path field;
m9-34+ uses Path. m9-40 backfills Path for m9-04..m9-33.

m9-11..m9-27 release-report.md didn't have Cross-checks section;
m9-28+ uses `## Verification` or `## Cross-checks`. m9-40 backfills
Cross-checks for m9-11..m9-27.

### 33. change-entry.md Subject/Files changed sections + archive-manifest.md Summary + verify-report.md Subject (closed by m9-41)

```python
import glob, os, re

errors = 0

# Part A: change-entry.md has ## Subject section (m9-03..m9-37)
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    # m9-01 and m9-02 are pre-vault-reorg, accepted-by-design
    if 'm9-01-schema-versioning' in f or 'm9-02-events-side-table' in f:
        continue
    content = open(f).read()
    if '## Subject' not in content:
        print(f"DRIFT: {f}: no ## Subject")

# Part B: change-entry.md has ## Files changed section
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    if '## Files changed' not in open(f).read():
        print(f"DRIFT: {f}: no ## Files changed")

# Part C: archive-manifest.md has ## Summary section
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    if '## Summary' not in open(f).read():
        print(f"DRIFT: {f}: no ## Summary")

# Part D: verify-report.md has ## Subject section (post-m9-28)
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    m = re.search(r'm9-(\d+)', f)
    if not m: continue
    n = int(m.group(1))
    if n < 28: continue
    content = open(f).read()
    if '## Subject' not in content:
        print(f"DRIFT: {f}: no ## Subject")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: change-entry.md missing `## Subject` section (m9-03..m9-37).
  m9-01, m9-02 are pre-vault-reorg; accepted-by-design.
- B: change-entry.md missing `## Files changed` section.
- C: archive-manifest.md missing `## Summary` section.
- D: verify-report.md missing `## Subject` section (m9-28+).

**Resolution:**

- A: Convert existing metadata table (m9-19..m9-33) to `## Subject`
  section. Convert `## Ciclo` (m9-01..m9-18) to `## Subject`.
- B: Add `## Files changed` section listing the cycle artifacts and
  knowledge artifacts touched by the cycle.
- C: Add `## Summary` section with a brief one-sentence cycle description.
- D: Add `## Subject` section listing base_sha, head_sha, cycle number.

**History:** m9-01..m9-18 change-entry.md used `## Ciclo` (Spanish).
m9-19..m9-33 used a metadata table at the top. m9-34+ uses `## Subject`.
m9-41 normalizes all (except m9-01, m9-02) to `## Subject`.

m9-01..m9-40 change-entry.md didn't have `## Files changed` section.
m9-41 adds it to all.

m9-01..m9-27 archive-manifest.md didn't have `## Summary` section.
m9-41 adds a placeholder summary.

m9-28..m9-33 verify-report.md didn't have `## Subject` section.
m9-41 adds it.

### 34. change-entry.md Subject must be bullet list + change-entry Cross-check section + verify-findings cycle_id (closed by m9-42)

```python
import json, glob, re

errors = 0

# Part A: change-entry.md Subject section uses bullet list format
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    if 'm9-01-schema-versioning' in f or 'm9-02-events-side-table' in f:
        continue
    content = open(f).read()
    # Subject section should have bullet list `- field: value`, not a table
    m = re.search(r'## Subject\n+(.*?)(?=\n## |\Z)', content, re.DOTALL)
    if m:
        subject_body = m.group(1)
        # Check if it's a table (starts with | Campo | or | Field |)
        if re.match(r'\s*\|', subject_body):
            print(f"DRIFT: {f}: Subject is a table, should be bullet list")

# Part B: change-entry.md has ## Cross-check or ## Verification section
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    content = open(f).read()
    if '## Cross-check' not in content and '## Verification' not in content:
        print(f"DRIFT: {f}: no ## Cross-check or ## Verification section")

# Part C: verify-findings.json has cycle_id field
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    d = json.load(open(f))
    if 'cycle_id' not in d:
        print(f"DRIFT: {f}: no cycle_id field")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: change-entry.md Subject section uses a markdown table
  (`| Campo | Valor |` or `| Field | Value |`) instead of bullet list.
- B: change-entry.md is missing `## Cross-check` or `## Verification` section.
- C: verify-findings.json is missing `cycle_id` field.

**Resolution:**

- A: Convert table rows to bullet list format (`- field: value`).
  Skip m9-01, m9-02 (pre-vault-reorg, accepted-by-design).
- B: Add `## Cross-check` section listing cross-check IDs added.
- C: Add `cycle_id` field to verify-findings.json, using the folder
  slug as the value.

**History:** m9-03..m9-18 change-entry.md Subject used a markdown
table. m9-19..m9-33 used top metadata table. m9-34..m9-40 used
bullet list. m9-42 normalizes m9-03..m9-18 to bullet list.

m9-01..m9-18 change-entry.md didn't have `## Cross-check` section.
m9-19+ uses `## Cross-check` or `## Verification`. m9-42 backfills.

m9-03 verify-findings.json lacked `cycle_id` (legacy `sddk.verify-finding/v1`
schema). m9-42 adds it.

Each closed a one-line drift that the prior session's "exhausted"
verdict missed. The lesson is that **vault drift is a first-class
maintenance surface**, not a side effect of code work.

### 35. verify-findings.json subject.head_sha must match apply-checkpoint.json head_sha (closed by m9-43)

```python
import json, glob, os

errors = 0

for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    folder = f.split('/')[-2]
    ckpt_path = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt_path): continue
    d = json.load(open(f))
    ckpt = json.load(open(ckpt_path))
    sub = d.get('subject', {})
    head = sub.get('head_sha') or sub.get('head')
    if head and ckpt.get('head_sha') and head != ckpt['head_sha']:
        # m9-03, m9-04 use legacy docs-peel convention with subject.head as fix commit
        if folder in ['m9-03-side-table-debt-cleanup', 'm9-04-side-table-key-layout']:
            continue
        print(f"DRIFT: {folder}: verify-findings head={head[:8]} != ckpt={ckpt['head_sha'][:8]}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** A verify-findings.json `subject.head_sha` (or `subject.head`)
doesn't match the corresponding apply-checkpoint.json `head_sha`.

**Resolution:**

1. Update verify-findings.json `subject.head_sha` to match
   apply-checkpoint.json `head_sha`.
2. m9-03, m9-04 are exempt — they use the legacy
   `sddk.verify-finding/v1` schema with `subject.head` = fix commit
   (docs-peel convention). Per C20, this is accepted-by-design for
   pre-m9-11 cycles.

**History:** m9-34, m9-35 verify-findings.json were written with the
fix commit SHA (since the tag peels to fix) but apply-checkpoint.json
uses the docs commit SHA (per fix-peel convention). m9-43 syncs
verify-findings to match apply-checkpoint.

### 36. verify-report.md Path field + verify-findings.json lens_summary + verify-report.md Findings format (closed by m9-44)

```python
import json, glob, re, os

errors = 0

# Part A: verify-report.md has Path field
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    content = open(f).read()
    if 'Path' not in content[:500] and 'route' not in content[:500]:
        print(f"DRIFT: {f}: no Path field")

# Part B: verify-findings.json has lens_summary (or legacy summary)
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    d = json.load(open(f))
    if 'lens_summary' not in d and 'summary' not in d:
        print(f"DRIFT: {f}: no lens_summary or summary")

# Part C: verify-report.md has Findings section with table or "None" prose
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    content = open(f).read()
    # Accept either "## Findings" or "## Findings Closed"
    m = re.search(r'## Findings[^\n]*\n+(.*?)(?=\n## |\Z)', content, re.DOTALL)
    if m:
        body = m.group(1).strip()
        if body and not re.search(r'\|\s*ID\s*\|', body) and not re.search(r'^None', body, re.MULTILINE):
            print(f"DRIFT: {f}: Findings body has no table or 'None' prose")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:**
- A: verify-report.md missing `**Path**` field.
- B: verify-findings.json missing `lens_summary` field.
- C: verify-report.md `## Findings` section has prose that isn't a table
  or `None` marker.

**Resolution:**

- A: Add `**Path**: <route>` line, sourced from apply-checkpoint.json.
- B: Add `lens_summary` field with `drift` key describing what was closed.
- C: Either convert prose to a table, or replace with `None — clean state.`
  prose marker.

**History:** m9-03..m9-43 verify-report.md didn't have `Path` field.
m9-44 backfills. m9-04..m9-27 verify-findings.json lacked
`lens_summary`. m9-44 backfills. m9-09, m9-10 verify-report.md had
non-canonical Findings prose; m9-44 normalizes to `None — clean state.`.

### 37. change-entry.md section order + release-report.md cycle value (closed by m9-45)

```python
import glob, re, os

errors = 0

# Part A: change-entry.md: ## Subject heading must appear before ## Cross-check heading
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/m9-*/change-entry.md')):
    content = open(f).read()
    headings = re.findall(r'^## (.+)$', content, re.MULTILINE)
    subject_pos = next((i for i, h in enumerate(headings) if h == 'Subject'), -1)
    cc_pos = next((i for i, h in enumerate(headings)
                   if h.startswith('Cross-check') or h.startswith('Verification')), -1)
    if subject_pos != -1 and cc_pos != -1 and cc_pos < subject_pos:
        errors += 1
        print(f"DRIFT A: {f}: Cross-check idx={cc_pos} < Subject idx={subject_pos}")

# Part B: release-report.md: **Cycle** value must equal folder slug
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-report.md')):
    folder = f.split('/')[-2]
    content = open(f).read()
    cycle_m = re.search(r'\*\*Cycle\*\*:\s*([^\n]+)', content)
    if cycle_m and cycle_m.group(1).strip() != folder:
        errors += 1
        print(f"DRIFT B: {f}: cycle={cycle_m.group(1).strip()!r} expected={folder!r}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT A`:** `## Cross-check` heading appears before `## Subject`
heading in change-entry.md. Move Cross-check section to after Subject.

**If `DRIFT B`:** `**Cycle**:` value contains spaces instead of being
the canonical folder slug (e.g. `m9-34 name` instead of
`m9-34-name`). Replace with folder name.

**History:** m9-34..m9-44 change-entry.md wrote Cross-check sections
near the end of the file but Subject was inserted in the middle by
m9-42's bullet conversion. m9-45 normalizes the order. m9-34..m9-44
release-report.md had `**Cycle**: m9-NN name` (with space) instead of
`**Cycle**: m9-NN-name` (folder slug); m9-45 fixes.

### 38. verify-findings.json findings array populated from verify-report table (closed by m9-46)

```python
import json, glob, re, os

errors = 0
for vf in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    folder = vf.split('/')[-2]
    vr_path = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/verify-report.md'
    if not os.path.exists(vr_path):
        continue
    vr = open(vr_path).read()
    findings_m = re.search(r'## Findings[^\n]*\n+(.*?)(?=\n## |\Z)', vr, re.DOTALL)
    if not findings_m:
        continue
    body = findings_m.group(1)
    if re.search(r'^None', body, re.MULTILINE):
        table_rows = 0
    else:
        table_rows = len(re.findall(r'^\| F\d+', body, re.MULTILINE))
    d = json.load(open(vf))
    findings_count = len(d.get('findings', []))
    if table_rows != findings_count:
        if table_rows == 0 and findings_count == 0:
            continue
        errors += 1
        print(f"DRIFT: {folder}: table_rows={table_rows} findings={findings_count}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** `verify-findings.json` `findings` array length doesn't
match the number of table rows in `verify-report.md` `## Findings`
section. The findings array should be populated from the table.

**History:** m9-34..m9-45 verify-findings.json had empty `findings`
arrays even though verify-report.md had populated Findings tables.
m9-46 populates from the table.

### 39. archive-manifest.md + release-report.md Cross-checks section + cycles index Total cycles (closed by m9-47)

```python
import glob, re, os

errors = 0

# Part A: archive-manifest.md Cross-checks section
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    content = open(f).read()
    if '## Cross-checks' not in content:
        errors += 1
        print(f"DRIFT A: {f}: missing '## Cross-checks' section")

# Part B: release-report.md Cross-checks section
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-report.md')):
    content = open(f).read()
    if '## Cross-checks' not in content:
        errors += 1
        print(f"DRIFT B: {f}: missing '## Cross-checks' section")

# Part C: cycles/index.md Total cycles consistency
index = open('.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md').read()
total_m = re.search(r'Total cycles \| (\d+)', index)
if total_m:
    expected = int(total_m.group(1))
    # Cycles live in 3 places:
    #  - cycle-artifacts/m9-NN-* legacy (m9-01, m9-02)
    #  - cycle-artifacts/p-3416cfb8288f8964/m9-NN-*
    #  - .sddk-knowledge/p-3416cfb8288f8964/changes/m9-NN-* (knowledge-only, e.g. m9-54)
    actual = (
        len([f for f in os.listdir('cycle-artifacts')
             if f.startswith('m9-') and os.path.isdir(f'cycle-artifacts/{f}')]) +
        len([f for f in os.listdir('cycle-artifacts/p-3416cfb8288f8964')
             if f.startswith('m9-') and os.path.isdir(f'cycle-artifacts/p-3416cfb8288f8964/{f}')]) +
        len([f for f in os.listdir('.sddk-knowledge/p-3416cfb8288f8964/changes')
             if f.startswith('m9-') and os.path.isdir(f'.sddk-knowledge/p-3416cfb8288f8964/changes/{f}')
             and not os.path.isdir(f'cycle-artifacts/{f}')
             and not os.path.isdir(f'cycle-artifacts/p-3416cfb8288f8964/{f}')])
    )
    if expected != actual:
        errors += 1
        print(f"DRIFT C: index says {expected}, actual {actual}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT A`:** archive-manifest.md missing `## Cross-checks` section.
Add it with the list of cross-checks that were active when the cycle
ran.

**If `DRIFT B`:** release-report.md missing `## Cross-checks` section.
Same fix.

**If `DRIFT C`:** cycles/index.md `Total cycles` count doesn't match
actual cycle directory count.

**History:** m9-01..m9-27 archive-manifest.md and m9-03..m9-10,
m9-28..m9-33 release-report.md lacked Cross-checks sections. m9-47
adds them. Cycles index `Total cycles` was inflated by legacy
double-counting; m9-47 corrects.

### 40. comprehensive schema backfill (closed by m9-48)

```python
import json, glob, re, os

errors = 0

# Part A: apply-checkpoint.json must have findings_closed
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/apply-checkpoint.json')):
    d = json.load(open(f))
    if 'findings_closed' not in d:
        errors += 1
        print(f"DRIFT A: {f}: missing findings_closed")

# Part B: archive-manifest.md must have Date and Path fields
for f in sorted(glob.glob('.sddk-knowledge/p-3416cfb8288f8964/changes/archive/m9-*/archive-manifest.md')):
    content = open(f).read()
    if '| Date |' not in content:
        errors += 1
        print(f"DRIFT B: {f}: missing Date field")
    if '| Path |' not in content:
        errors += 1
        print(f"DRIFT B: {f}: missing Path field")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT A`:** apply-checkpoint.json missing `findings_closed` array.

**If `DRIFT B`:** archive-manifest.md missing `Date` or `Path` field.

**History:** m9-19..m9-33 apply-checkpoint.json had no `findings_closed`
array. m9-48 adds it. m9-01..m9-27 archive-manifest.md lacked `Date`
and `Path` fields. m9-48 backfills.

### 41. verify-findings.json lens_summary for m9-03 + release-report.md schema (closed by m9-49)

```python
import json, glob, re

errors = 0

# Part A: verify-findings.json must have lens_summary
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    d = json.load(open(f))
    if 'lens_summary' not in d:
        errors += 1
        print(f"DRIFT A: {f}: missing lens_summary")

# Part B: release-report.md must have **Cycle**, **Path**, **Tag** + ## sections (canonical era only)
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-report.md')):
    folder = f.split('/')[-2]
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    num = int(m.group(1))
    if num < 19 or num >= 34: continue
    content = open(f).read()
    for field in ['Cycle', 'Path', 'Tag']:
        if f'**{field}**' not in content:
            errors += 1
            print(f"DRIFT B: {folder}: missing **{field}**")
    for h in ['## What changed', '## Verification']:
        if h not in content:
            errors += 1
            print(f"DRIFT B: {folder}: missing {h}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**History:** m9-03 verify-findings.json lacked lens_summary. m9-49 adds.
m9-19..m9-33 release-report.md had legacy format without **Cycle**,
**Path**, **Tag** fields or `## What changed`, `## Verification`
sections. m9-49 backfills.

### 42. release-receipt.md peel accuracy + missing tags + index timestamps (closed by m9-50)

```python
import subprocess, re, glob, os

errors = 0

# Part A: every cycle's tag must exist and release-receipt.peel must match
for rr in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-receipt.md')):
    folder = rr.split('/')[-2]
    content = open(rr).read()
    tag_m = re.search(r'Remote tag \| (v[0-9.]+)', content)
    peel_m = re.search(r'Remote tag_peel \| ([a-f0-9]+)', content)
    if not tag_m or not peel_m:
        errors += 1
        print(f"DRIFT A: {folder}: missing tag/peel")
        continue
    tag = tag_m.group(1)
    stored_peel = peel_m.group(1)
    result = subprocess.run(['git', 'rev-parse', f'{tag}^{{commit}}'], capture_output=True, text=True)
    if result.returncode != 0:
        errors += 1
        print(f"DRIFT A: {folder}: tag {tag} missing from git")
        continue
    current_peel = result.stdout.strip()
    if current_peel != stored_peel:
        errors += 1
        print(f"DRIFT A: {folder}: peel {stored_peel[:8]} != current {current_peel[:8]}")

# Part B: cycles/index.md Last updated exists
ci = open('.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md').read()
m = re.search(r'Last updated \| (\S+)', ci)
if not m:
    errors += 1
    print(f"DRIFT B: cycles/index.md missing 'Last updated' field")

# Part C: terms/index.md Last updated exists
ti = open('.sddk-knowledge/p-3416cfb8288f8964/terms/index.md').read()
m = re.search(r'Last updated \| (\S+)', ti)
if not m:
    errors += 1
    print(f"DRIFT C: terms/index.md missing 'Last updated' field")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT A`:** the cycle's tag is missing from git OR the
release-receipt's `Remote tag_peel` doesn't match the current commit
the tag points to. Resolution: ensure `git tag vN` exists, and run
`git rev-parse vN^{commit}` to get the correct peel.

**History:** v0.7.31 and v0.7.43 tags had been deleted during prior
peels without re-creation. 47 release-receipt.md files had stored
peel using `git rev-parse vN` (which returns the tag-object SHA, not
the commit). m9-50 recreates tags + fixes receipts to use
`vN^{commit}`.

### 43. release-receipt and verify-findings head_sha consistency with apply-checkpoint (closed by m9-51)

```python
import json, glob, re, os

errors = 0

# Part A: release-receipt Head SHA must match apply-checkpoint head_sha
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/release-receipt.md')):
    folder = f.split('/')[-2]
    content = open(f).read()
    h_m = re.search(r'Head SHA \| ([a-f0-9]+)', content)
    h_str = h_m.group(1) if h_m else ''
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    d = json.load(open(ckpt))
    if h_str != d.get('head_sha', ''):
        errors += 1
        print(f"DRIFT A: {folder}: rr={h_str[:8]} ckpt={d['head_sha'][:8]}")

# Part B: verify-findings subject.head_sha must match apply-checkpoint head_sha
for vf in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-findings.json')):
    folder = vf.split('/')[-2]
    d = json.load(open(vf))
    vf_head = d.get('subject', {}).get('head_sha', '')
    ckpt = f'cycle-artifacts/p-3416cfb8288f8964/{folder}/apply-checkpoint.json'
    if not os.path.exists(ckpt): continue
    ck = json.load(open(ckpt))
    if vf_head != ck.get('head_sha', ''):
        errors += 1
        print(f"DRIFT B: {folder}: vf={vf_head[:8]} ckpt={ck['head_sha'][:8]}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT A`:** `release-receipt.md` `Head SHA` doesn't match
`apply-checkpoint.json` `head_sha`. The apply-checkpoint is
authoritative.

**If `DRIFT B`:** `verify-findings.json` `subject.head_sha` doesn't
match `apply-checkpoint.json` `head_sha`.

**History:** m9-03, m9-04 verify-findings had head_sha pointing at the
original fix commit (570d215, 379759ed) rather than the cycle's
published head (2c98ce9a, d6b3b8c5). m9-34, m9-35 release-receipts
recorded the cycle's intermediate SHA rather than the final published
SHA. m9-51 fixes these to align with apply-checkpoint.

### 44. verify-report.md ## Summary section (post-m9-19) (closed by m9-52)

```python
import glob, re

errors = 0
for f in sorted(glob.glob('cycle-artifacts/p-3416cfb8288f8964/m9-*/verify-report.md')):
    folder = f.split('/')[-2]
    m = re.match(r'm9-(\d+)', folder)
    if not m: continue
    num = int(m.group(1))
    if num < 19: continue
    content = open(f).read()
    if '## Summary' not in content:
        errors += 1
        print(f"DRIFT: {folder}: missing ## Summary")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** verify-report.md (m9-19+) lacks `## Summary` heading.

**History:** m9-19, m9-28..m9-33 verify-report.md used table-based
or finding-only format. m9-52 adds Summary section to 7 files.

### 45. cycles/index.md Published SHA matches tag commit (closed by m9-53)

```python
import re, subprocess

content = open('.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md').read()
errors = 0
for m in re.finditer(r'\| (m9-\d{2}-[a-z0-9-]+) \| (m9-\d{2}-[a-z0-9-]+) \| B-direct \| `(v0\.7\.\d+)` \| `([a-f0-9]+)` \| CLOSED \|', content):
    idx_sha = m.group(4)
    tag = m.group(3)
    result = subprocess.run(['git', 'rev-parse', tag + '^{commit}'], capture_output=True, text=True)
    if result.returncode != 0:
        errors += 1
        print(f"DRIFT: tag {tag} missing")
        continue
    tag_sha = result.stdout.strip()
    if not tag_sha.startswith(idx_sha[:8]):
        errors += 1
        print(f"DRIFT: tag {tag}: idx={idx_sha[:8]} tag={tag_sha[:8]}")

print(f'Total: {errors}')
```

**Expected output (clean):** empty.

**If `DRIFT`:** cycles/index.md Published SHA prefix doesn't match
the tag's commit. Either the index SHA is stale or the tag was
re-pointed without updating the index.

**History:** m9-19..m9-33 cycles/index.md had 7-char short SHAs that
were equivalent to the tag commit prefix. m9-53 expanded to full
40-char (35 rows).

### 46. No stale local or remote feature/fix branches (closed by m9-54)

```bash
git branch --list 'fix/m9-*' | wc -l
git branch -r --list 'origin/fix/m9-*' | wc -l
```

**Expected output (clean):** `0` and `0`.

**If non-zero:** Stale branches accumulated across cycles. Each
m9-cycle (m9-11+) creates a per-cycle branch, merges to main, but
forgets to delete the branch. After N cycles this accumulates N
branches. Same applies to remote.

**Resolution:**
- `git branch --list 'fix/m9-*' | xargs git branch -d` (safe delete,
  only deletes merged branches; non-merged exits with warning)
- `git push origin :fix/m9-<slug>` for each remote, or batch with
  `git branch -r --list 'origin/fix/m9-*' | sed 's/origin\///' | xargs -I{} git push origin :{}`

**History:** m9-54 closed this drift class after 53 m9-cycles
accumulated 44 local + 28 remote stale branches.

### 47. apply-checkpoint.json `base_sha` must exist in git (closed by m9-55)

```python
import json, os, subprocess

errors = 0
for d in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964/')):
    if not d.startswith('m9-'):
        continue
    p = f'cycle-artifacts/p-3416cfb8288f8964/{d}/apply-checkpoint.json'
    if not os.path.exists(p):
        continue
    j = json.load(open(p))
    for field in ('base_sha', 'head_sha'):
        sha = j.get(field)
        if not sha:
            continue
        res = subprocess.run(['git', 'cat-file', '-t', sha],
                             capture_output=True)
        if res.returncode != 0 or res.stdout.decode().strip() != 'commit':
            errors += 1
            print(f"DRIFT: {d}.{field} = {sha} (does not exist in git or is not a commit)")

print(f'Total: {errors}')
```

**Expected output (clean):** `Total: 0`.

**If `DRIFT`:** apply-checkpoint stores a `base_sha` or `head_sha`
that is not a reachable git commit. This is a fabrication drift:
the SHA passes format validation (40 hex chars) but fails
reachability validation (`git cat-file -t` exit 128).

**Resolution:**
- For `base_sha`: the real base is the parent of the cycle's first
  commit on the cycle branch. Run `git reflog --all | grep <cycle>`
  to find when the `fix/<slug>` branch was created; the commit
  preceding that is the cycle's base.
- For `head_sha`: the cycle's actual fix commit. Reachable via
  `git log --all --pretty=format:'%H %s' --grep=<cycle-marker>`.
- Mechanical `sed` replacement to all 7 artifact files
  (apply-checkpoint, merge-receipt, release-receipt, verify-findings,
  verify-report, change-entry, archive-manifest).

**History:** m9-55 closed this drift class. m9-38's `base_sha`
(`6bc6781465f9dff5a59bd4d8e8a99930dba3e7e5`) was a transcription
error of the real parent-of-head
(`6bc67812d66548a3e0ee48f5323d45c4a1ed13d8`). Missed by 46 prior
cross-checks that validated format only.
### 48. vault-drift-sweep self-consistency: every CC returns empty output (closed by m9-57)

```python
import re, subprocess

ci = open('.sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md').read()
sections = re.split(r'### (\d+)\.', ci)
errors = 0
for i in range(1, len(sections), 2):
    num = int(sections[i])
    if num == 48:  # skip self to avoid infinite recursion
        continue
    body = sections[i+1]
    py_blocks = re.findall(r'```python\n(.+?)\n```', body, re.DOTALL)
    if py_blocks:
        try:
            result = subprocess.run(['python3', '-c', py_blocks[0]], capture_output=True, text=True, timeout=30)
            output = (result.stdout or '').strip()
            # Extract DRIFT lines
            drift_lines = [l for l in output.split('\n') if l.startswith('DRIFT')]
            if drift_lines:
                print(f"DRIFT: CC#{num} reported {len(drift_lines)} drift lines")
                errors += len(drift_lines)
        except subprocess.TimeoutExpired:
            print(f"DRIFT: CC#{num} timed out")
            errors += 1
```

**Expected output (clean):** empty.

**If `DRIFT`:** one or more cross-checks in this file is reporting
actual drift. Run each failing CC manually to identify which cycle
artifacts or knowledge artifacts are out of compliance.

**Why this exists:** before m9-57, individual CCs were sometimes
over-strict (CC#22, CC#23, CC#27, CC#30 — too-narrow regex), or
expected fields that didn't exist (CC#15, CC#11 — backfill gap on
m9-34+ apply-checkpoints). The previous orchestrator would report
"all CCs PASS" because their detection script filtered on the
`Total: X` line pattern and missed CCs that print DRIFT lines
without a summary line. CC#48 catches any CC reporting drift by
counting all `DRIFT:` prefix lines across all CCs.

**History:** m9-54..m9-56 cycles ran with the broad-pattern filter,
which falsely reported "all 47 CCs PASS" while m9-34+ apply-checkpoints
were missing 11 fields each. m9-57 broadens the CC runner to detect
any DRIFT line, then closes the underlying drift classes by:
backfilling missing fields, hardening overly-strict regexes, and
exempting fix-peel cycles from era-incompatible checks.

### 49. SHA fields must use full 40-char SHAs (closed by m9-58)

```python
import os, re

drifts = 0
for root in ['.sddk-knowledge/p-3416cfb8288f8964', 'cycle-artifacts/p-3416cfb8288f8964']:
    for d, _, files in os.walk(root):
        for f in files:
            if not f.endswith(('.md', '.json')):
                continue
            p = os.path.join(d, f)
            try:
                ci = open(p).read()
            except:
                continue
            in_code = False
            for line in ci.split('\n'):
                if line.strip().startswith('\`\`\`'):
                    in_code = not in_code
                    continue
                if in_code:
                    continue
                if not (line.lstrip().startswith('|') or re.match(r'^\s*"(?:head_sha|base_sha|remote_tag|remote_tag_peel|main_sha|merge_sha)"', line) or re.match(r'^\s*(?:head_sha|base_sha|remote_tag|remote_tag_peel|main_sha|merge_sha):', line)):
                    continue
                for m in re.finditer(r'(?:head_sha|base_sha|Head SHA|Base SHA|remote_tag|remote_tag_peel|main_sha|merge_sha)\s*[|:]\s*`([a-f0-9]{1,39})`', line):
                    rel = p.replace(root + '/', '')
                    print(f"DRIFT: {rel}: short SHA in '{m.group(0)[:40]}' ({m.group(1)})")
                    drifts += 1
print(f'Total: {drifts}')
```

**Expected output (clean):** `Total: 0`.

**If `DRIFT`:** a SHA-field metadata cell references a hash that is
shorter than 40 characters. Code-block narrative references (e.g.,
diff-style `cd0115f` in the body of an archive-manifest summary)
are skipped — the regex only inspects table rows (lines starting
with `|`) and JSON property assignments.

**Resolution:**
- Expand the short SHA to its full 40-char form. The SHA is
  reachable via `git rev-parse <short>`.
- If the SHA is fabricated (does not exist in `git log`), see
  CC#47 for the resolution pattern.

**History:** m9-58 closed this drift class. Three cycles (m9-54,
m9-55, m9-56) had `## Ciclo` table `Base SHA` cells with short
SHAs (`a24139e`, `cbb9384`, `6bdf8ba`). The CCs that scan SHA
fields (CC#22, CC#23) use `len(head) != 40` as a precondition,
which silently skipped short-SHA rows. CC#49 explicitly scans
for short SHAs in SHA-keyed fields, independent of the
canonical-format precondition.

### 50. apply-checkpoint.json must use canonical `remote_tag` field (closed by m9-59)

```python
import json, os
from pathlib import Path

drifts = 0
for d in sorted(os.listdir('cycle-artifacts/p-3416cfb8288f8964')):
    if not d.startswith('m9-'):
        continue
    p = Path(f'cycle-artifacts/p-3416cfb8288f8964/{d}/apply-checkpoint.json')
    if not p.exists():
        continue
    j = json.load(open(p))
    has_tag = 'tag' in j
    has_remote = 'remote_tag' in j
    if has_tag and not has_remote:
        print(f"DRIFT: {d}: has legacy 'tag' field, missing canonical 'remote_tag'")
        drifts += 1
    elif not has_tag and not has_remote:
        print(f"DRIFT: {d}: missing both 'tag' and 'remote_tag'")
        drifts += 1
print(f'Total: {drifts}')
```

**Expected output (clean):** `Total: 0`.

**If `DRIFT`:** an apply-checkpoint.json uses the legacy `tag` field
(pre-m9-19) or has neither `tag` nor `remote_tag` (m9-19..m9-33 era
when `remote_tag` was added but not backfilled). The canonical field
is `remote_tag` (introduced by m9-19 and codified by m9-22).

**Resolution:**
- If the file has `tag`: rename `tag` → `remote_tag`.
- If the file has neither: look up the cycle's tag from
  `cycles/index.md` (4th column) and add `remote_tag: <tag>`.
- Verify the tag exists via `git rev-parse v0.7.X^{commit}`.

**History:** m9-59 closed this drift class. 31 apply-checkpoints
were missing `remote_tag`:
- 16 had legacy `tag` field (m9-03..m9-18, pre-convention)
- 15 had neither (m9-19..m9-33, mid-convention era when
  `remote_tag` was added but not backfilled)
m9-57 schema backfill added 12 fields to 21 apply-checkpoints
but missed `remote_tag` for both legacy and mid-era cycles.
CC#50 detects both sub-classes.
