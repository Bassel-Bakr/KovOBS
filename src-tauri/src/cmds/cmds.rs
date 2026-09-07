// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::cache::Cache;
use crate::config::AppConfig;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use crate::shell::ShellExt;
use crate::{events, kovobs, notification, ui_println};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
use tauri_plugin_autostart::ManagerExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub async fn is_ready() -> bool {
    let state = &APP_STATE.wait().await.lock().await;
    state.is_ready
}

#[tauri::command]
pub async fn is_running() -> bool {
    let state = &APP_STATE.wait().await.lock().await;
    state.is_running
}

#[tauri::command]
pub async fn init_app() -> Result<(), String> {
    kovobs::init().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_app() -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    // If it's still running, do nothing
    if state.is_running {
        return Err(String::from("Already running"));
    }
    // TODO: refactor this part
    // Set true here to avoid 2+ threads running 2+ instances of the app
    // because the other place for setting is_running runs after releasing the lock
    state.is_running = true;

    // If it's closed, reopen it
    if state.task_tracker.is_closed() {
        state.task_tracker.reopen();
    }

    let cancellation_token = Arc::new(CancellationToken::new());
    state.cancellation_token.replace(cancellation_token.clone());

    let startup_task = async move {
        let res = tokio::select! {
            res = cancellation_token.cancelled() => Ok(res),
            res = kovobs::start() => res,
        };

        if let Err(e) = res {
            ui_println!("{e:?}");
        }
    };

    let handle = tauri::async_runtime::handle();
    state.task_tracker.spawn_on(startup_task, handle.inner());

    Ok(())
}

#[tauri::command]
pub async fn stop_app() -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    if let Some(cancellation_token) = state.cancellation_token.take() {
        cancellation_token.cancel();
    }

    state.stop();
    state.task_tracker.close();
    state.task_tracker.wait().await;
    events::emit(AppEvent::Running(false))?;
    ui_println!("⛔ Stopped");

    Ok(())
}

/// Exits for real, rather than hiding to the tray the way closing the window
/// does.
///
/// Stops first: `kovobs::run` saves the cache on its way out, and exiting
/// straight away skips that and loses whatever personal bests this session
/// recorded. The tray's Quit goes through here too, so both routes out behave
/// the same.
#[tauri::command]
pub async fn quit_app() {
    _ = stop_app().await;

    if let Some(app_handle) = APP_HANDLE.get() {
        app_handle.exit(0);
    }
}

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let state = &APP_STATE.wait().await.lock().await;
    let config = state.config.as_ref().cloned().unwrap_or_default();
    let mut config = (*config).clone();

    let app_handle = APP_HANDLE.get().unwrap();
    let auto_launch = app_handle.autolaunch();

    if let Ok(status) = auto_launch.is_enabled() {
        config.auto_start = status;
    }

    Ok(config)
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    let auto_start = config.auto_start;

    let app_handle = APP_HANDLE.get().unwrap();
    AppConfig::save(app_handle, config)
        .await
        .map_err(|e| e.to_string())?;

    let auto_launch = app_handle.autolaunch();

    if let Ok(status) = auto_launch.is_enabled() {
        if auto_start && !status {
            match auto_launch.enable() {
                Ok(()) => ui_println!("🫡 Enabled auto start"),
                Err(e) => ui_println!("👎 Failed to enable autostart: {e:?}"),
            }
        } else if !auto_start && status {
            match auto_launch.disable() {
                Ok(()) => ui_println!("🚫 Disabled auto start"),
                Err(e) => ui_println!("👎 Failed to disable autostart: {e:?}"),
            }
        }
    };

    let config = AppConfig::open(app_handle).map_err(|e| e.to_string())?;

    _ = events::emit(AppEvent::Config(config.clone().into()));

    state.config.replace(Arc::new(config));

    ui_println!("🫡 Config saved!");

    Ok(())
}

