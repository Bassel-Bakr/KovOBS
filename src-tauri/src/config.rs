use crate::consts;
use config::Config;
use std::path::Path;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub auto_start: bool,
    pub obs: ObsConfig,
    pub clips_folder: String,
    pub stats_folder: String,
    pub trim_padding_start: f32,
    pub trim_padding_end: f32,
    pub delete_after_trimming: bool,
    pub only_pb: bool,
    pub cache_version: String,
    pub cache_file: String,
    pub screenshot: ScreenshotConfig,
    #[serde(default)]
    pub ffmpeg_args: Box<[String]>,
    pub processes: ProcessesConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ObsConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub source_name: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessesConfig {
    pub scan_interval_secs: u64,
    pub paths: ProcessPaths,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProcessPaths {
    pub obs: String,
    pub kovaaks: String,
}

impl AppConfig {
    pub fn open(app_handle: &AppHandle) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(path) = Self::get_main_config_path(app_handle)
            && path.exists()
        {
            return Self::load(&path);
        } else if let path = Path::new(consts::CONFIG_FILE_NAME)
            && path.exists()
        {
            return Self::load(path);
        } else if let Ok(path) = Self::get_default_config_path(app_handle)
            && path.exists()
        {
            return Self::load(&path);
        }

        Err("Config file not found".into())
    }

    pub fn load(config_path: &Path) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let file = config::File::from(config_path);
        let settings = Config::builder().add_source(file).build()?;
        Ok(settings.try_deserialize::<AppConfig>()?)
    }

    pub async fn save(
        app_handle: &AppHandle,
        config: AppConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let main_config_path = Self::get_main_config_path(app_handle)?;
        let contents = serde_json::to_string_pretty(&config)?;
        tokio::fs::create_dir_all(main_config_path.parent().unwrap()).await?;
        tokio::fs::write(main_config_path, contents).await?;
        Ok(())
    }

    fn get_main_config_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, tauri::Error> {
        app_handle
            .path()
            .resolve(consts::CONFIG_FILE_NAME, BaseDirectory::Config)
            .map(|p| {
                p.with_file_name(consts::APP_NAME)
                    .join(consts::CONFIG_FILE_NAME)
            })
    }

    fn get_default_config_path(app_handle: &AppHandle) -> Result<std::path::PathBuf, tauri::Error> {
        app_handle
            .path()
            .resolve(consts::DEFAULT_CONFIG_FILE_NAME, BaseDirectory::Resource)
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            auto_start: false,
            obs: ObsConfig {
                host: "".into(),
                port: 0,
                password: "".into(),
                source_name: "".into(),
            },
            clips_folder: "".into(),
            stats_folder: "".into(),
            trim_padding_start: 0.0,
            trim_padding_end: 5.0,
            delete_after_trimming: false,
            only_pb: false,
            cache_version: "".into(),
            cache_file: "".into(),
            screenshot: ScreenshotConfig { enabled: true },
            ffmpeg_args: Box::new([]),
            processes: ProcessesConfig {
                scan_interval_secs: 1,
                paths: ProcessPaths {
                    obs: "".into(),
                    kovaaks: "".into(),
                },
            },
        }
    }
}
