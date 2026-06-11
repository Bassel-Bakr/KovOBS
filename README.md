# KovOBS
KovOBS watches Kovaaks stat files and integrates with OBS to automatically capture screenshots and save replay clips when notable events occur.

## Configuration

We'll need to set up credentials in OBS Studio:

1. Open OBS Studio and go to `Tools` > `WebSocket Server Settings`.
1. Enable the WebSocket server.
1. Set a server port (default is `4455`).
1. Optionally, set a password for added security.
1. Save your settings.

Before running the app, create a `config.json` file in the same folder as the executable with your OBS WebSocket password:

1. Open the `config.json` file in the project directory.
2. Locate the `"obs_password"` field.
3. Set its value to the password you configured in OBS WebSocket settings. For example:

   ```json
   {
     "obs_password": "your_password_here"
   }
   ```

4. Save the file.

Example:
```json
{
  "stats_folder": "C:\\Program Files (x86)\\Steam\\steamapps\\common\\FPSAimTrainer\\FPSAimTrainer\\stats",
  "clips_folder": "E:\\OBS\\KovOBS",
  "obs_host": "localhost",
  "obs_port": 4455,
  "obs_password": "blablabla",
  "obs_replay_folder": "E:\\OBS",
  "obs_source_name": "KovaaK's",
  "trim_padding_start": 1,
  "trim_padding_end": 5,
  "delete_after_trimming": false,
  "only_pb": false,
  "cache_version": "1.0.0",
  "cache_file": "cache.json",
  "screenshot": {
    "enabled": true
  }
}
```

## config.json

KovOBS is configured through a JSON file:

```json
{
  "stats_folder": "C:\\Program Files (x86)\\Steam\\steamapps\\common\\FPSAimTrainer\\FPSAimTrainer\\stats",
  "clips_folder": "E:\\OBS\\KovOBS",
  "obs_host": "localhost",
  "obs_port": 4455,
  "obs_password": "your_password",
  "obs_replay_folder": "E:\\OBS",
  "obs_source_name": "KovaaK's",
  "trim_padding_start": 1,
  "trim_padding_end": 5,
  "only_pb": false,
  "cache_version": "1.0.0",
  "cache_file": "cache.json",
  "screenshot": {
    "enabled": true
  }
}
```

### `stats_folder`

Path to the KovaaK's stats folder. KovOBS watches this folder for newly created stats files.

### `clips_folder`

Directory where trimmed clips and screenshots will be saved.

### `obs_host`

Hostname or IP address of the OBS WebSocket server.

Usually:

```json
"localhost"
```

### `obs_port`

Port used by OBS WebSocket.

Default:

```json
4455
```

### `obs_password`

Password configured in OBS WebSocket settings.

### `obs_replay_folder`

Folder where OBS stores replay buffer recordings.

### `obs_source_name`

Name of the OBS source used when taking screenshots.

This must exactly match the source name in OBS.

Example:

```json
"KovaaK's"
```

### `trim_padding_start`

Number of seconds to include before the event when trimming replay clips.

### `trim_padding_end`

Number of seconds to include after the event when trimming replay clips.

For example:

```json
"trim_padding_start": 1,
"trim_padding_end": 5
```

will create clips containing 1 second before and 5 seconds after the score was achieved.

### `only_pb`

Controls when clips are created.

* `true`: only save clips for personal best scores.
* `false`: save clips for every run.

### `cache_version`

Internal cache format version.

Normally, this should not be changed.

### `cache_file`

Path to the cache file used to prevent processing the same stats file multiple times.

### `screenshot.enabled`

Enables or disables automatic screenshots.

* `true`: save screenshots.
* `false`: disable screenshots.

## Legacy Python implemenation
[OBS-KovaaKs-Auto-Clipper](https://github.com/Bassel-Bakr/OBS-KovaaKs-Auto-Clipper)

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)
