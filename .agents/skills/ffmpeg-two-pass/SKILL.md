# FFmpeg Two-Pass Trimming

How clips are produced, and the rules that keep a user's own FFmpeg arguments
from costing them a clip. Read before changing anything in
`src-tauri/src/ffmpeg.rs`.

## The design

A run finishes → OBS writes its replay buffer → the buffer is trimmed into a
clip. The user can supply their own FFmpeg arguments in three slots
(`global_args`, `input_args`, `output_args`).

**The two passes are separate on purpose.**

| | Pass 1 — trim | Pass 2 — user args |
|---|---|---|
| Built by | `trim_args` | `extra_command` |
| Arguments | hardcoded | the user's three slots |
| Output | `<name>.trimming.<ext>` | `<name>.<ext>` |
| Runs when | always | only if any slot is non-empty |

When every slot is empty the trim writes straight to the final path and pass 2
is skipped entirely.

**The user's arguments never touch the trim.** `trim_args` takes no config. This
is the property that makes a bad argument survivable: the trim has already
produced a usable clip before the user's arguments are parsed.

---

## Mandatory rules

1. **Never let a user-argument failure lose the clip.** If parsing the slots
   fails, or pass 2 exits non-zero, promote the trimmed intermediate to the
   output path (`keep_trimmed`) and treat the clip as saved.
2. **Never pass config into `trim_args`.** Doing so makes a typo in a user
   setting fatal, and there is no earlier artifact to fall back to.
3. **Never let one clip's failure end the session.** Callers report and
   continue to the next event.
4. **Drain stderr on its own task.** `run` reads progress from stdout; if the
   stderr pipe fills while it is doing that, FFmpeg blocks writing and neither
   side finishes. This is a deadlock, not a slowdown.
5. **Report the command line with the error.** The arguments are the user's; the
   message is unusable without showing what actually ran.
6. **Quote for display, never backslash-escape.** Windows paths are full of
   backslashes; escaping them makes the printed command differ from the one
   that ran. See `display_command`.
7. **Delete the destination before renaming.** `rename` refuses to overwrite on
   Windows, and a failed pass may have left a partial file.

## Recommendations

- Impose an option on a pass only if it does real work. Currently: `-progress
  pipe:1` (progress parsing), `-y` (no stdin prompt), and `-hide_banner
  -loglevel error -nostats` (keeps captured stderr to the actual error).
- Put imposed options **before** the user's, so the user can override any of them.
- Prefer `-c copy` for the trim: it should be cheap and lossless.
- Use `-map 0?` on the trim. Without an explicit map, FFmpeg selects one stream
  per type and silently drops extra audio and video sources.

---

## Argument parsing

The three slots are free text, parsed by `src-tauri/src/args.rs`, not by a
shell. The rules exist because of Windows:

- space, tab, CR and LF separate arguments
- `'` and `"` both group, identically; the quotes are removed
- **a backslash is always literal** — never an escape
- except a backslash immediately before a line ending, which is a continuation

POSIX-style splitters (`shell-words`, `shlex`) treat backslashes as escapes and
silently corrupt every Windows path. Do not swap the parser for one.

An unterminated quote is an error, and reaches the user as a failure to read
the arguments — which then keeps the trimmed clip.

---

## Failure behaviour

| What fails | Result |
|---|---|
| Slot fails to parse | trimmed clip kept, reason logged |
| Pass 2 exits non-zero | trimmed clip kept, stderr + command logged |
| Pass 1 (the trim) fails | no clip — nothing to fall back to. Report, notify, continue the session |

Pass 1 failing is the only case that loses a clip. It should be rare, because
nothing user-supplied reaches it.

---

## Verification criteria

Changing this file requires all of:

- `cargo test --manifest-path src-tauri/Cargo.toml --lib` passes.
- A deliberately invalid user argument still produces a clip, and logs both the
  FFmpeg error and the full command.
- A deliberately broken trim reports and the session keeps listening for the
  next run.
- The logged command line reproduces when pasted into a terminal.

Testing the failure path means editing an argument to be invalid and running
the real app — there is no unit test for it. `display_command` and the argument
parser are unit-tested; the pass orchestration is not.
