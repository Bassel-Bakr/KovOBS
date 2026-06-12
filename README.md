# KovOBS

KovOBS watches KovaaK's stat files and integrates with OBS to automatically capture screenshots and save replay clips when notable events occur.

## Features

* Automatic replay saving
* Automatic screenshots
* Replay clip trimming
* Personal-best-only mode
* Configurable through `config.json`

## Installation

### Configure OBS

1. Open OBS Studio.
2. Go to **Tools → WebSocket Server Settings**.
3. Enable the WebSocket server.
4. Use the default port (`4455`) or choose another one.
5. Optionally set a password.

### Create `config.json`

Create a `config.json` file alongside the executable.

Example:

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

## Documentation

Detailed documentation is available in the [Wiki](../../wiki).

## Legacy Python Implementation

[OBS-KovaaKs-Auto-Clipper](https://github.com/Bassel-Bakr/OBS-KovaaKs-Auto-Clipper)

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)
