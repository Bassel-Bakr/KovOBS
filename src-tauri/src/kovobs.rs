use crate::cache::Cache;
use crate::delay::StatDelay;
use crate::events::AppEvent;
use crate::globals::{AppState, APP_HANDLE, APP_STATE};
use crate::stat::StatType;
use crate::{cmds, config::AppConfig, consts, events, ffmpeg, stat::Stat, ui_println, utils};
use anyhow::Context;
use chrono::{TimeDelta, Utc};
use encoding_rs_io::DecodeReaderBytesBuilder;
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
use tokio::time::Instant;

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

    // Rebuild cache
    if path::PathBuf::from(&config.stats_folder).exists() {
        tokio::spawn(rebuild_cache(config.clone(), cache.clone()));
    }

    // Last seen stat
    let (tx, rx) = mpsc::channel::<Stat>(1);

    let tx = Arc::new(tx);

    let mut tasks = JoinSet::new();
    let mut watch_tasks = JoinSet::new();

    tasks.spawn(listen_to_obs_events(config.clone(), client.clone(), rx));

    watch_tasks.spawn(watch_kovaaks_stats_folder(
        config.clone(),
        client.clone(),
        cache.clone(),
        tx.clone(),
    ));

    watch_tasks.spawn(watch_aimbeast_stats_folder(
        config.clone(),
        client.clone(),
        tx.clone(),
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

            let output_path = match stat.stat_type {
                StatType::Aimbeast => {
                    path::Path::new(&config.aimbeast.clips_folder).join(&stat.scenario)
                }
                StatType::KovaaKs => path::Path::new(&config.clips_folder).join(&stat.scenario),
            };

            let clip_path = output_path.join(format!("{}.mp4", stat));

            // Calculate duration based on the clip time and scenario end time
            let trim_start_point =
                stat.start_dt - Duration::from_secs_f32(config.trim_padding_start);
            let duration =
                utils::get_creation_or_modification_time(&replay_buffer)? - trim_start_point;

            // TODO: Aimbeast trimming is experimental and fixed at 1m. Figure out how to get the scenario length to fix it
            let trim_duration = if config.trim {
                // Trim using ffmpeg
                duration
            } else {
                // Copy the buffer and don't really trim it
                // Can't think of a scenario longer than 1 day
                TimeDelta::from_std(Duration::from_hours(24))?
            };

            ffmpeg::trim(
                &replay_buffer,
                &clip_path,
                trim_duration,
                &config.ffmpeg_args,
            )
            .await?;

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

async fn watch_kovaaks_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    cache: Arc<Mutex<Cache>>,
    stat_sender: Arc<mpsc::Sender<Stat>>,
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

    ui_println!("📁 Watching KovaaK's stats");

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

                let Ok(stat) = Stat::parse_kovaaks_stat(path) else {
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

async fn watch_aimbeast_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    stat_sender: Arc<mpsc::Sender<Stat>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel(1);

    let stats_folder = std::path::Path::new(&config.aimbeast.stats_folder);

    if !stats_folder.exists() {
        let msg = String::from("Error watching aimbeast stats folder");
        return Err(Box::from(msg));
    }

    let normal_scenarios = stats_folder.join("Normal");
    let ranked_scenarios = stats_folder.join("Ranked");

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            tx.blocking_send(res).expect("Failed to send file event");
        },
        notify::Config::default(),
    )
    .with_context(|| "Failed to create watcher")?;

    watcher
        .watch(&normal_scenarios, notify::RecursiveMode::NonRecursive)
        .with_context(|| "Failed to watch stats folder")?;

    watcher
        .watch(&ranked_scenarios, notify::RecursiveMode::NonRecursive)
        .with_context(|| "Failed to watch stats folder")?;

    ui_println!("📁 Watching Aimbeast stats");

    let mut pending = None;
    let mut timer = Box::pin(tokio::time::sleep(Duration::MAX));
    let mut debounce: Option<tokio::task::JoinHandle<()>> = None;

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                pending = Some(event);
                // Restart the debounce timer
                timer.as_mut().reset(Instant::now() + Duration::from_millis(100));
            }

            _ = &mut timer, if pending.is_some() => {
                match pending.take() {
                   Some(Ok(notify::Event {
                       kind: notify::EventKind::Create(_) | notify::EventKind::Modify(_),
                       ref paths,
                       ..
                   })) => {
                       let path = paths.first().with_context(|| "Failed to read path")?;

                       if path.extension().is_none_or(|ext| ext != "json") {
                           continue;
                       }

                       if let Some(task) = debounce.take() {
                           task.abort();
                       }

                       ui_println!(
                           "🆕 New stat file detected: {:?}",
                           path.file_name().unwrap_or_default()
                       );

                       // Wait until it's stable
                       utils::wait_for_file(path).await?;

                       let f = std::fs::File::open(path)?;

                       let mut reader = DecodeReaderBytesBuilder::new()
                           .encoding(None) // Auto-detect from BOM, otherwise UTF-8
                           .build(f);

                       let mut stat =
                           serde_json::from_reader::<_, crate::aimbeast::ScenarioStatistics>(&mut reader)?;

                       stat.scenario = path
                           .file_stem()
                           .map(|stem| stem.to_string_lossy().to_string())
                           .unwrap_or_default();

                       let (new_pb, old_high_score, new_score) = {
                           // let mut cache = cache.lock().await;
                           (stat.is_pb(), stat.prev_highscore(), stat.last_score())
                       };

                       if new_pb {
                           ui_println!(
                               "😃 New high score! Scenario: {}, Old: {}, New: {}",
                               stat.scenario,
                               old_high_score.unwrap_or(&0f32),
                               new_score.unwrap_or(&0f32)
                           );
                       } else {
                           ui_println!(
                               "😔 No new high score. Scenario: {}, Old: {}, New: {}",
                               stat.scenario,
                               old_high_score.unwrap_or(&0f32),
                               new_score.unwrap_or(&0f32)
                           );

                           if config.only_pb {
                               continue;
                           }
                       }

                       let stat: Stat = stat.into();
                       stat_sender.send(stat.clone()).await?;

                       let delay = Arc::new(StatDelay {
                           end_dt: Utc::now(),
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

    let clip_path = match stat.stat_type {
        StatType::Aimbeast => {
            std::path::Path::new(&config.aimbeast.clips_folder).join(&stat.scenario)
        }
        StatType::KovaaKs => std::path::Path::new(&config.clips_folder).join(&stat.scenario),
    };

    fs::create_dir_all(&clip_path)
        .await
        .with_context(|| format!("Failed to create clip directory '{}'", clip_path.display()))?;

    let sc_path = clip_path.join(format!("{}.png", stat));

    let source = match stat.stat_type {
        StatType::Aimbeast => {
            obws::requests::sources::SourceId::Name(&config.aimbeast.obs_source_name)
        }
        StatType::KovaaKs => obws::requests::sources::SourceId::Name(&config.obs.source_name),
    };

    let options = SaveScreenshot {
        source,
        format: "png",
        width: None,
        height: None,
        compression_quality: Some(0),
        file_path: &sc_path,
    };

    tokio::time::sleep(delay.get_delay_duration()).await;

    client
        .sources()
        .save_screenshot(options)
        .await
        .with_context(|| {
            format!(
                "Failed to save screenshot with options {}",
                sc_path.display()
            )
        })?;

    ui_println!("🗃️ Saved screenshot: {}", sc_path.to_string_lossy());

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
