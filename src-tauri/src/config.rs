use crate::consts;
use config::Config;
use std::default::Default;
use std::path::Path;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub auto_start: bool,
    pub obs: ObsConfig,
    pub clips_folder: String,
    pub stats_folder: String,
    pub trim: bool,
    pub trim_padding_start: f32,
    pub trim_padding_end: f32,
    pub delete_after_trimming: bool,
    pub only_pb: bool,
    pub cache_version: String,
    pub cache_file: String,
    pub screenshot: ScreenshotConfig,
    pub ffmpeg_args: Box<[String]>,
    pub processes: ProcessesConfig,
    pub aimbeast: AimbeastConfig,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ObsConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub source_name: String,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ScreenshotConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProcessesConfig {
    pub scan_interval_secs: u64,
    pub paths: ProcessPaths,
}

impl Default for ProcessesConfig {
    fn default() -> Self {
        Self {
            scan_interval_secs: 3,
            paths: ProcessPaths::default(),
        }
    }
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ProcessPaths {
    pub obs: String,
    pub kovaaks: String,
    pub aimbeast: String,
}

#[derive(Default, Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AimbeastConfig {
    pub stats_folder: String,
    pub clips_folder: String,
    pub obs_source_name: String,
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
            .resolve(consts::CONFIG_FILE_NAME, BaseDirectory::AppConfig)
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
            obs: Default::default(),
            clips_folder: "".into(),
            stats_folder: "".into(),
            trim: true,
            trim_padding_start: 0.0,
            trim_padding_end: 5.0,
            delete_after_trimming: false,
            only_pb: false,
            cache_version: "".into(),
            cache_file: "".into(),
            screenshot: Default::default(),
            ffmpeg_args: Default::default(),
            processes: ProcessesConfig {
                scan_interval_secs: 1,
                paths: Default::default(),
            },
            aimbeast: Default::default(),
        }
    }
}
