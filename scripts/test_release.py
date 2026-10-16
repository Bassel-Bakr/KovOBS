"""Checks release.py: each commit's level and the next version as pure functions, then the plan, the tag
and the stamp against real git in a temporary repository. Mirrors masar's scripts/release.test.ts.

    python -m unittest discover -s scripts -p "test_*.py"
"""

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

from release import FIRST_VERSION, level_of, next_version, plan, stamp, tag_release


class LevelOf(unittest.TestCase):
    def test_bumps_for_feat_fix_and_perf_and_for_any_breaking_change(self):
        self.assertEqual(level_of("feat(#76): a view"), "minor")
        self.assertEqual(level_of("fix: a crash"), "patch")
        self.assertEqual(level_of("perf(search): faster"), "patch")
        self.assertEqual(level_of("feat!: a new API"), "major")
        self.assertEqual(level_of("refactor(core)!: rename"), "major")
        self.assertEqual(level_of("fix: x\n\nBREAKING CHANGE: the old route is gone"), "major")
        self.assertEqual(level_of("chore: x\n\nBREAKING-CHANGE: gone"), "major")

    def test_skips_every_other_commit(self):
        self.assertEqual(level_of("docs(#78): words"), "none")
        self.assertEqual(level_of("chore(#76): bump version"), "none")
        self.assertEqual(level_of("Merge pull request #79 from Bassel-Bakr/78-update-docs"), "none")
        self.assertEqual(level_of("feature: not a type"), "none")


class NextVersion(unittest.TestCase):
    def test_releases_nothing_without_a_releasing_commit(self):
        self.assertIsNone(next_version("v0.4.2", ["docs: a", "test: b"]))
        self.assertIsNone(next_version(None, []))

    def test_starts_at_the_first_version(self):
        self.assertEqual(next_version(None, ["fix: a"]), FIRST_VERSION)

    def test_bumps_by_the_highest_level_among_the_commits(self):
        self.assertEqual(next_version("v0.4.2", ["fix: a", "perf: b"]), "0.4.3")
        self.assertEqual(next_version("v0.4.2", ["fix: a", "feat: b"]), "0.5.0")
        self.assertEqual(next_version("v1.4.2", ["feat: b", "fix: a"]), "1.5.0")
        self.assertEqual(next_version("v1.4.2", ["feat!: b", "fix: a"]), "2.0.0")

    def test_bumps_the_minor_version_for_a_breaking_change_before_1_0(self):
        self.assertEqual(next_version("v0.18.0", ["feat!: b"]), "0.19.0")

    def test_refuses_a_tag_that_is_not_a_release_version(self):
        with self.assertRaisesRegex(ValueError, "is not v"):
            next_version("0.4.2", ["fix: a"])
        with self.assertRaisesRegex(ValueError, "is not v"):
            next_version("v0.4", ["fix: a"])


MANIFEST = """[package]
name = "kovobs"
version = "0.0.0"
edition = "2024"

[dependencies.serde]
version = "1"
"""


class ARepository(unittest.TestCase):
    def setUp(self):
        self.folder = Path(tempfile.mkdtemp(prefix="release-"))

    def tearDown(self):
        shutil.rmtree(self.folder, ignore_errors=True)

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args], cwd=self.folder, check=True, capture_output=True, text=True
        ).stdout

    def commit(self, message: str) -> None:
        self.git(
            "-c", "user.name=Test", "-c", "user.email=test@example.com",
            "commit", "-q", "--allow-empty", "-m", message,
        )

    def test_plans_tags_and_stamps_each_release(self):
        self.git("init", "-q")
        self.commit("chore: start")
        self.assertEqual(plan(self.folder), (None, []))
        self.assertIsNone(tag_release(self.folder))

        self.commit("feat(#1): the first view")
        first = plan(self.folder)
        self.assertEqual(first.version, FIRST_VERSION)
        self.assertEqual([c.message for c in first.commits], ["feat(#1): the first view"])
        self.assertEqual(tag_release(self.folder), FIRST_VERSION)
        # A run again after a failure keeps the tag HEAD already has.
        self.assertEqual(tag_release(self.folder), FIRST_VERSION)
        self.assertIsNone(plan(self.folder).version)

        self.commit("docs: words")
        self.commit("fix(#2): a crash\n\nIt crashed.")
        second = plan(self.folder)
        self.assertEqual(second.version, "0.1.1")
        self.assertEqual([c.message for c in second.commits], ["fix(#2): a crash\n\nIt crashed."])
        self.assertEqual(tag_release(self.folder, "0.2.0"), "0.2.0")
        self.assertEqual(tag_release(self.folder, "0.2.0"), "0.2.0")
        self.assertEqual(self.git("tag", "--list").split(), ["v0.1.0", "v0.2.0"])
        with self.assertRaisesRegex(ValueError, "is not"):
            tag_release(self.folder, "next")

    def test_stamps_only_the_package_version(self):
        manifest = self.folder / "src-tauri" / "Cargo.toml"
        manifest.parent.mkdir()
        manifest.write_text(MANIFEST, encoding="utf-8")

        stamp(self.folder, "0.19.0")

        self.assertEqual(manifest.read_text(encoding="utf-8"), MANIFEST.replace('"0.0.0"', '"0.19.0"'))
        with self.assertRaisesRegex(ValueError, "is not"):
            stamp(self.folder, "v0.19.0")

    def test_stamping_keeps_crlf_line_endings(self):
        manifest = self.folder / "src-tauri" / "Cargo.toml"
        manifest.parent.mkdir()
        manifest.write_bytes(MANIFEST.replace("\n", "\r\n").encode())

        stamp(self.folder, "0.19.0")

        expected = MANIFEST.replace('"0.0.0"', '"0.19.0"').replace("\n", "\r\n").encode()
        self.assertEqual(manifest.read_bytes(), expected)


if __name__ == "__main__":
    unittest.main()