#[tauri::command]
pub async fn clear_cache() -> Result<(), String> {
    let state = &APP_STATE.wait().await.lock().await;

    if let Some(cache) = state.cache.as_ref() {
        cache.lock().await.clear();
    }

    if let Some(config) = state.config.as_ref() {
        let app_handle = APP_HANDLE.get().unwrap();

        let cache_path =
            Cache::get_cache_path(app_handle, &config.cache_file).map_err(|e| e.to_string())?;

        tokio::fs::remove_file(cache_path)
            .await
            .map_err(|e| e.to_string())?;
    }

    ui_println!("🗑️ Cache cleared");

    Ok(())
}

// Clone the config out of the state so the lock isn't held while we scan
// processes and spawn, which would stall start/stop and the process observer.
async fn config() -> Arc<AppConfig> {
    let state = &APP_STATE.wait().await.lock().await;
    state.config.as_ref().unwrap().clone()
}

#[tauri::command]
pub async fn about_info() -> crate::update::AboutInfo {
    crate::update::about()
}

#[tauri::command]
pub async fn check_for_update() -> Result<crate::update::UpdateInfo, String> {
    crate::update::check().await
}

/// Reports which of `paths` currently exist, in the same order. Used by the UI
/// to flag a misconfigured folder or executable next to the field itself.
#[tauri::command]
pub async fn paths_exist(paths: Vec<String>) -> Vec<bool> {
    let mut results = Vec::with_capacity(paths.len());

    for path in paths {
        results.push(!path.is_empty() && tokio::fs::try_exists(&path).await.unwrap_or(false));
    }

    results
}

#[tauri::command]
pub async fn run_obs() -> Result<(), String> {
    let config = config().await;
    let exe = &config.processes.paths.obs;
    run_exe(exe, Some(Box::from(["--minimize-to-tray".into()])), None).await
}

#[tauri::command]
pub async fn run_kovaaks() -> Result<(), String> {
    let config = config().await;
    let exe = &config.processes.paths.kovaaks;
    run_exe(exe, None, None).await
}

#[tauri::command]
pub async fn run_aimbeast() -> Result<(), String> {
    let config = config().await;
    let exe = &config.processes.paths.aimbeast;
    run_exe(exe, None, None).await
}

async fn run_exe(exe: &str, args: Option<Box<[String]>>, cwd: Option<&Path>) -> Result<(), String> {
    let exe_path = Path::new(exe);

    // Deliberately not logged: the auto start handler retries this several times a
    // second, so a misconfigured path would flood the log panel forever.
    if !tokio::fs::try_exists(exe).await.unwrap_or(false) {
        return Err(format!("Can't find {exe}"));
    }

    // Scanning every process is blocking work, so keep it off the async runtime.
    let target = exe_path.to_path_buf();
    let is_running = tokio::task::spawn_blocking(move || is_process_running(&target))
        .await
        .map_err(|e| e.to_string())?;

    // Already up, so there is nothing to do. Not an error, and not worth logging
    // for the same reason as above.
    if is_running {
        return Ok(());
    }

    let mut cmd = Command::new(exe);

    if let Some(dir) = cwd.or(exe_path.parent()) {
        cmd.current_dir(dir);
    }

    if let Some(args) = args {
        cmd.args(args);
    }

    cmd.no_window().spawn().map_err(|e| e.to_string())?;
    ui_println!("🚀 Started {exe}");

    Ok(())
}

// Matches on the full executable path, the same way `observe_processes` decides
// whether OBS/KovaaK's/Aimbeast are running. Matching on the file name instead
// would let an unrelated process with the same name suppress the launch while
// the UI still reports it as not running.
//
// Windows-shaped by design. On Linux `process.exe()` is `realpath("/proc/<pid>/exe")`,
// so it won't match a configured path that is a symlink or wrapper script, and it can
// never match a Flatpak/Snap/AppImage install whose real exe lives in another mount
// namespace. Accepted: KovaaK's and Aimbeast are Windows-only and run under Proton
// there anyway, so the Linux bundle can't detect them regardless.
fn is_process_running(exe: &Path) -> bool {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
    );

    system
        .processes()
        .values()
        .any(|process| process.exe() == Some(exe))
}

