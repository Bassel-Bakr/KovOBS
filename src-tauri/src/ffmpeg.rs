use crate::ui_println;
use anyhow::Context;
use chrono::TimeDelta;
use indicatif::{ProgressBar, ProgressStyle};
use std::path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Trims the last `trailing_duration` of `in_file` and writes the result to
/// `out_file` using FFmpeg.
///
/// Additional FFmpeg arguments are appended after the input arguments and
/// before the output path. Progress is tracked using FFmpeg's `-progress`
/// output and displayed via an `indicatif::ProgressBar`.
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

    let pb = ProgressBar::new(trailing_duration.num_microseconds().unwrap().try_into()?);

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

    ui_println!("🗃️ Saved clip: {}", out_file.to_string_lossy());

    Ok(())
}
