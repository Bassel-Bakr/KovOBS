use crate::cache::Cache;
use crate::config::AppConfig;
use obws::Client;
use std::sync::{Arc, LazyLock, OnceLock};
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio_util::task::TaskTracker;

pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub static APP_STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::new()));

pub static APP_TASK_TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);

pub struct AppState {
    pub is_ready: bool,
    pub cache: Option<Arc<Mutex<Cache>>>,
    pub config: Option<Arc<AppConfig>>,
    pub client: Option<Arc<Client>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            is_ready: false,
            cache: None,
            config: None,
            client: None,
        }
    }

    pub fn clear(&mut self) {
        self.is_ready = false;
        self.cache.take();
        self.config.take();
        self.client.take();
    }
}
