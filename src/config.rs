use config::{Config, File};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    pub obs_host: String,
    pub obs_port: u16,
    pub obs_password: String,
    pub obs_replay_folder: String,
    pub obs_source_name: String,
    pub clips_folder: String,
    pub stats_folder: String,
    pub trim_padding_start: f32,
    pub trim_padding_end: f32,
    pub delete_after_trimming: bool,
    pub only_pb: bool,
    pub cache_version: String,
    pub cache_file: String,
    pub screenshot: ScreenshotConfig,
    pub ffmpeg_args: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ScreenshotConfig {
    pub enabled: bool,
}

impl AppConfig {
    pub fn load(config_name: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_file = File::with_name(config_name);
        let settings = Config::builder().add_source(config_file).build()?;
        Ok(settings.try_deserialize::<AppConfig>()?)
    }
}
