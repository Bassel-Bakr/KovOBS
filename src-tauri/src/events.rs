use crate::globals::APP_HANDLE;
use tauri::Emitter;

pub enum AppEvent {
    Running(bool),
    Message(String),
    KovaaksRunning(bool),
    ObsRunning(bool),
    ObsSources(Box<[String]>),
}

pub fn emit(event: AppEvent) -> Result<(), String> {
    let app_handle = APP_HANDLE.get().unwrap();

    let res = match event {
        AppEvent::Running(is_running) => Emitter::emit(app_handle, "running", is_running),
        AppEvent::Message(msg) => Emitter::emit(app_handle, "message", msg),
        AppEvent::KovaaksRunning(is_running) => Emitter::emit(app_handle, "kovaaks_running", is_running),
        AppEvent::ObsRunning(is_running) => Emitter::emit(app_handle, "obs_running", is_running),
        AppEvent::ObsSources(sources) => Emitter::emit(app_handle, "obs_sources", sources),
    };

    res.map_err(|e| e.to_string())
}
