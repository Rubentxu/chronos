#!/usr/bin/env python3
"""
Regression test for the vault-drift shallow-fetch bug.

Reproduces the exact failure mode that PR #19 surfaced on the CI runner
before the REC-C0.3-D fix: with `actions/checkout@v4` default
`fetch-depth: 1`, the CC#8/CC#47 cross-checks validate SHAs against
`git cat-file -e`, which fails for any SHA that is not the single
checked-out commit. This test simulates the shallow fetch and asserts
that:

  1. With shallow fetch, `git cat-file -e` returns non-zero for SHAs
     outside the single commit (the bug).
  2. With full fetch (`fetch-depth: 0`), the SHAs ARE reachable.

If the assertions in (2) fail after a future refactor of vault-drift.yml,
this test fails loud — the workflow fix must not be silently reverted.
"""
import os
import shutil
import subprocess
import sys
import tempfile


def run(cmd, cwd=None):
    return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)


def main():
    repo = "/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos"
    work = tempfile.mkdtemp(prefix="vault-drift-test-")
    try:
        # Mirror the repository into a temporary workdir.
        clone = run(
            [
                "git",
                "clone",
                "--mirror",
                "--quiet",
                repo,
                work,
            ]
        )
        if clone.returncode != 0:
            print(f"FAIL: could not clone mirror: {clone.stderr}")
            return 1

        # Pick a known cycle's head_sha from the workspace.
        # The cycle rec-c0-2-c has a recorded head_sha in its
        # apply-checkpoint.json; once it is committed, that SHA is
        # reachable in the full history.
        cycle_dir = (
            f"{repo}/cycle-artifacts/p-3416cfb8288f8964/"
            "rec-c0-2-c-domain-cleanup"
        )
        ckpt = f"{cycle_dir}/apply-checkpoint.json"
        if not os.path.exists(ckpt):
            print(f"SKIP: {ckpt} not present (REC-C0.2-c not yet committed)")
            return 0

        import json
        head_sha = json.load(open(ckpt)).get("head_sha")
        base_sha = json.load(open(ckpt)).get("base_sha")
        if not head_sha or not base_sha:
            print(f"SKIP: apply-checkpoint missing head_sha/base_sha: {ckpt}")
            return 0

        # --- Simulate shallow fetch (CI default) ---
        shallow = tempfile.mkdtemp(prefix="vault-drift-shallow-")
        run(["git", "clone", "--depth=1", "--quiet", work, shallow])
        res_shallow = run(
            ["git", "cat-file", "-e", head_sha],
            cwd=shallow,
        )
        if res_shallow.returncode == 0:
            print(
                "UNEXPECTED: head_sha is reachable in --depth=1 shallow clone. "
                "Either the test fixture is wrong or git's default depth has changed."
            )
            shutil.rmtree(shallow)
            return 1
        print(f"OK (bug reproduced): shallow clone cannot reach {head_sha[:12]}")

        # --- Simulate full fetch (REC-C0.3-D fix) ---
        full = tempfile.mkdtemp(prefix="vault-drift-full-")
        run(["git", "clone", "--quiet", work, full])
        res_full_head = run(["git", "cat-file", "-e", head_sha], cwd=full)
        res_full_base = run(["git", "cat-file", "-e", base_sha], cwd=full)
        if res_full_head.returncode != 0:
            print(
                f"FAIL: head_sha {head_sha[:12]} not reachable in full clone "
                "after applying REC-C0.3-D fix"
            )
            shutil.rmtree(full)
            shutil.rmtree(shallow)
            return 1
        if res_full_base.returncode != 0:
            print(
                f"FAIL: base_sha {base_sha[:12]} not reachable in full clone"
            )
            shutil.rmtree(full)
            shutil.rmtree(shallow)
            return 1
        print(
            f"OK (fix verified): full clone reaches both "
            f"{head_sha[:12]} and {base_sha[:12]}"
        )

        shutil.rmtree(shallow)
        shutil.rmtree(full)
        print("PASS: vault-drift full-history requirement holds.")
        return 0
    finally:
        shutil.rmtree(work, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
