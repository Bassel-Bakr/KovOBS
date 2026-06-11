mod cache;
mod config;
mod stat;

use chrono::Utc;
use obws::requests::sources::SaveScreenshot;
use std::{sync::Arc, time::Duration};
use tokio::fs;
use tokio::process::Command;
use tokio::sync::{Mutex, mpsc::channel};

// Bring the type into scope for cleaner usage
use crate::{config::AppConfig, stat::Stat};

use futures_util::StreamExt;
use notify::{RecommendedWatcher, Watcher};
use obws::{Client, events::Event::ReplayBufferSaved};

#[tokio::main]
async fn main() {
    let config = AppConfig::load("config.json").expect("Failed to load configuration");

    println!("Re-building cache from stats files...");
    let mut cache = cache::Cache::new(&config.cache_file);
    cache.load();
    cache.update(&config.stats_folder);

    let client = Client::connect(
        &config.obs_host,
        config.obs_port,
        Some(&config.obs_password),
    )
    .await
    .expect("Failed to connect to OBS");

    println!("Successfully connected to OBS!");

    // Enable replay buffer
    if let Ok(true) = client.replay_buffer().status().await {
        println!("Replay buffer is already active.");
    } else {
        println!("Replay buffer is not active, starting it...");
        client
            .replay_buffer()
            .start()
            .await
            .expect("Failed to start replay buffer");
        println!("Replay buffer active.");
    }

    let config = Arc::new(config);
    let cache = Arc::new(Mutex::new(cache));
    let client = Arc::new(Mutex::new(client));
    let last_stat = Arc::new(Mutex::new(None::<Stat>));

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("Ctrl+C received, shutting down");
            println!("Saving cache updates...");
            cache.clone().lock().await.save(chrono::Utc::now());
            println!("Disconnecting from OBS...");
            client.lock().await.disconnect().await;
            println!("Bye!");
        }
        _ = listen_to_obs_events(config.clone(), client.clone(), last_stat.clone()) => {
            println!("OBS task completed");
        }
        _ = watch_stats_folder(config.clone(), client.clone(), cache.clone(), last_stat.clone()) => {
            println!("Stats folder watch task completed");
        }
    }
}

async fn listen_to_obs_events(
    config: Arc<AppConfig>,
    client: Arc<Mutex<Client>>,
    last_stat: Arc<Mutex<Option<Stat>>>,
) {
    // 2. Obtain the event stream
    let mut events = { client.lock().await.events().expect("Failed to get events") };

    // 3. Listen to events as they occur
    while let Some(event) = events.next().await {
        match event {
            ReplayBufferSaved { path } => {
                let stat = { last_stat.lock().await.clone() }.unwrap();

                let output_path = std::path::Path::new(&config.clips_folder).join(&stat.scenario);
                fs::create_dir_all(&output_path)
                    .await
                    .expect("Failed to create output directory");

                let clip_path = output_path.join(format!("{}.mp4", stat.to_string()));

                let duration_seconds = (config.trim_padding_start + config.trim_padding_end)
                    + (stat.end_dt - stat.start_dt).as_seconds_f32();

                Command::new("ffmpeg")
                    .args([
                        "-y",
                        "-sseof",
                        &format!("-{:.2}", duration_seconds),
                        "-accurate_seek",
                        "-i",
                        path.to_str().unwrap(),
                        "-c",
                        "copy",
                        "-avoid_negative_ts",
                        "make_zero",
                        clip_path.to_str().unwrap(),
                    ])
                    .status()
                    .await
                    .expect("Failed to execute ffmpeg command");
            }
            _ => (),
        }
    }
}

async fn watch_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Mutex<Client>>,
    cache: Arc<Mutex<cache::Cache>>,
    last_stat: Arc<Mutex<Option<Stat>>>,
) {
    let (tx, mut rx) = channel(10);

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            tx.blocking_send(res).expect("Failed to send file event");
        },
        notify::Config::default(),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(
            std::path::Path::new(&config.stats_folder),
            notify::RecursiveMode::NonRecursive,
        )
        .expect("Failed to watch stats folder");

    loop {
        match rx.recv().await {
            Some(Ok(notify::Event {
                kind: notify::EventKind::Create(_),
                ref paths,
                ..
            })) => {
                let path = paths.first().expect("No path in event");
                if let Some("csv") = path.extension().and_then(|s| s.to_str()) {
                    println!("New stats file detected: {:?}", path);
                    for _ in 0..100 {
                        match Stat::parse(path) {
                            Ok(stat) => {
                                let mut last_stat_lock = last_stat.lock().await;
                                *last_stat_lock = Some(stat.clone());

                                let (new_pb, old_high_score, new_score) = {
                                    let mut cache = cache.lock().await;
                                    cache.push(&stat)
                                };

                                if new_pb {
                                    println!(
                                        "New high score! Scenario: {}, Old: {}, New: {}",
                                        stat.scenario, old_high_score, new_score
                                    );
                                } else {
                                    println!(
                                        "No new high score. Scenario: {}, High Score: {}, New Score: {}",
                                        stat.scenario, old_high_score, new_score
                                    );

                                    if config.only_pb {
                                        break;
                                    }
                                }

                                // Calculated wasted time
                                let wasted_time = Utc::now() - stat.end_dt;
                                let sleep_duration = Duration::from_secs_f32(
                                    (config.trim_padding_end as f32
                                        - wasted_time.num_seconds() as f32)
                                        .max(0.0),
                                );

                                // Wait a bit before clipping
                                tokio::time::sleep(sleep_duration).await;

                                _ = tokio::join!(
                                    // Clip
                                    save_clip(client.clone()),
                                    // Screenshot
                                    save_screenshot(client.clone(), config.clone(), &stat)
                                );

                                break;
                            }
                            Err(_) => {
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }
            }
            _ => (),
        }
    }
}

async fn save_clip(
    client: Arc<Mutex<Client>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    client.lock().await.replay_buffer().save().await?;
    Ok(())
}

async fn save_screenshot(
    client: Arc<Mutex<Client>>,
    config: Arc<AppConfig>,
    stat: &Stat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !config.screenshot.enabled {
        return Ok(());
    }

    let clip_path = std::path::Path::new(&config.clips_folder).join(&stat.scenario);

    fs::create_dir_all(&clip_path)
        .await
        .expect("Failed to create screenshots output directory");

    let clip_path = clip_path.join(format!("{}.png", stat.to_string()));

    let options = SaveScreenshot {
        source: obws::requests::sources::SourceId::Name(&config.obs_source_name),
        format: "png",
        width: None,
        height: None,
        compression_quality: Some(0),
        file_path: &clip_path,
    };

    client
        .lock()
        .await
        .sources()
        .save_screenshot(options)
        .await
        .expect("Failed to save screenshot");

    Ok(())
}
