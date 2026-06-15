use crate::cache::Cache;
use crate::config::AppConfig;
use std::sync::{Arc, OnceLock};
use tauri::AppHandle;
use tokio::sync::Mutex;

pub const STAT_DATE_TIME_FORMAT: &str = "%Y.%m.%d-%H.%M.%S";
pub const STAT_DATE_TIME_LEN: usize = 19;
pub const STAT_FILE_SUFFIX: &str = " Stats.csv";

pub const CONFIG_FILE: &str = "config.json";

pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();
pub static APP_CACHE: OnceLock<Arc<Mutex<Cache>>> = OnceLock::new();
pub static APP_CONFIG: OnceLock<Arc<AppConfig>> = OnceLock::new();
