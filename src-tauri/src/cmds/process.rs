//! Launching OBS and the games, and noticing when they are already up.

use crate::config::AppConfig;
use crate::globals::APP_STATE;
use crate::shell::ShellExt;
use crate::ui_println;
use std::path::Path;
use std::sync::Arc;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
use tokio::process::Command;

// Clone the config out of the state so the lock isn't held while we scan
// processes and spawn, which would stall start/stop and the process observer.
async fn config() -> Arc<AppConfig> {
    let state = &APP_STATE.wait().await.lock().await;
    state.config.as_ref().unwrap().clone()
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
