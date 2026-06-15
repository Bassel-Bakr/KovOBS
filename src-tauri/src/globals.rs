use crate::cache::Cache;
use crate::config::AppConfig;
use obws::Client;
use std::sync::{Arc, LazyLock, OnceLock};
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

pub static APP_STATE: LazyLock<Mutex<AppState>> = LazyLock::new(|| Mutex::new(AppState::new()));

pub static APP_TASK_TRACKER: LazyLock<TaskTracker> = LazyLock::new(TaskTracker::new);

pub struct AppState {
    pub is_ready: bool,
    pub is_running: bool,
    pub cache: Option<Arc<Mutex<Cache>>>,
    pub config: Option<Arc<AppConfig>>,
    pub client: Option<Arc<Client>>,
    pub cancellation_token: Option<Arc<CancellationToken>>,
    pub task_tracker: TaskTracker,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            is_ready: false,
            is_running: false,
            cache: None,
            config: None,
            client: None,
            cancellation_token: None,
            task_tracker: TaskTracker::new(),
        }
    }

    pub fn start(&mut self) {
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        self.is_running = false;
    }

    pub fn clear(&mut self) {
        self.is_ready = false;
        self.is_running = false;
        self.cache.take();
        self.config.take();
        self.client.take();
    }
}
