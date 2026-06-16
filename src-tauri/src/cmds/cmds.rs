// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::config::AppConfig;
use crate::consts::CONFIG_FILE;
use crate::globals::APP_STATE;
use crate::globals::APP_TASK_TRACKER;
use crate::kovobs;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub async fn is_ready() -> bool {
    let state = &APP_STATE.lock().await;
    state.is_ready
}

#[tauri::command]
pub async fn start_app() {
    let state = &mut APP_STATE.lock().await;

    let task_tracker = &APP_TASK_TRACKER;

    // If it's still running, do nothing
    if !task_tracker.is_empty() {
        return;
    }

    // If it's closed, reopen it
    if task_tracker.is_closed() {
        task_tracker.reopen();
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
    task_tracker.spawn_on(startup_task, handle.inner());
}

#[tauri::command]
pub async fn stop_app() -> Result<(), String> {
    let state = &mut APP_STATE.lock().await;

    if !state.is_ready {
        return Err(String::from("App state is not ready"));
    }

    let task_tracker = &APP_TASK_TRACKER;

    if let Some(cancellation_token) = state.cancellation_token.take() {
        cancellation_token.cancel();
    }

    state.clear();
    task_tracker.close();

    task_tracker.wait().await;

    Ok(())
}

#[tauri::command]
pub async fn get_config() -> Result<Arc<AppConfig>, String> {
    let state = &APP_STATE.lock().await;

    if !state.is_ready {
        return Err(String::from("App state is not ready"));
    }

    Ok(state.config.as_ref().cloned().unwrap_or_default())
}

#[tauri::command]
pub async fn save_config(config: AppConfig) -> Result<(), String> {
    let state = &APP_STATE.lock().await;

    if !state.is_ready {
        return Err(String::from("App state is not ready"));
    }

    AppConfig::save(CONFIG_FILE, config)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn clear_cache() -> Result<(), String> {
    let state = &APP_STATE.lock().await;

    if !state.is_ready {
        return Err(String::from("App state is not ready"));
    }

    let config = state.config.as_ref().unwrap();
    let cache = state.cache.as_ref().unwrap();
    cache.lock().await.clear();
    tokio::fs::remove_file(&config.cache_file)
        .await
        .map_err(|e| e.to_string())
}
