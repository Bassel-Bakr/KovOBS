# Working in This Repository

Conventions and commands for KovOBS. This file is loaded for you; the skills in
`.agents/skills/` are not, so read the relevant one before starting on its area.

`CLAUDE.md` is one line importing this file, and is not redundant: Claude Code
does not pick `AGENTS.md` up on its own. Deleting it silently drops everything
below out of context, which is how this was found.

## What the app is

Desktop app (Tauri: Rust backend, Angular frontend) that watches KovaaK's and
Aimbeast stat files, and on each finished run tells OBS to save its replay
buffer, then trims that buffer into a clip with FFmpeg.

It runs unattended while the user plays a fullscreen game. **A failure the user
cannot see is the worst outcome** — prefer surfacing an error over failing
silently, and never let one bad clip end the session.

## Layout

| Path | |
|---|---|
| `src-tauri/src/kovobs/` | orchestration: `runs.rs` watches the stat folders, `clips.rs` turns saved buffers into clips |
| `src-tauri/src/aimbeast/` | reading Aimbeast's statistics and training log |
| `src-tauri/src/ffmpeg.rs` | two-pass trim, then optional user args, then the optional stats metadata pass |
| `src-tauri/src/config.rs` | all config structs and defaults |
| `src-tauri/src/programs.rs` | which program a process runs, including a Proton game on Linux (from Wine's `Z:\` command line), matched to the config by file name; builds on its own so it can be checked on Linux |
| `src-tauri/src/cmds/` | commands callable from the frontend, one file per concern |
| `src/app/home/` | the settings window |
| `.github/workflows/` | release and version bump |

Clips are written to `<clips_folder>/<scenario>/<name>.mp4` — one directory per
scenario, not flat.

---

## Commands

Run all from the **repo root**. `cd src-tauri` breaks relative paths for
following commands.

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

```bash
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
```

```bash
npx ng test
```

```bash
npx ng build
```

Clippy is clean, and CI runs it with `-D warnings`. Any warning you see is
from your change.

---

## Commit rules

1. **Read the branch immediately before writing the message**, every time:

   ```bash
   git branch --show-current
   ```

   The branch name starts with the issue number. Format:
   `type(#N): imperative subject`. Example: branch
   `68-add-agent-agnostic-skills` → `docs(#68): ...`. Getting this from
   conversation memory instead of the branch has produced wrong numbers twice.

2. **No AI attribution trailers.** No `Co-Authored-By` for tools, no generated-by
   footers, in commits or PR descriptions.

3. Types in use: `feat`, `fix`, `chore`, `docs`, `ci`, `refactor`.

4. `dist/` is gitignored. If it appears in `git status`, something regressed.

---

## Environment notes

- **Shell heredocs mangle backslashes.** Writing a patch script inline with
  `<<'EOF'` silently halves `\\` sequences, corrupting Rust/Windows-path
  strings. Write the script to a file with a file-writing tool, then run it.
- Line endings: the working tree is CRLF, the repo LF. `LF will be replaced by
  CRLF` warnings on commit are normal.
- Pushing requires credentials this environment does not have. Prepare commits
  locally and hand the push command to the user.

---

## CI

`.github/workflows/release.yml` runs on every push to `main`, and on manual
dispatch. It releases when a `feat`, `fix`, `perf` or breaking commit landed
since the last `v*` tag: `scripts/release.py` works out the version from those
commits (rules in `cliff.toml`; before 1.0 a breaking change bumps the minor),
tags the commit, stamps the version into the build, and publishes. A docs- or
chore-only push releases nothing. The same scheme as masar.

The Linux AppImage is not shipped as Tauri builds it. `scripts/linux-portable.sh`
removes the build machine's bundled `libwayland-*` and repacks it, because with
them WebKit cannot create its EGL display on distros with a newer Mesa (Arch,
CachyOS) and the window stays blank. It also publishes the same app as a
`tar.gz` for systems without FUSE. Do not drop that step from `release.yml`.

**The version lives in the tags, not in a file.** `src-tauri/Cargo.toml` holds
the placeholder `0.0.0`; never set a real version there. The commit types are
what release, so get them right. The `chore(#N): bump version` commits in the
history are from the old scheme, not a convention to copy.

`.github/workflows/checks.yml` runs formatting, clippy (`-D warnings`), the
Rust tests, a generated-types freshness check, the frontend tests and build,
the release script's tests and a ShellCheck of `scripts/*.sh` -- on pull
requests, on pushes to `main`, and on demand. `release.yml` runs the Rust tests
before it builds.

The build caches the registry and `target/` via a Rust cache action. Do not
reintroduce a compilation cache with a per-object remote backend; it fragmented
the Actions cache into ~1,400 entries and 5.6 GB of the 10 GB budget, evicting
the entries it needed.

---

## Skills

Depth lives in `.agents/skills/`. Read the one that covers what you are about
to touch.

| Skill | For |
|---|---|
| `aimbeast-stat-files` | anything reading Aimbeast's statistics or training log |
| `ffmpeg-two-pass` | changing how clips are trimmed, user args applied, or tags written and kept |
| `add-config-option` | adding a persisted setting end to end |
| `preview-ui-without-tauri` | seeing and measuring UI changes |
| `verify-by-effect` | anything that reports success but produces no effect |
