use crate::globals::APP_HANDLE;
use tauri::Emitter;

pub fn is_running(state: bool) -> Result<(), tauri::Error> {
    Emitter::emit(APP_HANDLE.get().unwrap(), "is_running", state)
}

pub fn message(msg: &str) -> Result<(), tauri::Error> {
    Emitter::emit(APP_HANDLE.get().unwrap(), "message", msg)
}
