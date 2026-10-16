# Aimbeast Stat Files

What Aimbeast writes, when it writes it, and the one rule that will cost you
runs if you break it. Read before changing anything under
`src-tauri/src/aimbeast/` or the Aimbeast half of `src-tauri/src/kovobs/runs.rs`.

## Never hold a statistics file open

**Aimbeast opens its statistics files without sharing them. While a handle of
ours is open, its next write to that scenario fails — silently, without
touching the file.**

This cost every run played within `trim_padding_end` of the last one. The read
handle lived to the end of the handler, which spans the padding wait while the
clip is made, so the next run was never recorded and never clipped.

Read through `read_statistics`, which parses and drops the file together. Do not
keep the `File`, the reader, or anything holding them, across an `await`.

The symptom is deceptive. From outside it looks exactly like Aimbeast
throttling its own writes: two runs of the same scenario seconds apart produce
one score row, minutes apart produce two. Both records appear the moment KovOBS
is closed. Diagnose it by closing KovOBS and replaying, not by staring at gaps.

## The two file sets

Both are **UTF-16 with a BOM**. Decode the bytes; do not read them as UTF-8.

| | `Statistics/{Normal,Ranked,Custom}/<scenario>.json` | `Training Data/training_data_<year>.json` |
|---|---|---|
| shape | parallel arrays: `Date`, `Score`, `Accuracy`, `TTK`, ... | day key → scenario → totals |
| written | in place, every run | in place, every run |
| holds a duration | **no** | yes, indirectly |

All three folders are watched, and which one a scenario lands in is the only
thing that distinguishes them: `Custom` holds scenarios the player built, and
went unwatched long enough for custom scenarios to produce no clips at all.
Only the folders that exist are watched, since a player who has never built a
scenario has no `Custom` folder.

### Deriving how long a run lasted

The statistics file carries no duration, so the length comes from the sibling
training log, whose entries look like:

```json
{"19/9/2026": {"1 WALL 6 TARGETS": {"Completed Sessions Time": 120, "Completed Sessions": 2, "Total Time": 129}}}
```

**Every run of over 3 seconds is counted, however it ended**: timer, kill
limit, miss limit, dying, and touching an end block all go through the same
`EndSession`, which saves the run as completed. It saves the length rounded to
whole seconds, with pauses left out, and only when that is over 3
(`Save Scenario To Training Streak` checks `Time > 3.0`). So a run that ended
without a log write lasted under 3.5 seconds (`UNCOUNTED_RUN_MAX`).

This replaces an earlier note here, that only runs whose timer ran out are
counted. That was observed on runs of 3 seconds or less. The bytecode says
otherwise, and so does the player's own log: `SPHERE FRENZY BAZ 10S 1SHOT`
has a 10 second timer, yet logged 14 completed runs totalling 93 seconds.
Decompiled and checked live in `aimbeast-reverse-engineer`, `docs/REPORT.md`
section 15b.

The totals move by exactly the run's duration when it is counted, so measure
rather than assume:

```
length = ΔCompleted Sessions Time / ΔCompleted Sessions
```

between the reading taken for the previous run and this one. `ScenarioLengths`
keeps those readings, in memory and per scenario per day, primed at startup.

With nothing to measure against, the answer is the longer of the day's average
`Completed Sessions Time / Completed Sessions` and the run's **kill span**,
`(Kills - 1) x TBK` from the statistics file. Aimbeast's `TBK` is the average
gap between consecutive kills, so the gaps add back up to the time from the
first kill to the last. The kill span is a floor: it leaves out the time before
the first kill, and it is game time, so a 1.25x scenario reads about a quarter
long. The average is exact only for a scenario that always runs its timer
out. Taking the longer of the two loses no start.

Whatever the source, the answer is capped twice. First, by the time since that
scenario's previous run ended. Second, by the time since the training-log
write before this run's own (`TrainingWrites`). The log is written when a
counted run ends, and also when a run is restarted from inside: fast restart,
the pause menu's Restart, and Smart Restart all save it as not completed, for
runs over 3 seconds. The results screen's Restart writes nothing. A run
started after the write before it, whichever scenario that was for.

The write times have to be taken as they happen, by a separate watcher on the
`Training Data` folder. By the time a run is handled, the file's timestamp is
that run's own write. The same watcher says whether the run wrote the log at
all, which is how an uncounted run is told apart from one logged under another
name.

**Ranked runs are logged under another name.** The training entry is
`<name> - RANKED`, while the statistics file under `Ranked/` drops the suffix
(Aimbeast builds the path with `Replace(ScenarioName, " - RANKED", "")`).
`logged_name` maps a `Ranked/` file back to the suffixed name when the log has
it, played no earlier than the plain name. Some `Ranked/` scenarios are logged
plainly, `PASU 5TGS SMOOTH V2 (2EZ)` for one. The clip folder keeps the file's
name. A wrong match stays possible, which is why "no write" is asked of the
watcher rather than read from unmoved totals.

**`Total Time` cannot give you a length.** It counts restarted runs too: 49
seconds across 3 sessions of a 15 second scenario.

**Pauses are left out of every logged length**, so a clip placed at
`end - length` starts late by however long the run was paused. Nothing on disk
records pause time. This is a known gap, not handled yet.

Day keys are `d/m/yyyy`, unpadded, from the local OS date, while the yearly
file is named from the UTC year. Take the log's own newest day rather than
today's date. The run being clipped was just written, so it is that day, and
the system clock only adds ways to disagree with the file. Two readings are
only comparable within one day; the totals restart at midnight.

## Timing

Aimbeast writes the training log about a millisecond **before** the statistics
file, so the run is already counted by the time a watcher event for the
statistics file arrives.

A run's end time is the statistics file's `modified()`, via
`utils::get_modification_time`. Reading the clock instead puts the whole clip
window late by the debounce, the wait for the file to settle, and however long
the watcher was busy with the previous run.

**Do not use `utils::get_creation_or_modification_time` here.** It prefers the
creation time, which is right for KovaaK's, where each run writes a new file,
and stale by a year for Aimbeast, which rewrites one file per scenario.

## Checking a change against the real game

The training log is the ground truth for what was played; the statistics file
is only what Aimbeast chose to record. Compare the two when runs go missing.

Clip length is checkable directly:

```bash
ffprobe -v error -show_entries format=duration -of csv=p=0 "<clip>.mp4"
```

Expect `scenario length + trim_padding_start + trim_padding_end`, plus about a
second for the gap between OBS being asked to save and the buffer's timestamp.

To exercise back-to-back runs, set `only_pb` to false and raise
`trim_padding_end` so the window is wide enough to overlap — and keep
`trim_padding_end` under OBS's `RecRBTime`, or the buffer will not hold the
whole clip. Restore the user's config afterwards.
