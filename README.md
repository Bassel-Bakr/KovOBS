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

## Legacy Python implemenation
[OBS-KovaaKs-Auto-Clipper](https://github.com/Bassel-Bakr/OBS-KovaaKs-Auto-Clipper)

## Contributing

Pull requests are welcome. For major changes, please open an issue first to discuss what you would like to change.

## License

[MIT](LICENSE)
