//! Turning a saved replay buffer into a clip.
//!
//! The other half of [`super::runs`]: that side asks OBS to save, this side
//! reacts to it having done so.

use crate::stat::StatType;
use crate::{config::AppConfig, ffmpeg, notification, stat::Stat, ui_println, utils};
use anyhow::Context;
use chrono::TimeDelta;
use futures_util::StreamExt;
use obws::{Client, events::Event::ReplayBufferSaved};
use std::path;
use std::{sync::Arc, time::Duration};
use tokio::sync::mpsc;
pub(super) async fn listen_to_obs_events(
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

            // One clip failing is not a reason to stop listening. Whatever
            // went wrong -- a bad FFmpeg arg, an unreadable buffer -- applies
            // to this run, and the session has to survive it: the alternative
            // is that a single typo silently ends clipping for the evening.
            if let Err(e) = store_clip(&config, &replay_buffer, &stat).await {
                ui_println!("👎 Could not save the clip for {}:\n{e}", stat.scenario);

                notification::failed(
                    "clip not saved",
                    &format!("{}\n{e}", stat.scenario),
                    &config.notifications,
                );
            }
        }
    }

    Ok(())
}

/// Turns a saved replay buffer into the finished clip, notifies, and cleans up.
///
/// Distinct from [`save_clip`], which asks OBS to write the buffer out in the
/// first place. Every failure in here belongs to a single run, so they are
/// returned rather than propagated out of the event loop.
async fn store_clip(
    config: &AppConfig,
    replay_buffer: &path::Path,
    stat: &Stat,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let output_path = match stat.stat_type {
        StatType::Aimbeast => path::Path::new(&config.aimbeast.clips_folder).join(&stat.scenario),
        StatType::KovaaKs => path::Path::new(&config.clips_folder).join(&stat.scenario),
    };

    let clip_path = output_path.join(format!("{}.mp4", stat));

    // Calculate duration based on the clip time and scenario end time
    let trim_start_point = stat.start_dt - Duration::from_secs_f32(config.trim_padding_start);
    let duration = utils::get_creation_or_modification_time(replay_buffer)? - trim_start_point;

    // TODO: Aimbeast trimming is experimental and fixed at 1m. Figure out how to get the scenario length to fix it
    let trim_duration = if config.trim {
        // Trim using ffmpeg
        duration
    } else {
        // Copy the buffer and don't really trim it
        // Can't think of a scenario longer than 1 day
        TimeDelta::from_std(Duration::from_hours(24))?
    };

    ffmpeg::trim(replay_buffer, &clip_path, trim_duration, &config.ffmpeg).await?;

    notification::clip_saved(
        "Clip saved",
        &stat.to_string(),
        &clip_path,
        &config.notifications,
    );

    // Delete the replay buffer clip if we no longer need it
    if config.delete_after_trimming {
        tokio::fs::remove_file(replay_buffer)
            .await
            .with_context(|| {
                format!(
                    "Failed to delete replay buffer after trimming: {}",
                    replay_buffer.display()
                )
            })?;
    }

    Ok(())
}
