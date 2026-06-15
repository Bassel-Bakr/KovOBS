use crate::consts::APP_HANDLE;
use std::path::PathBuf;

mod cache;
mod cmds;
mod config;
mod consts;
mod delay;
mod events;
mod ffmpeg;
mod kovobs;
mod macros;
mod stat;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            cmds::get_config,
            cmds::clear_cache
        ])
        .setup(|app| {
            APP_HANDLE.set(app.handle().clone()).unwrap();

            if cfg!(debug_assertions) {
                std::env::set_current_dir(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
                )?;
            }

            _ = tauri::async_runtime::spawn(async {
                if let Err(e) = kovobs::start().await {
                    eprintln!("{e:?}");
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
