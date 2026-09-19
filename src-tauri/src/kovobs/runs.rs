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

/// How many file events can wait while a run is being handled.
///
/// Handling a run outlives the run itself by `trim_padding_end`, so the next
/// run can finish while this one is still being dealt with. Anything short of
/// a real queue here blocks the watcher thread and loses that run.
const EVENT_QUEUE_SIZE: usize = 64;

/// How long to let a file settle before treating it as one run.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// Where Aimbeast files a scenario's statistics, by where the scenario came
/// from.
const AIMBEAST_SCENARIO_FOLDERS: [&str; 3] = ["Normal", "Ranked", "Custom"];

/// The statistics files waiting to be handled, in the order they arrived.
///
/// A queue rather than a single slot: handling a run outlives the run by
/// `trim_padding_end`, so the next run can finish while this one is still being
/// dealt with, and holding one entry overall would drop it.
#[derive(Debug, Default)]
struct PendingRuns(Vec<std::path::PathBuf>);

impl PendingRuns {
    /// Queues a statistics file, and says whether it was new.
    ///
    /// One run is written in several bursts, so the same file arriving again is
    /// already queued and is not a second run.
    fn queue(&mut self, path: &std::path::Path) -> bool {
        let is_stat_file = path.extension().is_some_and(|ext| ext == "json");

        if !is_stat_file || self.0.iter().any(|queued| queued == path) {
            return false;
        }

        self.0.push(path.to_path_buf());

        true
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn take(&mut self) -> Vec<std::path::PathBuf> {
        std::mem::take(&mut self.0)
    }
}

/// The files a watcher event says were written, and nothing for the events that
/// only read or removed one.
fn written_paths(event: &notify::Event) -> &[std::path::PathBuf] {
    match event.kind {
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) => &event.paths,
        _ => &[],
    }
}

/// Whether the file is one of KovaaK's per-run statistics files.
fn is_kovaaks_stat_file(file_name: &std::ffi::OsStr) -> bool {
    file_name
        .as_encoded_bytes()
        .ends_with(consts::STAT_FILE_SUFFIX.as_bytes())
}

pub(super) async fn watch_kovaaks_stats_folder(
    config: Arc<AppConfig>,
    client: Arc<Client>,
    cache: Arc<Mutex<Cache>>,
    stat_sender: Arc<mpsc::Sender<Stat>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (tx, mut rx) = mpsc::channel(EVENT_QUEUE_SIZE);

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

                if !is_kovaaks_stat_file(file_name) {
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
    let (tx, mut rx) = mpsc::channel(EVENT_QUEUE_SIZE);

    let stats_folder = std::path::Path::new(&config.aimbeast.stats_folder);

    if !stats_folder.exists() {
        let msg = String::from("Error watching aimbeast stats folder");
        return Err(Box::from(msg));
    }

    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            tx.blocking_send(res).expect("Failed to send file event");
        },
        notify::Config::default(),
    )
    .with_context(|| "Failed to create watcher")?;

    // A scenario you built yourself is still a run worth clipping, and Aimbeast
    // files those under `Custom`. Only the folders that exist are watched: a
    // player who has never made one has no `Custom` folder, and refusing to
    // start over that would cost them the other two.
    let watched: Vec<_> = AIMBEAST_SCENARIO_FOLDERS
        .iter()
        .map(|kind| stats_folder.join(kind))
        .filter(|folder| folder.exists())
        .collect();

    if watched.is_empty() {
        let msg = format!(
            "No Aimbeast scenario folders under {}",
            stats_folder.display()
        );
        return Err(Box::from(msg));
    }

    for folder in &watched {
        watcher
            .watch(folder, notify::RecursiveMode::NonRecursive)
            .with_context(|| format!("Failed to watch {}", folder.display()))?;
    }

    ui_println!("📁 Watching Aimbeast stats");

    let mut pending = PendingRuns::default();

    // Read the totals before any run lands, so the first one of the session is
    // measured rather than averaged.
    let mut lengths = crate::aimbeast::ScenarioLengths::default();
    lengths.prime(stats_folder);
    let mut timer = Box::pin(tokio::time::sleep(Duration::MAX));

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                let event = event.map_err(Box::new)?;

                for path in written_paths(&event) {
                    if pending.queue(path) {
                        ui_println!(
                            "🆕 New stat file detected: {:?}",
                            path.file_name().unwrap_or_default()
                        );
                    }
                }

                if !pending.is_empty() {
                    // Restart the debounce timer
                    timer.as_mut().reset(Instant::now() + DEBOUNCE);
                }
            }

            _ = &mut timer, if !pending.is_empty() => {
                // Sequentially, so OBS is never asked to save two buffers at
                // once and the clips stay paired with the runs that made them.
                for path in pending.take() {
                    if let Err(e) =
                        handle_aimbeast_run(&path, &mut lengths, &config, &client, &stat_sender).await
                    {
                        ui_println!(
                            "👎 Could not handle the run in {:?}:\n{e}",
                            path.file_name().unwrap_or_default()
                        );
                    }
                }
            }
        }
    }
}

