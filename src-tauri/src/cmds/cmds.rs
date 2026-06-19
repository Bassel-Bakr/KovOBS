// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::config::AppConfig;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use crate::{events, kovobs, ui_println};
use std::path::Path;
use std::sync::Arc;
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

    Ok(())
}

#[tauri::command]
pub async fn clear_cache() -> Result<(), String> {
    let state = &APP_STATE.wait().await.lock().await;

    let config = state.config.as_ref().unwrap();
    let cache = state.cache.as_ref().unwrap();
    cache.lock().await.clear();
    tokio::fs::remove_file(&config.cache_file)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_obs() -> Result<(), String> {
    let state = &APP_STATE.wait().await.lock().await;
    let config = state.config.as_ref().unwrap();
    let exe = &config.processes.paths.obs;
    run_exe(exe, None).await
}

#[tauri::command]
pub async fn run_kovaaks() -> Result<(), String> {
    let state = &APP_STATE.wait().await.lock().await;
    let config = state.config.as_ref().unwrap();
    let exe = &config.processes.paths.kovaaks;
    run_exe(exe, None).await
}

async fn run_exe(exe: &str, cwd: Option<&Path>) -> Result<(), String> {
    if let Ok(true) = tokio::fs::try_exists(exe).await.map_err(|e| e.to_string()) {
        let mut cmd = Command::new(exe);

        if let Some(dir) = cwd.or(Path::new(exe).parent()) {
            cmd.current_dir(dir);
        }

        cmd.spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}
