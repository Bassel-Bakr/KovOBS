use crate::args;
use crate::config::FFmpegConfig;
use crate::globals::APP_HANDLE;
use crate::shell::ShellExt;
use crate::{consts, ui_println};
use anyhow::Context;
use chrono::TimeDelta;
use std::borrow::Cow;
use std::collections::VecDeque;
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

/// How many lines of FFmpeg's stderr to keep for an error message.
///
/// Deliberately generous: a failure explains itself across a run of lines
/// rather than in the last one, and a real one runs to a couple of dozen. This
/// exists only to bound memory if FFmpeg decides to complain once per frame.
const STDERR_LIMIT_LINES: usize = 500;

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

    // The arguments are the user's own, so a mistake in them must not cost the
    // clip that was already trimmed successfully. On failure the trimmed file is
    // promoted to the output and the run is still a success.
    match extra_command(&intermediate, out_file, extra) {
        Err(e) => {
            ui_println!(
                "👎 Could not read the FFmpeg args, keeping the trimmed clip instead:\n{e}"
            );
            keep_trimmed(&intermediate, out_file).await?;
        }
        Ok(command) => match run(&command, trailing_duration, "🎛️ Applying FFmpeg args").await
        {
            Ok(()) => {
                _ = tokio::fs::remove_file(&intermediate).await;
            }
            Err(e) => {
                ui_println!("👎 FFmpeg args failed, keeping the trimmed clip instead:\n{e}");
                keep_trimmed(&intermediate, out_file).await?;
            }
        },
    }

    ui_println!("🗃️ Saved clip: {}", out_file.to_string_lossy());

    Ok(())
}

/// Moves the trimmed file into the output path after the arguments pass failed.
async fn keep_trimmed(
    intermediate: &path::Path,
    out_file: &path::Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // A failed pass may have left a partial file behind, and `rename` refuses to
    // overwrite on Windows.
    _ = tokio::fs::remove_file(out_file).await;

    tokio::fs::rename(intermediate, out_file)
        .await
        .with_context(|| {
            format!(
                "Failed to move the trimmed clip to '{}'",
                out_file.display()
            )
        })?;

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
fn extra_command(
    in_file: &path::Path,
    out_file: &path::Path,
    extra: &FFmpegConfig,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut command: Vec<String> = vec!["-progress".into(), "pipe:1".into(), "-y".into()];

    command.extend(args::parse(&extra.global_args)?);
    command.extend(args::parse(&extra.input_args)?);

    command.push("-i".into());
    command.push(in_file.to_string_lossy().into_owned());

    command.extend(args::parse(&extra.output_args)?);
    command.push(out_file.to_string_lossy().into_owned());

    Ok(command)
}

/// Renders a command the way it would be typed, so a failed run can be pasted
/// into a terminal and reproduced.
///
/// Only the parts that need it are quoted, and never with backslash escapes:
/// Windows paths are full of backslashes, and escaping them would make what is
/// printed differ from what actually ran.
fn display_command(program: &path::Path, args: &[String]) -> String {
    std::iter::once(program.to_string_lossy())
        .chain(args.iter().map(|arg| Cow::from(arg.as_str())))
        .map(|part| match &*part {
            // A double quote inside means the single quotes have to do the
            // grouping. Something holding both isn't worth contorting for.
            part if part.contains('"') => format!("'{part}'"),
            part if part.is_empty() || part.contains([' ', '\t', '\n', '\r', '\'']) => {
                format!("\"{part}\"")
            }
            part => part.to_owned(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs FFmpeg, reporting progress against `total_duration` under `label`.
async fn run(
    args: &[String],
    total_duration: TimeDelta,
    label: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Falling back to the bare name lets a system-wide FFmpeg work when the
    // bundled one hasn't been downloaded.
    let program = {
        let app_handle = APP_HANDLE.get().unwrap();
        get_ffmpeg_path(app_handle).unwrap_or_else(|_| PathBuf::from("ffmpeg"))
    };
    let command_line = display_command(&program, args);

    let mut process = Command::new(&program)
        .no_window()
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute {command_line}"))?;

    // Drained on its own task: if the stderr pipe fills while this is reading
    // stdout, FFmpeg blocks on the write and neither side ever finishes.
    let stderr = process.stderr.take().expect("stderr is piped");
    let stderr_tail = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        let mut tail: VecDeque<String> = VecDeque::with_capacity(STDERR_LIMIT_LINES);

        while let Ok(Some(line)) = lines.next_line().await {
            // One line in, so at most one out: the end, where FFmpeg reports
            // what went wrong, is what survives.
            if tail.len() == STDERR_LIMIT_LINES {
                tail.pop_front();
            }

            tail.push_back(line);
        }

        Vec::from(tail).join("\n")
    });

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
    let stderr_tail = stderr_tail.await.unwrap_or_default();

    if !status.success() {
        // FFmpeg returns negative errno values, which Windows renders as an
        // unsigned hex blob (`0xffffffea`). The signed code reads better.
        let code = status
            .code()
            .map_or_else(|| status.to_string(), |code| code.to_string());

        // FFmpeg explains itself on stderr, so that is the part worth reading.
        // The command goes with it: the args are the user's, and a complaint
        // about them rarely makes sense without seeing what actually ran.
        let detail = stderr_tail.trim();

        return Err(if detail.is_empty() {
            format!("FFmpeg exited with code {code}\n{command_line}")
        } else {
            format!("FFmpeg exited with code {code}\n{command_line}\n\n{detail}")
        }
        .into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::display_command;
    use std::path::Path;

    fn render(args: &[&str]) -> String {
        let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();

        display_command(Path::new("ffmpeg"), &args)
    }

    #[test]
    fn plain_args_are_left_alone() {
        assert_eq!(
            render(&["-i", "in.mp4", "-crf", "23"]),
            "ffmpeg -i in.mp4 -crf 23"
        );
    }

    /// The whole point of printing this: a Windows path must come back out
    /// exactly as it went in, backslashes and all.
    #[test]
    fn windows_paths_survive_verbatim() {
        let program = Path::new(r"C:\Users\me\AppData\ffmpeg.exe");
        let args = [r"D:\Clips\run.mp4".to_owned()];

        assert_eq!(
            display_command(program, &args),
            r"C:\Users\me\AppData\ffmpeg.exe D:\Clips\run.mp4"
        );
    }

    #[test]
    fn spaces_are_quoted() {
        assert_eq!(
            render(&["-metadata", "title=My Run", r"D:\My Clips\run.mp4"]),
            "ffmpeg -metadata \"title=My Run\" \"D:\\My Clips\\run.mp4\""
        );
    }

    /// A value already holding double quotes gets single ones, so the grouping
    /// stays unambiguous without inventing an escape the parser doesn't have.
    #[test]
    fn embedded_quotes_flip_the_quoting() {
        assert_eq!(
            render(&["-metadata", r#"title=He said "go""#]),
            "ffmpeg -metadata 'title=He said \"go\"'"
        );
    }

    /// Otherwise an empty argument vanishes from the line entirely, which is the
    /// one case where what is printed would mislead.
    #[test]
    fn empty_args_stay_visible() {
        assert_eq!(render(&["-vf", ""]), "ffmpeg -vf \"\"");
    }
}
