#!/usr/bin/env python3
"""Releases, the way masar does them.

Works out the next version from the Conventional Commits since the last `v*`
tag, with git-cliff's rules (cliff.toml): a breaking change bumps the major
version (the minor one before 1.0), feat the minor one, fix and perf the patch,
and every other commit releases nothing. The first release, with no tag before
it, is 0.1.0.

The version lives in the tags. src-tauri/Cargo.toml holds a placeholder, which
CI stamps with the release's version before it builds. There are no version
bump commits. .github/workflows/release.yml runs this.

Usage:
    python scripts/release.py --dry-run         the next version and its commits
    python scripts/release.py --stamp <version> write it into src-tauri/Cargo.toml
    python scripts/release.py --tag [<version>] tag HEAD v<version>, never pushed

--dry-run and --tag also write `version=` to $GITHUB_OUTPUT when it is set,
empty for no release.
"""

import os
import re
import subprocess
import sys
from pathlib import Path
from typing import NamedTuple

ROOT = Path(__file__).resolve().parent.parent

LEVELS = ["none", "patch", "minor", "major"]
# The commit types that release, and what each bumps (cliff.toml's commit_parsers).
TYPE_LEVEL = {"feat": "minor", "fix": "patch", "perf": "patch"}

# A subject's start: a type, an optional scope, "!" for a breaking change, then ":".
HEADER = re.compile(r"^(?P<type>\w+)(?:\([^)]*\))?(?P<bang>!)?:")
BREAKING_FOOTER = re.compile(r"^BREAKING[ -]CHANGE: ", re.MULTILINE)

# The version of the first release, when no tag comes before it (cliff.toml's initial_tag).
FIRST_VERSION = "0.1.0"
VERSION = re.compile(r"^(\d+)\.(\d+)\.(\d+)$")
TAG_PREFIX = "v"
TAG_PATTERN = f"{TAG_PREFIX}*"

# The manifest CI stamps, and its [package] table's version line.
MANIFEST = Path("src-tauri") / "Cargo.toml"
PACKAGE_VERSION = re.compile(
    r'(^\[package\][^\[]*?^version\s*=\s*")[^"]*(")', re.MULTILINE | re.DOTALL
)

USAGE_EXIT = 2


class Commit(NamedTuple):
    hash: str
    message: str


class Plan(NamedTuple):
    # The next version, or None when no commit since the last tag releases anything.
    version: str | None
    # The commits since the last tag that release something, oldest first.
    commits: list[Commit]


def level_of(message: str) -> str:
    """How much a commit message bumps the version: any breaking change the most, then its type."""
    header = HEADER.match(message)

    if (header and header.group("bang")) or BREAKING_FOOTER.search(message):
        return "major"

    return TYPE_LEVEL.get(header.group("type"), "none") if header else "none"


def next_version(last_tag: str | None, messages: list[str]) -> str | None:
    """The version after `last_tag` ("v0.4.2", or None before the first release) for these commit
    messages; None when none of them releases anything. Before 1.0 a breaking change bumps the minor
    version, as feat does."""
    level = max((level_of(m) for m in messages), key=LEVELS.index, default="none")

    if level == "none":
        return None
    if last_tag is None:
        return FIRST_VERSION

    parts = VERSION.match(last_tag[len(TAG_PREFIX) :])
    if not last_tag.startswith(TAG_PREFIX) or not parts:
        raise ValueError(f"the tag {last_tag} is not v<major>.<minor>.<patch>")

    major, minor, patch = (int(p) for p in parts.groups())

    if level == "major" and major > 0:
        return f"{major + 1}.0.0"
    if level == "patch":
        return f"{major}.{minor}.{patch + 1}"
    return f"{major}.{minor + 1}.0"


def git(folder: Path, *args: str) -> str:
    return subprocess.run(
        ["git", *args],
        cwd=folder,
        check=True,
        capture_output=True,
        text=True,
        encoding="utf-8",
    ).stdout.strip()


