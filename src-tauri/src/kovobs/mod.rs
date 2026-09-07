//! A clipping session: connect to OBS, start the watchers, and shut down
//! cleanly when one of them stops.

use crate::cache::Cache;
use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE, AppState};
use crate::{cmds, config::AppConfig, events, notification, stat::Stat, ui_println};
use chrono::Utc;
use obws::Client;
use std::sync::Arc;
use std::{panic, path};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;
mod clips;
mod runs;

use clips::listen_to_obs_events;
use runs::{watch_aimbeast_stats_folder, watch_kovaaks_stats_folder};

pub async fn start() -> Result<(), anyhow::Error> {
    // Register panic handler
    panic::set_hook(Box::new(|info| {
        ui_println!("💥 App crashed: {}", info);
    }));

    if let Err(err) = run().await {
        ui_println!("🛑 Error: {}", err);
    }

    let app_state = &mut APP_STATE.wait().await.lock().await;
    app_state.is_running = false;
    _ = events::emit(AppEvent::Running(app_state.is_running));

    Ok(())
}

pub async fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let app_handle = APP_HANDLE.get().unwrap();

    let config = AppConfig::open(app_handle)?;
    let config_clone = config.clone();
    // Set app state
    let mut app_state = AppState::new();
    app_state.config.replace(Arc::new(config));
    app_state.is_ready = true;

    APP_STATE
        .set(Mutex::new(app_state))
        .map_err(|e| e.to_string())?;

    _ = events::emit(AppEvent::Config(config_clone.into()));

    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = {
        let app_state = &APP_STATE.wait().await.lock().await;
        app_state.config.clone().unwrap()
    };

    let cache = {
        let app_handle = APP_HANDLE.get().unwrap();
        Cache::new(app_handle, &config.cache_file).await?
    };

    ui_println!("⏺️ Connecting to OBS...");
    let client = Client::connect(
        &config.obs.host,
        config.obs.port,
        Some(&config.obs.password),
    )
    .await?;
    ui_println!("✅ Done");

    ui_println!("🔃 Making sure replay buffer is enabled...");
    if let Ok(true) = client.replay_buffer().status().await {
        ui_println!("😃 Already active!");
    } else {
        ui_println!("🫡 Activating...");
        client.replay_buffer().start().await?;
    }
    ui_println!("✅ Done");

    let mut client = Arc::new(client);
    let cache = Arc::new(Mutex::new(cache));

    // Update app state
    {
        let app_state = &mut APP_STATE.wait().await.lock().await;

        app_state.cache.replace(cache.clone());
        app_state.client.replace(client.clone());

        // We're ready to display the UI now
        app_state.is_ready = true;
        app_state.is_running = true;
    };
    events::emit(AppEvent::Running(true))?;

    let obs_sources = cmds::get_obs_sources().await?;
    events::emit(events::AppEvent::ObsSources(obs_sources.into()))?;

    let kovaaks_stats = path::PathBuf::from(&config.stats_folder);
    let aimbeast_stats = path::PathBuf::from(&config.aimbeast.stats_folder);

    if kovaaks_stats.exists() {
        tokio::spawn(rebuild_cache(config.clone(), cache.clone()));
    }

    // Last seen stat
    let (tx, rx) = mpsc::channel::<Stat>(1);

    let tx = Arc::new(tx);

    let mut tasks = JoinSet::new();
    let mut watch_tasks = JoinSet::new();

    tasks.spawn(listen_to_obs_events(config.clone(), client.clone(), rx));

    if kovaaks_stats.exists() {
        watch_tasks.spawn(watch_kovaaks_stats_folder(
            config.clone(),
            client.clone(),
            cache.clone(),
            tx.clone(),
        ));
    }

    if aimbeast_stats.exists() {
        watch_tasks.spawn(watch_aimbeast_stats_folder(
            config.clone(),
            client.clone(),
            tx.clone(),
        ));
    }

    let res: Result<(), Box<dyn std::error::Error + Send + Sync>> = tokio::select! {
        res = tokio::signal::ctrl_c() => {
            ui_println!("🔎 Ctrl+C received. Shutting down!");
            tasks.shutdown().await;
            res.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
        res = tasks.join_next() => {
            res.transpose()?.transpose().map(|_| ())
        }
        // A watcher only ever finishes by failing, and nothing was watching for
        // that: the session carried on with the tray still showing "running"
        // while no runs were being noticed at all. The guard matters -- an
        // empty JoinSet yields None immediately and would spin this select.
        Some(res) = watch_tasks.join_next(), if !watch_tasks.is_empty() => {
            let error = match res {
                Ok(Err(e)) => Some(e.to_string()),
                Err(e) => Some(e.to_string()),
                Ok(Ok(())) => None,
            };

            match error {
                Some(error) => {
                    notification::failed(
                        "stopped watching for runs",
                        &format!("No more clips will be saved until you restart.\n{error}"),
                        &config.notifications,
                    );

                    Err(error.into())
                }
                None => Ok(()),
            }
        }
    };

    // Both sets, whichever branch ended the session. Only the Ctrl+C path shut
    // `tasks` down itself, so a watcher failing used to leave the OBS listener
    // running here -- holding a clone of the client, which made the disconnect
    // below silently impossible.
    tasks.shutdown().await;
    watch_tasks.shutdown().await;

    if let Err(e) = res {
        ui_println!("❌ Error: {}", e);
    }

    ui_println!("📦 Saving cache updates...");
    cache.clone().lock().await.save(Utc::now())?;
    ui_println!("✅ Done");

    if let Some(client) = Arc::get_mut(&mut client) {
        ui_println!("🚫 Disconnecting from OBS...");
        client.disconnect().await;
        ui_println!("✅ Done");
    }

    Ok(())
}

async fn rebuild_cache(
    config: Arc<AppConfig>,
    cache: Arc<Mutex<Cache>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ui_println!("📦 Rebuilding cache from stat files...");

    let instant = Instant::now();

    let mut cache = cache.lock().await;

    let res = async {
        cache.load()?;
        cache.update(&config.stats_folder)?;
        Ok::<_, anyhow::Error>(())
    }
    .await;

    if let Err(e) = res {
        ui_println!("❌ Cache rebuild failed: {}", e);
    }

    ui_println!(
        "✅ Done rebuilding cache in {:.2}s",
        instant.elapsed().as_secs_f32()
    );

    Ok(())
}
