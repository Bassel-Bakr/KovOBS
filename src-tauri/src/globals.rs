use crate::cache::Cache;
use crate::config::AppConfig;
use crate::ffmpeg::FFmpegDownloadProgress;
use obws::Client;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio::sync::SetOnce;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

pub static APP_HANDLE: SetOnce<AppHandle> = SetOnce::const_new();
pub static APP_STATE: SetOnce<Mutex<AppState>> = SetOnce::const_new();
pub static FFMPEG_DOWNLOAD_PROGRESS: LazyLock<Mutex<FFmpegDownloadProgress>> =
    LazyLock::new(|| Mutex::new(FFmpegDownloadProgress::default()));

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

    pub fn stop(&mut self) {
        self.is_running = false;
        self.cache.take();
        self.client.take();
    }
}
