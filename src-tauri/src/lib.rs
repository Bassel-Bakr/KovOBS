use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use std::path::PathBuf;
use std::time::Duration;
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

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
            cmds::run_obs,
            cmds::run_kovaaks,
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

        let mut obs_running = false;
        let mut kovaaks_running = false;

        loop {
            let (config_processes, is_running) = {
                let app_state = APP_STATE.wait().await.lock().await;
                (
                    app_state.config.as_ref().unwrap().processes.clone(),
                    app_state.is_running,
                )
            };

            system.refresh_processes_specifics(
                ProcessesToUpdate::All,
                true,
                ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
            );

            let mut new_obs_running = false;
            let mut new_kovaaks_running = false;

            for process in system.processes().values() {
                let Some(exe_path) = process.exe() else {
                    continue;
                };

                new_obs_running = new_obs_running || exe_path == &config_processes.paths.obs;
                new_kovaaks_running =
                    new_kovaaks_running || exe_path == &config_processes.paths.kovaaks;

                if new_obs_running && new_kovaaks_running {
                    break;
                }
            }

            if new_obs_running != obs_running {
                obs_running = new_obs_running;
            }

            if new_kovaaks_running != kovaaks_running {
                kovaaks_running = new_kovaaks_running;
            }

            _ = events::emit(AppEvent::Running(is_running));
            _ = events::emit(AppEvent::ObsRunning(obs_running));
            _ = events::emit(AppEvent::KovaaksRunning(kovaaks_running));

            tokio::time::sleep(Duration::from_secs(config_processes.scan_interval_secs)).await;
        }
    });
}
