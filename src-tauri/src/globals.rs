use crate::cache::Cache;
use crate::config::AppConfig;
use std::sync::{Arc, LazyLock, OnceLock};
use obws::Client;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
pub static APP_CACHE: OnceLock<Arc<Mutex<Cache>>> = OnceLock::new();
pub static APP_CONFIG: OnceLock<Arc<AppConfig>> = OnceLock::new();
pub static APP_OBS: OnceLock<Arc<Client>> = OnceLock::new();

pub static APP_IS_READY: LazyLock<Mutex<bool>> = LazyLock::new(|| Mutex::new(false));
pub static APP_TASK_TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);