def last_tag(folder: Path) -> str | None:
    """The latest release tag HEAD descends from, or None before the first release."""
    run = subprocess.run(
        ["git", "describe", "--tags", "--abbrev=0", "--match", TAG_PATTERN],
        cwd=folder,
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    return run.stdout.strip() if run.returncode == 0 else None


def commits_since(folder: Path, tag: str | None) -> list[Commit]:
    """The commits after `tag` up to HEAD (all of HEAD's when `tag` is None), oldest first."""
    # %x00 separates commits, since a body can hold blank lines.
    log = git(folder, "log", "--reverse", "--format=%h%n%B%x00", f"{tag}..HEAD" if tag else "HEAD")
    commits = []

    for entry in (e.strip() for e in log.split("\0")):
        if entry:
            short, _, message = entry.partition("\n")
            commits.append(Commit(short, message.strip()))

    return commits


def plan(folder: Path) -> Plan:
    """The next release of the repository in `folder`: its version and the commits that make it one."""
    tag = last_tag(folder)
    commits = [c for c in commits_since(folder, tag) if level_of(c.message) != "none"]

    return Plan(next_version(tag, [c.message for c in commits]), commits)


def stamp(folder: Path, version: str) -> None:
    """Writes `version` into the [package] table of src-tauri/Cargo.toml in `folder`, keeping the rest as it is."""
    if not VERSION.match(version):
        raise ValueError(f"{version} is not <major>.<minor>.<patch>")

    manifest = folder / MANIFEST

    # newline="" both ways, so a CRLF checkout keeps its line endings.
    with open(manifest, encoding="utf-8", newline="") as f:
        contents = f.read()

    stamped, count = PACKAGE_VERSION.subn(rf"\g<1>{version}\g<2>", contents, count=1)

    if count != 1:
        raise ValueError(f"no [package] version in {MANIFEST}")

    with open(manifest, "w", encoding="utf-8", newline="") as f:
        f.write(stamped)


def tag_release(folder: Path, version: str | None = None) -> str | None:
    """Tags HEAD in `folder` v<version>: `version` when given, otherwise the next one; HEAD's own release
    tag when it has one already (a run again after a failure). The version, or None for no release.
    Nothing is pushed."""
    if version is not None and not VERSION.match(version):
        raise ValueError(f"{version} is not <major>.<minor>.<patch>")

    at_head = git(folder, "tag", "--points-at", "HEAD", "--list", TAG_PATTERN).splitlines()

    for tag in at_head:
        own = tag[len(TAG_PREFIX) :]
        if (version and own == version) or (not version and VERSION.match(own)):
            return own

    next_one = version or plan(folder).version

    if next_one:
        git(folder, "tag", f"{TAG_PREFIX}{next_one}")

    return next_one


def output(version: str | None) -> None:
    """Adds `version=<version>` to the step's outputs on GitHub Actions (empty for no release)."""
    path = os.environ.get("GITHUB_OUTPUT")

    if path:
        with open(path, "a", encoding="utf-8") as f:
            f.write(f"version={version or ''}\n")


def main(argv: list[str]) -> int:
    flag, value = (argv + [None, None])[:2]

    if flag == "--dry-run":
        next_release = plan(ROOT)
        print(f"next version: {next_release.version}" if next_release.version else "no release")
        for commit in next_release.commits:
            print(f"  {commit.hash} {commit.message.splitlines()[0]}")
        output(next_release.version)
    elif flag == "--stamp" and value:
        stamp(ROOT, value)
        print(f"{MANIFEST.as_posix()}: version {value}")
    elif flag == "--tag":
        version = tag_release(ROOT, value or None)
        print(f"tagged {TAG_PREFIX}{version}" if version else "no release")
        output(version)
    else:
        print(
            "usage: python scripts/release.py --dry-run | --stamp <version> | --tag [<version>]",
            file=sys.stderr,
        )
        return USAGE_EXIT

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
