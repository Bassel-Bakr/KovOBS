//! Reading and writing the config, and checking the paths in it.

use crate::cache::Cache;
use crate::config::AppConfig;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use crate::{events, ui_println};
use std::sync::Arc;
use tauri_plugin_autostart::ManagerExt;

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
