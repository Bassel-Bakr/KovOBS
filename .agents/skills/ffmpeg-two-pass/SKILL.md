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
8. **Every pass that writes MP4 keeps the tags.** It carries `-movflags
   +use_metadata_tags`; without it a remux drops every tag MP4 has no name for.
   See [Keeping metadata](#keeping-metadata).

## Recommendations

- Impose an option on a pass only if it does real work. Currently: `-progress
  pipe:1` (progress parsing), `-y` (no stdin prompt), `-hide_banner
  -loglevel error -nostats` (keeps captured stderr to the actual error), and
  `-movflags +use_metadata_tags` (keeps custom tags).
- Put imposed options **before** the user's, so the user can override any of them.
- Prefer `-c copy` for the trim: it should be cheap and lossless.
- Use `-map 0?` on the trim. Without an explicit map, FFmpeg selects one stream
  per type and silently drops extra audio and video sources.

---

## The metadata pass

With `embed_stats` on, `add_metadata` remuxes the finished clip once more
(`-c copy`) to add the run's stats CSV under the key `kovobs_kovaaks_stats`.
It runs after `trim` returns, from `kovobs/clips.rs`, never inside it, and a
failure only logs: the clip is already saved.

- The value goes through an `FFMETADATA1` file, not `-metadata`: a stats file
  can pass Windows' 32,767-character command line. `ffmetadata` escapes `\r`
  too: FFmpeg's reader ends a line at a bare one, which cut a CRLF CSV after
  its first line.
- `-map_metadata 1 -map_metadata 0` adds the stats and keeps the clip's own
  tags. The order matters: on a key both have, the first map wins, so the
  other way round a tag already in the clip (from the user's own `-metadata`,
  say) silently kept the stats out while the log reported them added.
  `-movflags +use_metadata_tags` is what lets MP4 store a custom key.
- The user's arguments cannot reach this pass, which takes no config. The most
  they can do is leave a clip it cannot remux, and then only the stats are lost.
- The tagged clip swaps in through `<name>.untagged.<ext>`, so a failed rename
  puts the original back rather than leaving no clip.

`real_ffmpeg_embeds_the_stats_readably` (ignored; needs `ffmpeg` and `ffprobe`
on the PATH) runs the whole pass on a clip that already has a stale tag under
the key, and reads the tag back, for MP4 and MKV.

### Keeping metadata

Checked with FFmpeg 8.1:

- Container tags are a flat list of text keys and text values: no nesting, no
  types. Structure has to live in the key name or be encoded in the value.
- A plain `-c copy` remux to MP4 keeps only the keys MP4 has a name for
  (`title`, `encoder`, ...) and silently drops the rest. `-movflags
  +use_metadata_tags` keeps them all, and adds nothing to an OBS buffer's tags.
- MKV keeps every tag without a flag but upper-cases the names:
  `KOVOBS_KOVAAKS_STATS`. Match key names case-insensitively, and prefer
  snake_case keys, since camelCase loses its word breaks.
- Later `-movflags` combine: `+faststart` adds to an earlier
  `+use_metadata_tags`, a bare `faststart` or `-use_metadata_tags` replaces it.
- Read tags back with `ffprobe -show_entries format_tags -of json`, which is
  exact. `-of default` on Windows writes `\n` as `\r\n`, doubling a CSV's
  carriage returns.
- In MP4 these keys are QuickTime "keys" metadata. FFmpeg-based readers see
  them; iTunes-style taggers (mutagen, TagLib), Explorer and players are not
  expected to, and re-encoding editors and upload sites drop them. Untested
  beyond FFmpeg.

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
