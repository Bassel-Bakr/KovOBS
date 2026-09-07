// Preview-only stand-in for the Tauri runtime, so the built frontend can run in
// an ordinary browser. Never bundled into the app.
//
// Every Tauri command the UI calls needs an entry in R below. A missing one logs
// "[shim] unhandled <cmd>" to the console and resolves null, which usually shows
// up as an empty or stuck panel rather than an error.
(function () {
  const listeners = new Map();
  const callbacks = new Map();

  const CONFIG = {
    auto_start: true, setup_completed: !location.search.includes('setup'), theme: 'system',
    stats_folder: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\FPSAimTrainer\\stats",
    clips_folder: 'E:\\OBS\\KovOBS',
    obs: { host: 'localhost', port: 4455, password: 'hunter2', source_name: "KovaaK's" },
    aimbeast: { stats_folder: 'C:\\Aimbeast\\Statistics', clips_folder: 'E:\\OBS\\AimbeastOBS', obs_source_name: 'Aimbeast' },
    trim: true, trim_padding_start: 0, trim_padding_end: 5,
    delete_after_trimming: false, only_pb: false,
    cache_version: '1.0.0', cache_file: 'cache.json',
    screenshot: { enabled: true },
    notifications: { enabled: true, urgent_clips: false, failures: true, sound: true },
    ffmpeg: { global_args: [], input_args: [], output_args: ['-c:v libx264'] },
    processes: { scan_interval_secs: 3, paths: {
      obs: 'C:\\Program Files\\obs-studio\\bin\\64bit\\obs64.exe',
      kovaaks: 'C:\\Steam\\FPSAimTrainer-Win64-Shipping.exe',
      aimbeast: 'C:\\Games\\Aimbeast\\Aimbeast-Win64-Shipping.exe' } },
  };

  const R = {
    get_config: () => CONFIG, save_config: () => null, init_app: () => null,
    start_app: () => null, stop_app: () => null, clear_cache: () => null,
    is_ready: () => true, is_running: () => true,
    get_obs_sources: () => ["KovaaK's", 'Aimbeast', 'Desktop'],
    is_ffmpeg_downloaded: () => true, download_ffmpeg: () => null, remove_ffmpeg: () => null,
    about_info: () => ({ version: '0.12.0', releases_url: '#' }),
    check_for_update: () => ({ current: '0.12.0', latest: '0.13.0', release_url: '#', update_available: true }),
    paths_exist: (a) => (a.paths || []).map((p) => location.search.includes('setup') ? false : true),
    test_notification: () => null,
    quit_app: () => { window.__QUIT_CALLED__ = true; return null; },
    run_obs: () => null, run_kovaaks: () => null, run_aimbeast: () => null,
  };

  const emit = (event, payload) => {
    const cb = callbacks.get(listeners.get(event));
    if (cb) cb({ event, id: 1, payload });
  };

  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: 'main' }, currentWebview: { windowLabel: 'main', label: 'main' } },
    transformCallback(cb) { const id = Math.floor(Math.random() * 1e9); callbacks.set(id, cb); return id; },
    invoke(cmd, args = {}) {
      if (cmd === 'plugin:event|listen') { listeners.set(args.event, args.handler); return Promise.resolve(1); }
      if (cmd === 'plugin:window|title') return Promise.resolve('KovOBS');
      if (cmd.startsWith('plugin:')) return Promise.resolve(null);
      const h = R[cmd];
      if (!h) { console.warn('[shim] unhandled', cmd); return Promise.resolve(null); }
      try { return Promise.resolve(h(args)); } catch (e) { return Promise.reject(e); }
    },
  };

  const LOGS = [
    '🔔 Listening to OBS events',
    "📁 Watching KovaaK's stats",
    '✂️ Trimming in progress (100.00%)',
    '🗃️ Saved clip: E:\\OBS\\KovOBS\\1w4ts reload - 847 - 2026.09.05.mp4',
    "👎 Can't find C:\\Games\\Aimbeast\\Aimbeast-Win64-Shipping.exe",
  ];

  window.addEventListener('DOMContentLoaded', () => setTimeout(() => {
    emit('config', CONFIG);
    emit('running', true);
    emit('obs_running', true);
    emit('kovaaks_running', false);
    emit('aimbeast_running', false);
    emit('obs_sources', ["KovaaK's", 'Aimbeast', 'Desktop']);
    emit('ffmpeg_download_progress', { state: 'Done', progress: 100 });
    LOGS.forEach((m, i) => setTimeout(() => emit('message', m), 60 * i));
  }, 120));
})();
