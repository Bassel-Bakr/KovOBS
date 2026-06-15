use crate::consts::APP_HANDLE;
use tauri::Emitter;

pub fn ready() -> Result<(), tauri::Error> {
    Emitter::emit(APP_HANDLE.get().unwrap(), "ready", true)
}

pub fn message(msg: &str) -> Result<(), tauri::Error> {
    Emitter::emit(APP_HANDLE.get().unwrap(), "message", msg)
}
