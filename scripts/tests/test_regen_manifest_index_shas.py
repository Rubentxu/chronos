#!/usr/bin/env python3
"""Unit tests for scripts/regen_manifest_index_shas.py.

Zero dependencies: plain `unittest`, runnable as

    python3 scripts/tests/test_regen_manifest_index_shas.py

The tests build a throwaway repository root on disk (never the real vault), so
they exercise the same code path CC#4 does: row parsing, self-row preservation,
dangling-row preservation, `--check`/`--dry-run` exit codes, and the fixpoint
loop across two manifests that list each other's files.
"""

from __future__ import annotations

import hashlib
import contextlib
import io
import os
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import regen_manifest_index_shas as regen  # noqa: E402

ZEROS = "0" * 64


def sha_of(path: str) -> str:
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


class RegenFixture(unittest.TestCase):
    def setUp(self) -> None:
        self.root = tempfile.mkdtemp(prefix="regen-repro-")
        self.addCleanup(shutil.rmtree, self.root)

    # -- helpers ---------------------------------------------------------------
    def write(self, relpath: str, content: str) -> str:
        path = os.path.join(self.root, relpath)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(content)
        return path

    def manifest(
        self,
        change: str,
        rows: list[tuple[str, str, str]],
        *,
        self_row: bool = False,
    ) -> str:
        rel = f".sddk-knowledge/p-test/changes/archive/{change}/archive-manifest.md"
        body = ["# archive-manifest", "", "## Artifact index", "", "| Kind | Path | SHA-256 |", "|---|---|---|"]
        if self_row:
            body.append(f"| archive-manifest (this file) | `{rel}` | `{ZEROS}` |")
        body += [f"|{label}| `{path}` | `{sha}` |" for label, path, sha in rows]
        body.append("")
        return self.write(rel, "\n".join(body))

    def run_main(self, *argv: str) -> int:
        return regen.main([*argv, "--root", self.root])

    def read(self, path: str) -> str:
        with open(path, encoding="utf-8") as fh:
            return fh.read()


class TestCheckMode(RegenFixture):
    def test_clean_manifest_exits_zero(self) -> None:
        src = self.write("src.txt", "hello\n")
        rel = "src.txt"
        self.manifest("m9-01-x", [("source", rel, sha_of(src))], self_row=True)
        self.assertEqual(self.run_main("--check"), 0)

    def test_stale_row_is_reported_and_exits_one(self) -> None:
        self.write("src.txt", "hello\n")
        self.manifest("m9-01-x", [("source", "src.txt", "deadbeef" + "0" * 56)])
        self.assertEqual(self.run_main("--check"), 1)

    def test_stale_row_is_named_in_check_output(self) -> None:
        # `--check` is meant to be actionable in CI logs, so the offending row
        # path AND the stale value must be printed, not just a summary count.
        self.write("src.txt", "hello\n")
        stale = "deadbeef" + "0" * 56
        self.manifest("m9-01-x", [("source", "src.txt", stale)])
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = self.run_main("--check")
        self.assertEqual(rc, 1)
        out = buf.getvalue()
        self.assertIn("src.txt", out)
        self.assertIn(stale, out)

    def test_dry_run_reports_but_exits_zero_and_writes_nothing(self) -> None:
        self.write("src.txt", "hello\n")
        mf = self.manifest("m9-01-x", [("source", "src.txt", "deadbeef" + "0" * 56)])
        before = self.read(mf)
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = self.run_main("--dry-run")
        self.assertEqual(rc, 0)
        self.assertEqual(self.read(mf), before)
        self.assertIn("stale row(s) listed above", buf.getvalue())

    def test_check_and_dry_run_are_exclusive(self) -> None:
        self.write("src.txt", "hello\n")
        self.manifest("m9-01-x", [("source", "src.txt", ZEROS)])
        self.assertEqual(self.run_main("--check", "--dry-run"), 2)

    def test_missing_target_is_preserved_and_invisible_to_check(self) -> None:
        # CC#4 guards every comparison with `[ -f "$path" ]`, so a row whose
        # target was deleted is invisible to the gate. `--check` must agree
        # (exit 0) and the row must survive `--check` byte-for-byte.
        original = "ab" * 32
        mf = self.manifest("m9-01-x", [("source", "gone.txt", original)])
        self.assertEqual(self.run_main("--check"), 0)
        self.assertIn(f"`{original}`", self.read(mf))
        self.assertEqual(self.run_main(), 0)
        self.assertIn(f"`{original}`", self.read(mf), "dangling row must stay byte-identical")


class TestRewrite(RegenFixture):
    def test_rewrite_restores_the_true_sha(self) -> None:
        src = self.write("src.txt", "hello\n")
        mf = self.manifest("m9-01-x", [("source", "src.txt", "deadbeef" + "0" * 56)])
        self.assertEqual(self.run_main(), 0)
        self.assertIn(f"`{sha_of(src)}`", self.read(mf))
        self.assertEqual(self.run_main("--check"), 0)

    def test_self_row_is_preserved(self) -> None:
        mf = self.manifest("m9-01-self", [], self_row=True)
        rel = os.path.relpath(mf, self.root)
        self.assertEqual(self.run_main(), 0)
        self.assertIn(f"| archive-manifest (this file) | `{rel}` | `{ZEROS}` |", self.read(mf))
        self.assertEqual(self.run_main("--check"), 0)

    def test_fixpoint_across_interlinked_manifests(self) -> None:
        # manifest A lists a source file and manifest B; B lists its own source.
        # Rewriting B changes B's bytes, which changes A's row for B: the fixpoint
        # loop must converge and `--check` must then be clean.
        self.write("a.txt", "a\n")
        self.write("b.txt", "b\n")
        b = self.manifest("m9-02-b", [("source", "b.txt", ZEROS)])
        a = self.manifest(
            "m9-01-a",
            [("source", "a.txt", ZEROS), ("manifest b", os.path.relpath(b, self.root), ZEROS)],
        )
        self.assertEqual(self.run_main(), 0)
        self.assertIn(f"`{sha_of(b)}`", self.read(a), "A must record B's post-rewrite bytes")
        self.assertEqual(self.run_main("--check"), 0)

    def test_missing_manifest_argument_exits_two(self) -> None:
        self.assertEqual(self.run_main(os.path.join(self.root, "nope.md")), 2)

    def test_no_manifests_found_exits_two(self) -> None:
        self.assertEqual(self.run_main(), 2)

    def test_explicit_manifest_argument_is_used(self) -> None:
        src = self.write("src.txt", "hello\n")
        mf = self.manifest("m9-01-x", [("source", "src.txt", ZEROS)])
        self.assertEqual(self.run_main(mf), 0)
        self.assertIn(f"`{sha_of(src)}`", self.read(mf))


class TestDiscovery(RegenFixture):
    def test_default_manifests_finds_archives(self) -> None:
        self.manifest("m9-02-b", [])
        self.manifest("m9-01-a", [])
        found = regen.default_manifests(self.root)
        self.assertEqual(len(found), 2)
        self.assertTrue(all(p.endswith("archive-manifest.md") for p in found))


if __name__ == "__main__":
    unittest.main(verbosity=2)
