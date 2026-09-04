#!/usr/bin/env python3
"""Set the crate version from the conventional commits on this branch.

The target is always computed from the base branch's version, never from the
current one:

    target = bump(version on base, highest level in base..HEAD)

That makes the script idempotent and order-independent. A branch that lands a
fix and then a feature ends up on a minor bump either way, and re-running never
double-bumps. Run it as often as you like; it only writes when the answer
changes.
"""

import os
import re
import subprocess
import sys

MANIFEST = "src-tauri/Cargo.toml"
PACKAGE = "kovobs"

# Highest wins. `chore`, `docs`, `style`, `refactor`, `test` and friends imply
# no release on their own, matching the Release workflow's behaviour of skipping
# a version that already has a tag.
LEVELS = {"none": 0, "patch": 1, "minor": 2, "major": 3}
TYPE_LEVEL = {"feat": "minor", "fix": "patch", "perf": "patch"}

HEADER = re.compile(r"^(?P<type>[a-z]+)(?:\([^)]*\))?(?P<bang>!)?:")
VERSION = re.compile(r'^version\s*=\s*"([^"]+)"', re.MULTILINE)


def git(*args: str) -> str:
    return subprocess.run(
        ["git", *args], check=True, capture_output=True, text=True
    ).stdout.strip()


def level_of(message: str) -> str:
    subject, _, body = message.partition("\n")

    if "BREAKING CHANGE:" in body or "BREAKING-CHANGE:" in body:
        return "major"

    match = HEADER.match(subject)
    if not match:
        return "none"
    if match.group("bang"):
        return "major"

    return TYPE_LEVEL.get(match.group("type"), "none")


def highest_level(base_sha: str) -> str:
    # %x00 separates commits so a multi-line body can't be mistaken for one.
    log = git("log", "--format=%B%x00", f"{base_sha}..HEAD")
    commits = [c.strip() for c in log.split("\0") if c.strip()]

    return max(
        (level_of(c) for c in commits), key=lambda l: LEVELS[l], default="none"
    )


def bump(version: str, level: str) -> str:
    try:
        major, minor, patch = (int(p) for p in version.split("."))
    except ValueError:
        sys.exit(f"Can't bump '{version}': expected a plain MAJOR.MINOR.PATCH")

    if level == "major":
        return f"{major + 1}.0.0"
    if level == "minor":
        return f"{major}.{minor + 1}.0"
    return f"{major}.{minor}.{patch + 1}"


def version_at(ref: str) -> str:
    match = VERSION.search(git("show", f"{ref}:{MANIFEST}"))
    if not match:
        sys.exit(f"No version found in {MANIFEST} at {ref}")
    return match.group(1)


def issue_number(branch: str) -> str:
    # The repo's convention: branches are named <issue>-<slug>.
    match = re.match(r"(\d+)", branch)
    return match.group(1) if match else os.environ.get("PR_NUMBER", "")


def emit(**outputs: str) -> None:
    for key, value in outputs.items():
        print(f"{key}={value}")

    path = os.environ.get("GITHUB_OUTPUT")
    if path:
        with open(path, "a", encoding="utf-8") as f:
            for key, value in outputs.items():
                f.write(f"{key}={value}\n")


def main() -> None:
    base = os.environ.get("BASE_REF", "main")
    branch = os.environ.get("HEAD_REF") or git("rev-parse", "--abbrev-ref", "HEAD")

    base_sha = git("merge-base", f"origin/{base}", "HEAD")
    level = highest_level(base_sha)

    if level == "none":
        emit(changed="false", reason="no releasable commits")
        return

    target = bump(version_at(f"origin/{base}"), level)

    with open(MANIFEST, encoding="utf-8") as f:
        current = VERSION.search(f.read()).group(1)

    if current == target:
        emit(changed="false", version=target, reason="already at target")
        return

    subprocess.run(
        ["cargo", "set-version", "--package", PACKAGE, target], check=True
    )
    emit(changed="true", version=target, level=level, issue=issue_number(branch))


if __name__ == "__main__":
    main()
