use crate::log::APP_HANDLE;
use std::path::PathBuf;

mod cache;
mod config;
mod consts;
mod delay;
mod ffmpeg;
mod kovobs;
mod log;
mod stat;
mod utils;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Events!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet])
        .setup(|app| {
            APP_HANDLE.set(app.handle().clone()).unwrap();

            if cfg!(debug_assertions) {
                std::env::set_current_dir(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
                )?;
            }

            tauri::async_runtime::spawn(async {
                if let Err(e) = kovobs::start().await {
                    eprintln!("{e:?}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
