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

The watcher only watches `Normal` and `Ranked`.

### Deriving how long a run lasted

The statistics file carries no duration, so the length comes from the sibling
training log, whose entries look like:

```json
{"19/9/2026": {"1 WALL 6 TARGETS": {"Completed Sessions Time": 120, "Completed Sessions": 2, "Total Time": 129}}}
```

**Do not read a scenario's length as a constant.** Scenarios that end early —
on a miss, on running out of lives — last a different time every run.
`SPHERE FRENZY BAZ 10S 1SHOT` is nominally 10 seconds and recorded 93 seconds
across 14 completed runs: an average of 6.64 that describes none of them.

The totals only move when a run completes, and by exactly that run's duration,
so measure rather than assume:

```
length = ΔCompleted Sessions Time / ΔCompleted Sessions
```

between the reading taken for the previous run and this one. `ScenarioLengths`
keeps those readings, in memory and per scenario per day.

The day's own average, `Completed Sessions Time / Completed Sessions`, is only
the fallback — the first run of a scenario since launch, or since the day
rolled over, has nothing to measure against. It is right for fixed-length
scenarios and wrong for the rest, which is the reason it is not the answer.

**`Total Time` cannot give you a length.** It counts abandoned runs too: 49
seconds across 3 sessions of a 15 second scenario.

Day keys are `d/m/yyyy`, unpadded. Take the log's own newest day rather than
today's date — the run being clipped was just written, so it is that day, and
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
