use crate::config::FFmpegConfig;
use crate::globals::APP_HANDLE;
use crate::shell::ShellExt;
use crate::{consts, ui_println};
use anyhow::Context;
use chrono::TimeDelta;
use std::path;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Copy, Clone, serde::Serialize, serde::Deserialize)]
pub struct FFmpegDownloadProgress {
    pub state: &'static str,
    pub progress: f32,
}

impl Default for FFmpegDownloadProgress {
    fn default() -> Self {
        Self {
            state: "NotDone",
            progress: 0.0,
        }
    }
}

pub fn get_ffmpeg_folder_path(app_handle: &AppHandle) -> Result<PathBuf, tauri::Error> {
    app_handle.path().resolve(
        consts::DOWNLOADED_FFMPEG_FOLDER,
        BaseDirectory::AppLocalData,
    )
}

pub fn get_ffmpeg_path(
    app_handle: &AppHandle,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let folder = get_ffmpeg_folder_path(app_handle)?;
    let entries = std::fs::read_dir(folder);

    for entry in entries? {
        let path = entry?.path();
        let ffmpeg_exe = path
            .file_name()
            .map(|n| n.to_string_lossy() == "ffmpeg" || n.to_string_lossy() == "ffmpeg.exe")
            .unwrap_or(false);

        if ffmpeg_exe {
            return Ok(path);
        }
    }

    Err("FFmpeg executable not found".into())
}

/// Trims the last `trailing_duration` of `in_file` and writes the result to
/// `out_file` using FFmpeg.
///
/// The trim itself always runs with the same defaults: a stream copy, which is
/// a fast remux rather than a re-encode. When `extra` holds any arguments a
/// second pass runs over the trimmed file, placing them in their proper slots
/// of `ffmpeg [global] [input] -i in [output] out`, so the user's arguments
/// can't interfere with the trim. When all three slots are empty the trim is
/// the whole job and only one pass runs.
///
/// The output directory is created automatically if it does not already exist.
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
    extra: &FFmpegConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tokio::fs::create_dir_all(&out_file.parent().expect("No parent directory"))
        .await
        .with_context(|| format!("Failed to create output directory '{}'", out_file.display()))?;

    if extra.is_empty() {
        run(
            &trim_args(in_file, out_file, trailing_duration),
            trailing_duration,
            "✂️ Trimming",
        )
        .await?;
        ui_println!("🗃️ Saved clip: {}", out_file.to_string_lossy());
        return Ok(());
    }

    // Trim to a scratch file first so the user's arguments apply to the trimmed
    // clip rather than competing with the trim's own output options.
    let intermediate = intermediate_path(out_file);

    run(
        &trim_args(in_file, &intermediate, trailing_duration),
        trailing_duration,
        "✂️ Trimming",
    )
    .await?;

    let result = run(
        &extra_command(&intermediate, out_file, extra),
        trailing_duration,
        "🎛️ Applying FFmpeg args",
    )
    .await;

    // Clean up whether or not the second pass worked.
    _ = tokio::fs::remove_file(&intermediate).await;

    result?;

    ui_println!("🗃️ Saved clip: {}", out_file.to_string_lossy());

    Ok(())
}

/// A sibling of `out_file` that the trim writes to before the second pass.
fn intermediate_path(out_file: &path::Path) -> PathBuf {
    let ext = out_file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("mp4");
    out_file.with_extension(format!("trimming.{ext}"))
}

fn trim_args(
    in_file: &path::Path,
    out_file: &path::Path,
    trailing_duration: TimeDelta,
) -> Vec<String> {
    vec![
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
        // Remux rather than re-encode: the trim should be cheap and lossless.
        "-c".into(),
        "copy".into(),
        out_file.to_string_lossy().into_owned(),
    ]
}

/// Builds `ffmpeg [global] [input] -i in [output] out` from the user's three
/// slots.
///
/// Only two options are imposed, and both do real work: `-progress pipe:1` is
/// what `run` reads to report progress, and `-y` stops FFmpeg prompting on stdin
/// when the output exists, which would hang because nothing writes to stdin.
/// `-hide_banner`, `-loglevel` and `-nostats` are deliberately absent: they only
/// shape stderr, which isn't captured, so here they'd do nothing.
///
/// The user's args come after, so both can still be overridden.
fn extra_command(in_file: &path::Path, out_file: &path::Path, extra: &FFmpegConfig) -> Vec<String> {
    let mut args: Vec<String> = vec!["-progress".into(), "pipe:1".into(), "-y".into()];

    args.extend(extra.global_args.iter().cloned());
    args.extend(extra.input_args.iter().cloned());

    args.push("-i".into());
    args.push(in_file.to_string_lossy().into_owned());

    args.extend(extra.output_args.iter().cloned());
    args.push(out_file.to_string_lossy().into_owned());

    args
}

/// Runs FFmpeg, reporting progress against `total_duration` under `label`.
async fn run(
    args: &[String],
    total_duration: TimeDelta,
    label: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ffmpeg_path = {
        let app_handle = APP_HANDLE.get().unwrap();
        get_ffmpeg_path(app_handle)
    };

    let mut ffmpeg_cmd = if let Ok(path) = ffmpeg_path {
        Command::new(path)
    } else {
        Command::new("ffmpeg")
    };

    let mut process = ffmpeg_cmd
        .no_window()
        .args(args)
        .stdout(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute ffmpeg {args:?}"))?;

    let reader = BufReader::new(process.stdout.take().unwrap());

    let mut lines = reader.lines();

    let duration_micros = total_duration.num_microseconds().unwrap().try_into()?;
    while let Some(line) = lines.next_line().await? {
        if let Some(ms) = line.strip_prefix("out_time_ms=") {
            let current_ms: u64 = ms.parse()?;

            // Clamp to avoid going over 100%
            let current_micros = current_ms.min(duration_micros);
            let progress_percent = 100f32 * current_micros as f32 / duration_micros as f32;
            ui_println!("{label} in progress ({progress_percent:.2}%)");
        }
    }

    let status = process.wait().await?;

    if !status.success() {
        return Err(format!("FFmpeg exited with {status}").into());
    }

    Ok(())
}
