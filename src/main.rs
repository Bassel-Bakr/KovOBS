mod cache;
mod config;
mod delay;
mod stat;
mod utils;

use anyhow::Context;
use chrono::Utc;
use futures_util::StreamExt;
use notify::{RecommendedWatcher, Watcher};
use obws::requests::sources::SaveScreenshot;
use obws::{Client, events::Event::ReplayBufferSaved};
use std::panic;
use std::{sync::Arc, time::Duration};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::sync::broadcast;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc::channel;
use tokio::time;

use crate::cache::Cache;
use crate::delay::StatDelay;
use crate::utils::Utils;
use crate::{config::AppConfig, stat::Stat};

const CONFIG_FILE: &str = "config.json";
const STAT_FILE_SUFFIX: &str = "Stats.csv";

#[tokio::main]
async fn main() {
    // Register panic handler
    panic::set_hook(Box::new(|info| {
        eprintln!("💥 App crashed: {}", info);
        wait_for_enter_key();
    }));

    if let Err(err) = run().await {
        eprintln!("🛑 Error: {}", err);
        wait_for_enter_key();
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::load(CONFIG_FILE)?;

    println!("📦 Re-building cache from stat files...");
    let mut cache = cache::Cache::new(&config.cache_file);
    cache.load();
    cache.update(&config.stats_folder);
    println!("✅ Done");

    println!("⏺️ Connecting to OBS...");
    let client = Client::connect(
        &config.obs_host,
        config.obs_port,
        Some(&config.obs_password),
    )
    .await?;
    println!("✅ Done");

    println!("🔃 Making sure replay buffer is enabled...");
    if let Ok(true) = client.replay_buffer().status().await {
        println!("😃 Already active!");
    } else {
        println!("🫡 Activating...");
        client.replay_buffer().start().await?;
    }
    println!("✅ Done");

    let config = Arc::new(config);
    let cache = Arc::new(Mutex::new(cache));
    let mut client = Arc::new(client);

    // Last seen stat
    let (tx, mut rx) = broadcast::channel::<Stat>(1);

    let res: Result<(), Box<dyn std::error::Error>> = tokio::select! {
        res = tokio::signal::ctrl_c() => {
            println!("🔎 Ctrl+C received. Shutting down!");
            res.map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
        }
        res = listen_to_obs_events(config.clone(), client.clone(),  &mut rx) => res,
        res = watch_stats_folder(config.clone(), client.clone(), cache.clone(), &tx) => res,
    };

    if let Err(e) = res {
        eprintln!("❌ Error: {}", e);
    }

    println!("📦 Saving cache updates...");
    cache.clone().lock().await.save(Utc::now());
    println!("✅ Done");

    if let Some(client) = Arc::get_mut(&mut client) {
        println!("🚫 Disconnecting from OBS...");
        client.disconnect().await;
        println!("✅ Done");
    }

    wait_for_enter_key();

    Ok(())
}

async fn listen_to_obs_events(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    stat_receiver: &mut Receiver<Stat>,
) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Obtain the event stream
    let mut events = client.events()?;

    println!("🔔 Listening to OBS events");

    // 2. Listen to events as they occur
    while let Some(event) = events.next().await {
        if let ReplayBufferSaved {
            path: replay_buffer,
        } = event
        {
            let stat = match stat_receiver.try_recv() {
                Ok(stat) => stat,
                Err(_) => {
                    println!("ℹ️ Ignoring external ReplayBufferSaved event");
                    continue;
                }
            };

            let output_path = std::path::Path::new(&config.clips_folder).join(&stat.scenario);
            fs::create_dir_all(&output_path).await.with_context(|| {
                format!(
                    "Failed to create output directory '{}'",
                    output_path.display()
                )
            })?;

            let clip_path = output_path.join(format!("{}.mp4", stat));

            // Calculate duration based on the clip time and scenario end time
            let trim_start_point =
                stat.start_dt - Duration::from_secs_f32(config.trim_padding_start);
            let duration =
                Utils::get_creation_or_modification_time(&replay_buffer)? - trim_start_point;

            let args = [
                "-y",
                "-sseof",
                &format!("-{:.2}", duration.as_seconds_f32()),
                "-accurate_seek",
                "-i",
                replay_buffer.to_str().unwrap(),
                "-c",
                "copy",
                "-avoid_negative_ts",
                "make_zero",
                clip_path.to_str().unwrap(),
            ];

            Command::new("ffmpeg")
                .args(args)
                .status()
                .await
                .with_context(|| format!("Failed to execute ffmpeg {:?}", args))?;

            // Delete the replay buffer clip if we no longer need it
            if config.delete_after_trimming {
                std::fs::remove_file(&replay_buffer)?
            }
        }
    }

    Ok(())
}

async fn watch_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    cache: Arc<Mutex<Cache>>,
    stat_sender: &Sender<Stat>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, mut rx) = channel(10);

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

    println!("📁 Watching stats folder");

    loop {
        match rx.recv().await {
            Some(Ok(notify::Event {
                kind: notify::EventKind::Create(_),
                ref paths,
                ..
            })) => {
                let path = paths.first().with_context(|| "Failed to read path")?;

                let is_stat_file = path.file_name().is_some_and(|name| {
                    name.as_encoded_bytes()
                        .ends_with(STAT_FILE_SUFFIX.as_bytes())
                });

                if !is_stat_file {
                    continue;
                }

                println!("🆕 New stat file detected: {:?}", path.display());

                // Retry multiple times just in case the file wasn't fully written to disk
                for _ in 0..100 {
                    match Stat::parse(path) {
                        Ok(stat) => {
                            let (new_pb, old_high_score, new_score) = {
                                let mut cache = cache.lock().await;
                                cache.push(&stat)
                            };

                            if new_pb {
                                println!(
                                    "😃 New high score! Scenario: {}, Old: {}, New: {}",
                                    stat.scenario, old_high_score, new_score
                                );
                            } else {
                                println!(
                                    "😔 No new high score. Scenario: {}, Old: {}, New: {}",
                                    stat.scenario, old_high_score, new_score
                                );

                                if config.only_pb {
                                    break;
                                }
                            }

                            stat_sender.send(stat.clone())?;

                            // Calculated wasted time
                            let delay = Arc::new(StatDelay {
                                end_dt: stat.end_dt,
                                duration: Duration::from_secs_f32(config.trim_padding_end),
                            });

                            _ = tokio::join!(
                                // Clip
                                save_clip(client.clone(), delay.clone()),
                                // Screenshot
                                save_screenshot(
                                    client.clone(),
                                    config.clone(),
                                    delay.clone(),
                                    &stat
                                )
                            );

                            break;
                        }
                        Err(_) => time::sleep(Duration::from_millis(100)).await,
                    }
                }
            }
            Some(Err(e)) => return Err(Box::from(e)),
            None => return Ok(()),
            _ => (),
        }
    }
}

async fn save_clip(
    client: Arc<Client>,
    delay: Arc<StatDelay>,
) -> Result<(), Box<dyn std::error::Error>> {
    tokio::time::sleep(delay.get_delay_duration()).await;

    client.replay_buffer().save().await?;
    Ok(())
}

async fn save_screenshot(
    client: Arc<Client>,
    config: Arc<AppConfig>,
    delay: Arc<StatDelay>,
    stat: &Stat,
) -> Result<(), Box<dyn std::error::Error>> {
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

fn wait_for_enter_key() {
    println!("👋 Bye! Press Enter key to exit");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).unwrap();
}
