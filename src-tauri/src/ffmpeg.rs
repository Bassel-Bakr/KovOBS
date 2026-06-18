use crate::ui_println;
use anyhow::Context;
use chrono::TimeDelta;
use std::path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Trims the last `trailing_duration` of `in_file` and writes the result to
/// `out_file` using FFmpeg.
///
/// Additional FFmpeg arguments are appended after the input arguments and
/// before the output path.
///
/// The output directory is created automatically if it does not already
/// exist.
///
/// # Errors
///
/// Returns an error if:
/// - the output directory cannot be created;
/// - FFmpeg cannot be started;
/// - progress output cannot be read or parsed;
/// - FFmpeg exits unsuccessfully.
pub async fn trim(
    in_file: &path::Path,
    out_file: &path::Path,
    trailing_duration: TimeDelta,
    ffmpeg_args: &[String],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    fs::create_dir_all(&out_file.parent().expect("No parent directory"))
        .await
        .with_context(|| format!("Failed to create output directory '{}'", out_file.display()))?;

    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostats".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-y".into(),
        "-sseof".into(),
        format!("-{:.2}", trailing_duration.as_seconds_f32()),
        "-accurate_seek".into(),
        "-i".into(),
        in_file.to_string_lossy().into_owned(),
    ];

    args.extend(ffmpeg_args.iter().cloned());

    args.push(out_file.to_string_lossy().into_owned());

    let mut process = Command::new("ffmpeg")
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute ffmpeg {:?}", args))?;

    let reader = BufReader::new(process.stdout.take().unwrap());

    let mut lines = reader.lines();

    let duration_micros = trailing_duration.num_microseconds().unwrap().try_into()?;
    while let Some(line) = lines.next_line().await? {
        if let Some(ms) = line.strip_prefix("out_time_ms=") {
            let current_ms: u64 = ms.parse()?;

            // Clamp to avoid going over 100%
            let current_micros = current_ms.min(duration_micros);
            let progress_percent = 100f32 * current_micros as f32 / duration_micros as f32;
            ui_println!("✂️ Trimming in progress ({:.2}%)", progress_percent);
        }
    }

    process.wait().await?;

    ui_println!("🗃️ Saved clip: {}", out_file.to_string_lossy());

    Ok(())
}
