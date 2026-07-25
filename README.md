# KovOBS

Automatically save your best Kovaak's (and Aimbeast) runs with OBS Replay Buffer.

KovOBS watches your score files, detects when you achieve a new personal best, and tells OBS to save the replay buffer so you never lose your best clips.

No more remembering to press a hotkey after a good run.

---

## Features

- 🏆 Automatically detects new Personal Bests
- 🎥 Saves the OBS Replay Buffer automatically
- 📸 Optional automatic screenshots
- ✂️ Automatically trims clips
- 🎯 Kovaak's support
- 🧪 Experimental Aimbeast support
- ⚡ Runs quietly in the background
- 🖥️ Simple graphical interface
- ⚙️ No manual configuration files required

---

## How it works

1. Start OBS and enable the Replay Buffer.
2. Launch KovOBS.
3. Select your Kovaak's (or Aimbeast) stats folder if it isn't detected automatically.
4. Connect to OBS.
5. Start playing.

Whenever you beat your previous score, KovOBS will automatically save the replay buffer.

---

## Requirements

- Windows (linux is supported but I didn't test it)
- OBS Studio 28+ with the built-in WebSocket server
- Replay Buffer enabled in OBS

---

## Installation

1. Download the latest release.
2. Extract it anywhere.
3. Run `KovOBS.exe`.
4. Connect to OBS from the application.

That's it.

KovOBS now stores and manages its own settings automatically—you no longer need to create or edit a `config.json` file.

---

## OBS Setup

In OBS:

1. Open **Tools → WebSocket Server Settings**.
2. Enable the WebSocket server.
3. Set a password (recommended).
4. Enter the same password in KovOBS.

Make sure the Replay Buffer is running before starting a scenario.

---

## Trimming

KovOBS can optionally trim saved replays so the clip only contains the end of your run instead of the entire replay buffer.

This is useful if you keep a long replay buffer but only want the important part of each attempt.

---

## Screenshots

KovOBS can automatically save screenshots alongside your clips whenever a replay is saved.

---

## Experimental Aimbeast Support

Aimbeast support is available but currently considered experimental.

Some scenarios may require disabling trimming because Aimbeast does not always expose enough information to determine the exact run length.

---

## FAQ

### Does KovOBS record video?

No.

OBS does all recording. KovOBS simply tells OBS when to save the Replay Buffer.

### Does this work without OBS?

No.

OBS Studio is required.

### Do I need to edit a config file?

No.

Everything can be configured through the application's interface.

---

## Roadmap

- Better Aimbeast support
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