/// Reads a run out of an Aimbeast statistics file, and closes the file again.
///
/// Closing it matters as much as reading it. Aimbeast opens these files without
/// sharing them, so while a handle of ours is open its next write to that
/// scenario fails, silently and without touching the file. Holding one across
/// the padding wait cost every run played within `trim_padding_end` of the
/// last: the training log counted them, the statistics file never got them, and
/// nothing was ever clipped for them.
fn read_statistics(
    path: &std::path::Path,
) -> Result<crate::aimbeast::ScenarioStatistics, Box<dyn std::error::Error + Send + Sync>> {
    let mut reader = DecodeReaderBytesBuilder::new()
        .encoding(None) // Auto-detect from BOM, otherwise UTF-8
        .build(std::fs::File::open(path)?);

    Ok(serde_json::from_reader(&mut reader)?)
}

/// Turns one written Aimbeast statistics file into a clip and a screenshot.
///
/// Returns once OBS has been asked for both, which is `trim_padding_end` after
/// the run ended. Runs are handled one at a time, so a run finishing inside
/// that window waits here rather than overlapping with this one.
async fn handle_aimbeast_run(
    path: &std::path::Path,
    lengths: &mut crate::aimbeast::ScenarioLengths,
    config: &Arc<AppConfig>,
    client: &Arc<Client>,
    stat_sender: &mpsc::Sender<Stat>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Wait until it's stable
    utils::wait_for_file(path).await?;

    // The run ended when Aimbeast wrote this file, not now: the debounce, the
    // wait above, and a watcher still busy with the previous run all sit
    // between the two, and a clock reading here would push the whole clip
    // window that far late.
    let end_dt = utils::get_modification_time(path)?;

    let mut stat = read_statistics(path)?;

    stat.scenario = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_default();

    let (new_pb, old_high_score, new_score) =
        (stat.is_pb(), stat.prev_highscore(), stat.last_score());

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
            return Ok(());
        }
    }

    let stats_folder = std::path::Path::new(&config.aimbeast.stats_folder);

    let length = lengths
        .last_run_length(stats_folder, &stat.scenario, end_dt)
        .unwrap_or_else(|| {
            ui_println!(
                "⏱️ No training data for {}, assuming {}s",
                stat.scenario,
                crate::aimbeast::DEFAULT_SCENARIO_LENGTH.as_secs()
            );

            crate::aimbeast::DEFAULT_SCENARIO_LENGTH
        });

    // The one number the user cannot check for themselves. A clip that opens
    // too early or cuts the start off is this being wrong, and without it there
    // is nothing to compare against the run they just played.
    ui_println!("⏱️ Run lasted {:.2}s", length.as_secs_f32());

    let stat = stat.into_stat(end_dt, length);
    stat_sender.send(stat.clone()).await?;

    // Padding is measured from the run, so a late start shortens the wait
    // rather than extending the clip.
    let delay = Arc::new(StatDelay {
        end_dt,
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

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{PendingRuns, is_kovaaks_stat_file, written_paths};
    use std::path::{Path, PathBuf};

    fn event(kind: notify::EventKind, paths: &[&str]) -> notify::Event {
        notify::Event {
            kind,
            paths: paths.iter().map(PathBuf::from).collect(),
            attrs: Default::default(),
        }
    }

    fn queued(pending: &PendingRuns) -> Vec<String> {
        pending
            .0
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn a_statistics_file_is_queued() {
        let mut pending = PendingRuns::default();

        assert!(pending.queue(Path::new("/stats/TEST.json")));
        assert_eq!(queued(&pending), ["/stats/TEST.json"]);
    }

    /// Aimbeast writes plenty beside the per-scenario statistics.
    #[test]
    fn anything_that_is_not_json_is_ignored() {
        let mut pending = PendingRuns::default();

        assert!(!pending.queue(Path::new("/stats/steam_autocloud.vdf")));
        assert!(!pending.queue(Path::new("/stats/TEST")));
        assert!(pending.is_empty());
    }

    /// One run is written in bursts, and the bursts are not separate runs.
    #[test]
    fn the_same_file_is_only_queued_once() {
        let mut pending = PendingRuns::default();

        assert!(pending.queue(Path::new("/stats/TEST.json")));
        assert!(!pending.queue(Path::new("/stats/TEST.json")));
        assert_eq!(queued(&pending), ["/stats/TEST.json"]);
    }

    /// The case this queue exists for: a second run landing while the first is
    /// still being handled must not replace it.
    #[test]
    fn a_second_scenario_does_not_evict_the_first() {
        let mut pending = PendingRuns::default();

        pending.queue(Path::new("/stats/FIRST.json"));
        pending.queue(Path::new("/stats/SECOND.json"));

        assert_eq!(
            queued(&pending),
            ["/stats/FIRST.json", "/stats/SECOND.json"]
        );
    }

    /// Runs are handled in the order they were played.
    #[test]
    fn taking_the_queue_empties_it_in_order() {
        let mut pending = PendingRuns::default();

        pending.queue(Path::new("/stats/FIRST.json"));
        pending.queue(Path::new("/stats/SECOND.json"));

        let taken = pending.take();

        assert_eq!(
            taken,
            [
                PathBuf::from("/stats/FIRST.json"),
                PathBuf::from("/stats/SECOND.json")
            ]
        );
        assert!(pending.is_empty());

        // The same file is a new run once the previous one has been handled.
        assert!(pending.queue(Path::new("/stats/FIRST.json")));
    }

    #[test]
    fn a_written_file_is_taken_from_the_event() {
        use notify::event::{CreateKind, ModifyKind};

        for kind in [
            notify::EventKind::Create(CreateKind::File),
            notify::EventKind::Modify(ModifyKind::Any),
        ] {
            assert_eq!(
                written_paths(&event(kind, &["/stats/TEST.json"])),
                [PathBuf::from("/stats/TEST.json")]
            );
        }
    }

    /// Reading or deleting a statistics file is not a finished run.
    #[test]
    fn an_event_that_wrote_nothing_has_no_paths() {
        use notify::event::{AccessKind, RemoveKind};

        for kind in [
            notify::EventKind::Access(AccessKind::Any),
            notify::EventKind::Remove(RemoveKind::File),
            notify::EventKind::Other,
        ] {
            assert!(written_paths(&event(kind, &["/stats/TEST.json"])).is_empty());
        }
    }

    #[test]
    fn a_kovaaks_statistics_file_is_recognised_by_its_suffix() {
        assert!(is_kovaaks_stat_file(
            "scenario - Challenge - 2026.09.19-00.05.29 Stats.csv".as_ref()
        ));
        assert!(!is_kovaaks_stat_file("scenario.csv".as_ref()));
        assert!(!is_kovaaks_stat_file("Stats.csv".as_ref()));
    }

    /// The file must be closed by the time the run is read out of it. Aimbeast
    /// opens these files without sharing them, so a handle left open makes its
    /// next write to that scenario fail without a trace.
    #[cfg(windows)]
    #[test]
    fn reading_a_run_does_not_keep_the_file_open() {
        use std::os::windows::fs::OpenOptionsExt;

        let path = std::env::temp_dir().join("kovobs_runs_read_statistics.json");
        let json = r#"{"Score":[10.0,20.0]}"#;

        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(json.encode_utf16().flat_map(u16::to_le_bytes));
        std::fs::write(&path, &bytes).expect("written");

        let stat = super::read_statistics(&path).expect("read");
        assert_eq!(stat.last_score(), Some(&20.0));

        // Opening with no sharing at all is how Aimbeast writes: it succeeds
        // only while nothing else holds the file.
        std::fs::OpenOptions::new()
            .write(true)
            .share_mode(0)
            .open(&path)
            .expect("nothing else holds the file");

        std::fs::remove_file(&path).expect("cleaned up");
    }
}
