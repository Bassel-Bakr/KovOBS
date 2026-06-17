use crate::globals::APP_HANDLE;
use std::path::PathBuf;

mod cache;
mod cmds;
mod config;
mod consts;
mod delay;
mod events;
mod ffmpeg;
mod globals;
mod kovobs;
mod macros;
mod stat;
mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            cmds::is_ready,
            cmds::is_running,
            cmds::init_app,
            cmds::start_app,
            cmds::stop_app,
            cmds::get_config,
            cmds::save_config,
            cmds::clear_cache,
            cmds::get_obs_sources,
        ])
        .setup(|app| {
            APP_HANDLE.set(app.handle().clone()).unwrap();

            // Make debug config reference the root folder instead of Tauri's
            if cfg!(debug_assertions) {
                std::env::set_current_dir(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
                )?;
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
