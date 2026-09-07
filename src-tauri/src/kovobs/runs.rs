//! Noticing that a run has finished.
//!
//! Watches each game's stats folder and, when a new run lands, asks OBS to save
//! its replay buffer and take the screenshot. What OBS then writes is picked up
//! by [`super::clips`].

use crate::cache::Cache;
use crate::delay::StatDelay;
use crate::stat::StatType;
use crate::{config::AppConfig, consts, stat::Stat, ui_println, utils};
use anyhow::Context;
use chrono::Utc;
use encoding_rs_io::DecodeReaderBytesBuilder;
use notify::{RecommendedWatcher, Watcher};
use obws::Client;
use obws::requests::sources::SaveScreenshot;
use std::panic;
use std::{sync::Arc, time::Duration};
use tokio::fs;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinSet;
use tokio::time::Instant;
pub(super) async fn watch_kovaaks_stats_folder(
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

pub(super) async fn watch_aimbeast_stats_folder(
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
