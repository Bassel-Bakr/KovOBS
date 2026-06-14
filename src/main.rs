mod cache;
mod config;
mod consts;
mod delay;
mod stat;
mod utils;

use anyhow::Context;
use chrono::Utc;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use notify::{RecommendedWatcher, Watcher};
use obws::requests::sources::SaveScreenshot;
use obws::{events::Event::ReplayBufferSaved, Client};
use std::panic;
use std::process::Stdio;
use std::{sync::Arc, time::Duration};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use tokio::task::JoinSet;
use tokio::time;

use crate::cache::Cache;
use crate::delay::StatDelay;
use crate::{config::AppConfig, stat::Stat};

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

async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = AppConfig::load(consts::CONFIG_FILE)?;

    println!("📦 Re-building cache from stat files...");
    let mut cache = Cache::new(&config.cache_file);
    let cache_rebuild_duration = {
        let instant = std::time::Instant::now();
        cache.load()?;
        cache.update(&config.stats_folder);
        instant.elapsed()
    };
    println!("✅ Done in {:.2}s", cache_rebuild_duration.as_secs_f32());

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
            println!("🔎 Ctrl+C received. Shutting down!");
            tasks.shutdown().await;
            res.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
        res = tasks.join_next() => {
           res.unwrap().map_err(|e|Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?
        }
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
    mut stat_receiver: mpsc::Receiver<Stat>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                utils::get_creation_or_modification_time(&replay_buffer)? - trim_start_point;

            let mut args = vec![
                "-hide_banner".into(),
                "-loglevel".into(),
                "error".into(),
                "-nostats".into(),
                "-progress".into(),
                "pipe:1".into(),
                "-y".into(),
                "-sseof".into(),
                format!("-{:.2}", duration.as_seconds_f32()),
                "-accurate_seek".into(),
                "-i".into(),
                replay_buffer.to_string_lossy().into_owned(),
            ];

            args.extend(config.ffmpeg_args.iter().cloned());

            args.push(clip_path.to_string_lossy().into_owned());

            let pb = ProgressBar::new(duration.num_microseconds().unwrap().try_into()?);

            pb.set_style(ProgressStyle::with_template(
                "[{elapsed_precise}] {bar:40.cyan/blue} {percent}% ({eta})",
            )?);

            let mut process = Command::new("ffmpeg")
                .args(&args)
                .stdout(Stdio::piped())
                .spawn()
                .with_context(|| format!("Failed to execute ffmpeg {:?}", args))?;

            let reader = BufReader::new(process.stdout.take().unwrap());

            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await? {
                if let Some(ms) = line.strip_prefix("out_time_ms=") {
                    let current_ms: u64 = ms.parse()?;

                    // Clamp to avoid going over 100%
                    pb.set_position(current_ms.min(pb.duration().as_micros().try_into()?));
                }
            }

            let status = process.wait().await?;

            if status.success() {
                pb.finish_with_message("Done");
            } else {
                pb.abandon_with_message("Failed");
            }

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

    println!("📁 Watching stats folder");

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

                println!("🆕 New stat file detected: {:?}", file_name);

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

                            stat_sender.send(stat.clone()).await?;

                            // Calculated wasted time
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

fn wait_for_enter_key() {
    println!("👋 Bye! Press Enter key to exit");
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).unwrap();
}
