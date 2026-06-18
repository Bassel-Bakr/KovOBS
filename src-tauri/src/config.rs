use config::{Config, File};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ObsConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub source_name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
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
    pub fn load(config_name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_file = File::with_name(config_name);
        let settings = Config::builder().add_source(config_file).build()?;
        Ok(settings.try_deserialize::<AppConfig>()?)
    }

    pub async fn save(
        config_name: &str,
        config: AppConfig,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let contents = serde_json::to_string_pretty(&config)?;
        tokio::fs::write(config_name, contents).await?;
        Ok(())
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
