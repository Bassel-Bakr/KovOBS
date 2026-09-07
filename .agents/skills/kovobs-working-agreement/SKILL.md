# Working in This Repository

Conventions and commands for KovOBS. Read before the first change in a session.

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
| `src-tauri/src/kovobs.rs` | orchestration: OBS events, stat watchers, task set |
| `src-tauri/src/ffmpeg.rs` | two-pass trim, then optional user args |
| `src-tauri/src/config.rs` | all config structs and defaults |
| `src-tauri/src/cmds/` | commands callable from the frontend |
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
npx ng build
```

Three clippy warnings are pre-existing (`cmds/ffmpeg.rs` redundant import,
`cmds/mod.rs` module inception, `aimbeast/scenario_statistics.rs` partial
comparison). Do not treat them as caused by your change.

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

`.github/workflows/release.yml` triggers on a push to `main` touching
`src-tauri/Cargo.toml`, and on manual dispatch. It builds and publishes.

**It runs no tests, no clippy, and nothing on pull requests.** Tests gate
nothing today — verify locally before claiming a change is good.

The build caches the registry and `target/` via a Rust cache action. Do not
reintroduce a compilation cache with a per-object remote backend; it fragmented
the Actions cache into ~1,400 entries and 5.6 GB of the 10 GB budget, evicting
the entries it needed.

---

## Related skills

| Skill | For |
|---|---|
| `preview-ui-without-tauri` | seeing and measuring UI changes |
| `add-config-option` | adding a persisted setting end to end |
| `ffmpeg-two-pass` | changing how clips are trimmed or user args applied |
| `verify-by-effect` | anything that reports success but produces no effect |
