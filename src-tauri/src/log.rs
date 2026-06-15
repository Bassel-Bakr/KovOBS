use std::sync::OnceLock;
use tauri::{AppHandle, Emitter};

pub static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

#[macro_export]
macro_rules! ui_println {
    () => {
        $crate::log::emit("")
    };
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::log::emit(&msg)
    }};
}

pub fn emit(payload: &str) {
    Emitter::emit(APP_HANDLE.get().unwrap(), "message", payload).unwrap()
}
