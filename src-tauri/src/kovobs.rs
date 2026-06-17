use crate::cache::Cache;
use crate::delay::StatDelay;
use crate::globals::{AppState, APP_STATE};
use crate::{config::AppConfig, consts, ffmpeg, stat::Stat, ui_println, utils};
use anyhow::Context;
use chrono::Utc;
use futures_util::StreamExt;
use notify::{RecommendedWatcher, Watcher};
use obws::requests::sources::SaveScreenshot;
use obws::{events::Event::ReplayBufferSaved, Client};
use std::{panic, path};
use std::{sync::Arc, time::Duration};
use tokio::fs;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;

pub async fn start() -> Result<(), anyhow::Error> {
    // Register panic handler
    panic::set_hook(Box::new(|info| {
        eprintln!("💥 App crashed: {}", info);
    }));

    if let Err(err) = run().await {
        eprintln!("🛑 Error: {}", err);
    }

    Ok(())
}

pub async fn init() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::load(consts::CONFIG_FILE)?;

    // Set app state
    let mut app_state = AppState::new();
    app_state.config.replace(Arc::new(config));
    app_state.is_ready = true;

    APP_STATE
        .set(Mutex::new(app_state))
        .map_err(|e| e.to_string())?;

    Ok(())
}

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = {
        let app_state = &mut APP_STATE.wait().await.lock().await;
        app_state.config.clone().unwrap()
    };

    ui_println!("📦 Re-building cache from stat files...");
    let mut cache = Cache::new(&config.cache_file);
    let cache_rebuild_duration = {
        let instant = std::time::Instant::now();
        cache.load()?;
        cache.update(&config.stats_folder)?;
        instant.elapsed()
    };
    ui_println!("✅ Done in {:.2}s", cache_rebuild_duration.as_secs_f32());

    ui_println!("⏺️ Connecting to OBS...");
    let client = Client::connect(
        &config.obs_host,
        config.obs_port,
        Some(&config.obs_password),
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
    let mut cache = Arc::new(Mutex::new(cache));

    // Update app state
    {
        let app_state = &mut APP_STATE.wait().await.lock().await;

        app_state.cache.replace(cache.clone());
        app_state.client.replace(client.clone());

        // We're ready to display the UI now
        app_state.is_ready = true;
        app_state.is_running = true;
    }

    // Last seen stat
    let (tx, rx) = mpsc::channel::<Stat>(1);

    let mut tasks = JoinSet::new();

    tasks.spawn(listen_to_obs_events(config.clone(), client.clone(), rx));
    tasks.spawn(watch_stats_folder(
        config.clone(),
        client.clone(),
        cache.clone(),
        tx,
    ));

    let res: Result<(), Box<dyn std::error::Error + Send + Sync>> = tokio::select! {
        res = tokio::signal::ctrl_c() => {
            ui_println!("🔎 Ctrl+C received. Shutting down!");
            tasks.shutdown().await;
            res.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
        res = tasks.join_next() => {
            res.transpose()?.transpose().map(|_| ())
        }
    };

    if let Err(e) = res {
        eprintln!("❌ Error: {}", e);
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

async fn listen_to_obs_events(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    mut stat_receiver: mpsc::Receiver<Stat>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Obtain the event stream
    let mut events = client.events()?;

    ui_println!("🔔 Listening to OBS events");

    // 2. Listen to events as they occur
    while let Some(event) = events.next().await {
        if let ReplayBufferSaved {
            path: replay_buffer,
        } = event
        {
            let stat = match stat_receiver.try_recv() {
                Ok(stat) => stat,
                Err(_) => {
                    ui_println!("ℹ️ Ignoring external ReplayBufferSaved event");
                    continue;
                }
            };

            let output_path = path::Path::new(&config.clips_folder).join(&stat.scenario);
            let clip_path = output_path.join(format!("{}.mp4", stat));

            // Calculate duration based on the clip time and scenario end time
            let trim_start_point =
                stat.start_dt - Duration::from_secs_f32(config.trim_padding_start);
            let duration =
                utils::get_creation_or_modification_time(&replay_buffer)? - trim_start_point;

            ffmpeg::trim(&replay_buffer, &clip_path, duration, &config.ffmpeg_args).await?;

            // Delete the replay buffer clip if we no longer need it
            if config.delete_after_trimming {
                tokio::fs::remove_file(&replay_buffer)
                    .await
                    .with_context(|| {
                        format!(
                            "Failed to delete replay buffer after trimming: {}",
                            replay_buffer.display()
                        )
                    })?;
            }
        }
    }

    Ok(())
}

async fn watch_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    cache: Arc<Mutex<Cache>>,
    stat_sender: mpsc::Sender<Stat>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel(1);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            tx.blocking_send(res).expect("Failed to send file event");
        },
        notify::Config::default(),
    )
    .with_context(|| "Failed to create watcher")?;

    watcher
        .watch(
            std::path::Path::new(&config.stats_folder),
            notify::RecursiveMode::NonRecursive,
        )
        .with_context(|| "Failed to watch stats folder")?;

    ui_println!("📁 Watching stats folder");

    loop {
        match rx.recv().await {
            Some(Ok(notify::Event {
                kind: notify::EventKind::Create(_),
                ref paths,
                ..
            })) => {
                let path = paths.first().with_context(|| "Failed to read path")?;

                let Some(file_name) = path.file_name() else {
                    continue;
                };

                let is_stat_file = file_name
                    .as_encoded_bytes()
                    .ends_with(consts::STAT_FILE_SUFFIX.as_bytes());

                if !is_stat_file {
                    continue;
                }

                ui_println!("🆕 New stat file detected: {:?}", file_name);

                // Wait until it's stable
                utils::wait_for_file(path).await?;

                let Ok(stat) = Stat::parse(path) else {
                    continue;
                };

                let (new_pb, old_high_score, new_score) = {
                    let mut cache = cache.lock().await;
                    cache.push(&stat)
                };

                if new_pb {
                    ui_println!(
                        "😃 New high score! Scenario: {}, Old: {}, New: {}",
                        stat.scenario,
                        old_high_score,
                        new_score
                    );
                } else {
                    ui_println!(
                        "😔 No new high score. Scenario: {}, Old: {}, New: {}",
                        stat.scenario,
                        old_high_score,
                        new_score
                    );

                    if config.only_pb {
                        continue;
                    }
                }

                stat_sender.send(stat.clone()).await?;

                let delay = Arc::new(StatDelay {
                    end_dt: stat.end_dt,
                    duration: Duration::from_secs_f32(config.trim_padding_end),
                });

                let mut tasks = JoinSet::new();

                tasks.spawn(save_clip(client.clone(), delay.clone()));
                tasks.spawn(save_screenshot(
                    client.clone(),
                    config.clone(),
                    delay.clone(),
                    stat,
                ));

                tasks.join_all().await;
            }
            Some(Err(e)) => return Err(Box::from(e)),
            _ => (),
        };
    }
}

async fn save_clip(
    client: Arc<Client>,
    delay: Arc<StatDelay>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::time::sleep(delay.get_delay_duration()).await;
    client.replay_buffer().save().await?;
    Ok(())
}

async fn save_screenshot(
    client: Arc<Client>,
    config: Arc<AppConfig>,
    delay: Arc<StatDelay>,
    stat: Stat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !config.screenshot.enabled {
        return Ok(());
    }

    let clip_path = std::path::Path::new(&config.clips_folder).join(&stat.scenario);

    fs::create_dir_all(&clip_path)
        .await
        .with_context(|| format!("Failed to create clip directory '{}'", clip_path.display()))?;

    let clip_path = clip_path.join(format!("{}.png", stat));

    let options = SaveScreenshot {
        source: obws::requests::sources::SourceId::Name(&config.obs_source_name),
        format: "png",
        width: None,
        height: None,
        compression_quality: Some(0),
        file_path: &clip_path,
    };

    tokio::time::sleep(delay.get_delay_duration()).await;

    client
        .sources()
        .save_screenshot(options)
        .await
        .with_context(|| {
            format!(
                "Failed to save screenshot with options {}",
                clip_path.display()
            )
        })?;

    Ok(())
}
