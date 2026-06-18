// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::config::AppConfig;
use crate::consts::CONFIG_FILE;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use crate::{events, kovobs};
use std::sync::Arc;
use tauri_plugin_autostart::ManagerExt;
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
            eprintln!("{e:?}");
        }
    };

    let handle = tauri::async_runtime::handle();
    state.task_tracker.spawn_on(startup_task, handle.inner());
    events::emit(AppEvent::Running(true))
}

#[tauri::command]
pub async fn stop_app() -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    if let Some(cancellation_token) = state.cancellation_token.take() {
        cancellation_token.cancel();
    }

    state.stop();
    events::emit(AppEvent::Running(false))?;
    state.task_tracker.close();

    state.task_tracker.wait().await;

    Ok(())
}

#[tauri::command]
pub async fn get_config() -> Result<Arc<AppConfig>, String> {
    let state = &APP_STATE.wait().await.lock().await;

    let config = state.config.as_ref().cloned().unwrap_or_default();

    Ok(config)
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    let auto_start = config.auto_start;

    AppConfig::save(CONFIG_FILE, config)
        .await
        .map_err(|e| e.to_string())?;

    let app_handle = APP_HANDLE.get().unwrap();
    let auto_launch = app_handle.autolaunch();
    if auto_start {
        auto_launch.enable().map_err(|e| e.to_string())?;
    } else {
        auto_launch.disable().map_err(|e| e.to_string())?;
    }

    let config = AppConfig::load(CONFIG_FILE).map_err(|e| e.to_string())?;
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
