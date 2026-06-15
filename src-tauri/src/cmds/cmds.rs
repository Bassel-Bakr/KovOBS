// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::config::AppConfig;
use crate::consts::{APP_CACHE, APP_CONFIG};
use std::sync::Arc;

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