/// Fires the same notification a saved clip would, so the whole chain -- the
/// toast appearing, the click reaching us, the folder opening -- can be checked
/// on demand instead of only after a run that has already been lost.
#[tauri::command]
pub async fn test_notification() -> Result<(), String> {
    let config = {
        let state = &APP_STATE.wait().await.lock().await;
        state.config.as_ref().cloned().unwrap_or_default()
    };

    let folder = [&config.clips_folder, &config.aimbeast.clips_folder]
        .into_iter()
        .map(Path::new)
        .find(|folder| folder.is_dir())
        .ok_or("Set a clips folder first, otherwise there is nothing to open.")?;

    // Point at a real clip when there is one. Revealing a folder selects it in
    // its parent, so a test against the folder itself would open the level
    // above the one a real notification opens.
    let target = newest_clip(folder).unwrap_or_else(|| folder.to_path_buf());

    notification::clip_saved(
        "KovOBS test",
        "If clicking this opens your clips folder, notifications are working.",
        &target,
        &config.notifications,
    );

    Ok(())
}

/// The most recently written clip, or `None` if nothing has been saved yet.
///
/// Looks one level down as well as in the folder itself, because clips are
/// filed under a directory per scenario -- the top level holds those
/// directories, not clips. Extensions are not filtered: what ends up there is
/// whatever the user's own FFmpeg args produced.
fn newest_clip(folder: &Path) -> Option<PathBuf> {
    fn files_in(folder: &Path) -> impl Iterator<Item = PathBuf> {
        std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
    }

    files_in(folder)
        .flat_map(|path| {
            if path.is_dir() {
                files_in(&path).collect::<Vec<_>>()
            } else {
                vec![path]
            }
        })
        .filter(|path| path.is_file())
        .max_by_key(|path| path.metadata().and_then(|meta| meta.modified()).ok())
}

#[cfg(test)]
mod tests {
    use super::newest_clip;
    use std::path::{Path, PathBuf};

    /// Builds `<root>/<scenario>/<file>` for each entry, oldest first, touching
    /// them in order so modification time follows the argument order.
    fn tree(name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("kovobs-clips-{name}"));
        _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        for (folder, file) in entries {
            let dir = if folder.is_empty() {
                root.clone()
            } else {
                root.join(folder)
            };

            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(file), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        root
    }

    /// The layout that matters: clips live under a folder per scenario, so a
    /// top-level scan finds only directories and would report nothing at all.
    #[test]
    fn finds_a_clip_one_level_down() {
        let root = tree("nested", &[("Bounce", "old.mp4"), ("Reactive", "new.mp4")]);

        assert_eq!(newest_clip(&root), Some(root.join("Reactive").join("new.mp4")));
    }

    /// The screenshot feature writes .png beside the clips, and the user's own
    /// FFmpeg args decide the container, so extension is not a filter.
    #[test]
    fn any_extension_counts() {
        let root = tree("mixed", &[("Bounce", "clip.mp4"), ("Bounce", "shot.png")]);

        assert_eq!(newest_clip(&root), Some(root.join("Bounce").join("shot.png")));
    }

    #[test]
    fn finds_a_clip_at_the_top_level_too() {
        let root = tree("flat", &[("", "loose.mp4")]);

        assert_eq!(newest_clip(&root), Some(root.join("loose.mp4")));
    }

    /// A folder holding only scenario directories has nothing to reveal, and
    /// the caller falls back to the folder itself.
    #[test]
    fn an_empty_tree_finds_nothing() {
        let root = tree("empty", &[]);
        std::fs::create_dir_all(root.join("Bounce")).unwrap();

        assert_eq!(newest_clip(&root), None);
    }

    #[test]
    fn a_missing_folder_finds_nothing() {
        assert_eq!(newest_clip(Path::new("Z:/nope/nothing/here")), None);
    }
}
