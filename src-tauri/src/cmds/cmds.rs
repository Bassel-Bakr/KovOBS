// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::config::AppConfig;
use crate::globals::{APP_CACHE, APP_CONFIG};
use crate::globals::{APP_IS_READY, APP_TASK_TRACKER};
use crate::kovobs;
use std::sync::Arc;

#[tauri::command]
pub async fn is_ready() -> bool {
    *APP_IS_READY.lock().await
}

#[tauri::command]
pub fn start_app() {
    let task_tracker = &APP_TASK_TRACKER;

    // If it's still running, do nothing
    if !task_tracker.is_empty() {
        return;
    }

    // If it's closed, reopen it
    if task_tracker.is_closed() {
        task_tracker.reopen();
    }

    let startup_task = async {
        if let Err(e) = kovobs::start().await {
            eprintln!("{e:?}");
        }
    };

    let handle = tauri::async_runtime::handle();
    task_tracker.spawn_on(startup_task, handle.inner());
}

#[tauri::command]
pub async fn stop_app() {
    APP_IS_READY.lock().await.clone_from(&false);

    let task_tracker = &APP_TASK_TRACKER;

    task_tracker.close();
    task_tracker.wait().await;
}

#[tauri::command]
pub fn get_config() -> Arc<AppConfig> {
    APP_CONFIG.get().cloned().unwrap_or_default()
}

#[tauri::command]
pub async fn clear_cache() -> Result<(), String> {
    let config = APP_CONFIG.get().unwrap();
    let cache = APP_CACHE.get().unwrap();
    cache.lock().await.clear();
    tokio::fs::remove_file(&config.cache_file)
        .await
        .map_err(|e| e.to_string())
}
