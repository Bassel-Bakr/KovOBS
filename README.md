# KovOBS

Automatically save your best Kovaak's (and Aimbeast) runs with OBS Replay Buffer.

KovOBS watches your score files and, when a run finishes, tells OBS to save the replay buffer and trims it down to the run itself.

No more remembering to press a hotkey after a good run.

---

## Features

- 🎥 Saves the OBS Replay Buffer automatically when a run ends
- 🏆 Optional **personal bests only** mode
- ✂️ Trims each clip to the run, with adjustable padding
- 📸 Optional automatic screenshots
- 🔔 Notifications, including ones that tell you when clipping stopped
- 🎯 Kovaak's support
- 🧪 Experimental Aimbeast support
- ⚡ Runs quietly in the tray
- 🖥️ Simple graphical interface

---

## How it works

1. Start OBS. KovOBS starts the Replay Buffer itself if it isn't already running.
2. Launch KovOBS and finish the first-run setup.
3. Set your Kovaak's and Aimbeast stats folders in Game settings.
4. Connect to OBS.
5. Start playing.

Every finished run is clipped. Turn on **Personal bests only** if you want clips just for runs that beat your best.

---

## Requirements

- OBS Studio 28+ with the built-in WebSocket server
- FFmpeg — KovOBS can download a bundled copy for you, or use one already on your `PATH`
- Windows. Linux builds (`deb`, `AppImage`, and a `tar.gz` that needs no FUSE) exist, but KovOBS detects a running program by its full executable path, which Proton, symlinks and Flatpak installs all defeat — so the "running" indicators and starting OBS automatically when a game launches don't work there. Watching, clipping and the launch buttons do, if you point them at something Linux can run

---

## Installation

1. Download the latest release.
2. Extract it anywhere.
3. Run `KovOBS.exe`.
4. Work through the setup: clips folder, OBS connection, FFmpeg, and your game's executable.

KovOBS stores its own settings — you don't need to write a `config.json` by hand.

The default stats folder paths assume a standard Steam install on `C:`. If yours is elsewhere, set it in Game settings; nothing is auto-detected.

### Linux

- **AppImage**: `chmod +x KovOBS-linux-*.AppImage`, then run it. It needs FUSE's `fusermount` helper (the `fuse3` or `fuse2` package). Without it, run it with `--appimage-extract-and-run`, or use the `tar.gz`.
- **tar.gz**: `tar -xzf KovOBS-linux-*.tar.gz`, then run `AppRun` in the folder it creates. No FUSE needed.
- **deb**: for Debian, Ubuntu and their relatives; it uses the system's WebKitGTK.

The AppImage of releases up to 0.18.0 shows a blank window on Arch, CachyOS and other distros with a recent Mesa; later releases do not.

---

## OBS Setup

In OBS:

1. Open **Tools → WebSocket Server Settings**.
2. Enable the WebSocket server.
3. Set a password (recommended).
4. Enter the same password in KovOBS, along with the host and port if you changed them.

Pick the OBS source for each game in KovOBS. Screenshots need it too.

---

## Trimming

KovOBS trims each saved replay down to the run instead of keeping the whole buffer.

- **Padding** — `trim_padding_start` and `trim_padding_end` extend the clip either side of the run. End padding defaults to 5 seconds, and also delays the buffer save by that long.
- **Your own FFmpeg arguments** — global, input and output argument slots run as a second pass over the trimmed clip, so you can re-encode or change container. If your arguments fail, the trimmed clip is kept.
- **Delete after trimming** — optional, and it deletes the original replay buffer file.

Turning trimming off still passes the buffer through FFmpeg, so FFmpeg is needed either way.

---

## Screenshots

KovOBS can save a PNG of the configured OBS source alongside each clip.

---

## Experimental Aimbeast Support

Aimbeast support is available but still considered experimental.

Scenarios under **Normal**, **Ranked** and **Custom** are watched, so scenarios you built yourself are clipped too. Only folders that exist are watched, and only their top level.

Aimbeast's stats file records that a run happened, but not how long it took. KovOBS works the length out from Aimbeast's training log instead:

- Aimbeast logs every run that lasts **over 3 seconds**, however it ends: timer, kill limit, or a miss. KovOBS measures those exactly, to the second.
- A run of **3 seconds or less** is never logged. KovOBS spots the missing write and clips it as at most 3.5 seconds.
- With nothing to measure against, KovOBS takes the longer of that scenario's average and the run's own kill timing (time from first kill to last, from the stats file). With nothing at all, it falls back to one minute.
- A clip never reaches back past your previous run of that scenario, or past the last restart or finish of any run.

Ranked runs are logged as `<name> - RANKED`, and KovOBS looks them up under that name. Paused time is left out of every logged length, so a run you paused is clipped starting late by the length of the pause.

---

## Notifications

- Clip saved — clicking it opens the folder
- Failures, on by default, sent as urgent so they aren't silenced
- Optional sound, and an optional urgent style for clips
- A **Test notification** button in settings

---

## FAQ

### Does KovOBS record video?

No.

OBS does all recording. KovOBS simply tells OBS when to save the Replay Buffer.

### Does this work without OBS?

No.

OBS Studio is required.

### Do I need to edit a config file?

No. Everything you need is in the interface.

### Does closing the window stop it?

No. KovOBS hides to the tray and keeps clipping. Use **Quit** in the tray menu to stop it.

### Does it update itself?

No. The About page checks for a newer release when you ask it to.

---

## Roadmap

- Exact run length for Aimbeast runs that end early
- More game support
- Improved clip trimming
- Additional screenshot options
- More customization

---

## Contributing

Issues, feature requests, and pull requests are welcome.

---

## License

MIT
