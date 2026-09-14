# Spec: m9-83 CC#39 Total cycles off-by-one

> Spec is the contract.

## REQ-M9-83-01 — `Total cycles` field matches actual folder count

The `Total cycles` metadata field in
`.sddk-knowledge/p-3416cfb8288f8964/cycles/index.md` SHALL equal the
number of m9-* rows in that file (one row per cycle, m9-01 through
m9-NN inclusive). The current value (84) is 2 more than the actual
row count (82); this cycle closes the gap by correcting the value to
82.

**Scenarios:**
- `total_cycles_field_matches_index_row_count` — `python3 -c "
  import re; t=open('cycles/index.md').read().split('| Total cycles | ')[1].split(' |')[0];
  rows=len(re.findall(r'^\| (m9-\d+) \|', t, re.MULTILINE));
  assert int(t)==rows, f'field {t} != rows {rows}'; print('PASS')"` exits 0.
- `cc39_clean_after_fix` — `bash scripts/check_vault_drift.sh` exits 0.

## REQ-M9-83-02 — Lint and clippy clean

- `cargo fmt --all -- --check` exits 0.
- `cargo clippy --workspace --all-targets -- -D warnings` exits 0.

(No source files are touched, so these should be unchanged from main.)
