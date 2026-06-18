use crate::config::ProcessesConfig;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{ProcessesToUpdate, System};

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
        .plugin(tauri_plugin_autostart::Builder::new().build())
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

            observe_processes();

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn observe_processes() {
    tauri::async_runtime::spawn(async move {
        let mut system = System::new();
        let mut previous = HashSet::new();

        loop {
            let config_processes: ProcessesConfig = {
                let app_state = APP_STATE.wait().await.lock().await;
                app_state.config.as_ref().unwrap().processes.clone()
            };

            system.refresh_processes(ProcessesToUpdate::All, true);

            let running: HashSet<_> = system
                .processes()
                .values()
                .filter_map(|p| p.exe().map(|p| p.to_string_lossy().into_owned()))
                .filter(|p| {
                    *p == config_processes.paths.obs || *p == config_processes.paths.kovaaks
                })
                .collect();

            // Newly started processes
            for process in running.difference(&previous) {
                if *process == config_processes.paths.kovaaks {
                    _ = events::emit(AppEvent::KovaaksRunning(true));
                } else {
                    _ = events::emit(AppEvent::ObsRunning(true));
                }
            }

            // Newly stopped processes
            for process in previous.difference(&running) {
                if *process == config_processes.paths.kovaaks {
                    _ = events::emit(AppEvent::KovaaksRunning(false));
                } else {
                    _ = events::emit(AppEvent::ObsRunning(false));
                }
            }

            previous = running;

            tokio::time::sleep(Duration::from_secs(config_processes.scan_interval_secs)).await;
        }
    });
}
