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
/// The trim itself always runs with the same defaults: a stream copy of every
/// video and audio source, which is a fast remux rather than a re-encode. When
/// `extra` holds any arguments a second pass runs over the trimmed file,
/// placing them in their proper slots of
/// `ffmpeg [global] [input] -i in [output] out`, so the user's arguments can't
/// interfere with the trim. When all three slots are empty the trim is the
/// whole job and only one pass runs.
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
    sibling(out_file, "trimming")
}

/// `<name>.<stage>.<ext>` beside `file`.
fn sibling(file: &path::Path, stage: &str) -> PathBuf {
    let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    file.with_extension(format!("{stage}.{ext}"))
}

/// The container metadata key a KovaaK's run's stats file goes under, as the
/// CSV itself. Prefixed so it cannot be taken for another program's tag.
pub const STATS_METADATA_KEY: &str = "kovobs_kovaaks_stats";

/// Adds `key=value` to the finished clip's metadata, keeping the tags it had.
///
/// A pass of its own after the clip is saved, never part of the trim, so
/// nothing a setting turns on can cost the clip. It is a remux, cheap and
/// lossless. The value goes in through an FFmpeg metadata file rather than
/// `-metadata` on the command line: a long run's stats file runs past the
/// 32,767 characters Windows allows a command line.
///
/// On any failure the clip is left exactly as it was.
pub async fn add_metadata(
    clip: &path::Path,
    key: &str,
    value: &str,
    clip_duration: TimeDelta,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let metadata_file = clip.with_extension("ffmetadata");
    let tagged = sibling(clip, "tagging");

    tokio::fs::write(&metadata_file, ffmetadata(key, value))
        .await
        .with_context(|| format!("Failed to write '{}'", metadata_file.display()))?;

    let tagging = run(
        &metadata_args(clip, &metadata_file, &tagged),
        clip_duration,
        "🏷️ Adding the stats",
    )
    .await;

    _ = tokio::fs::remove_file(&metadata_file).await;

    if let Err(e) = tagging {
        _ = tokio::fs::remove_file(&tagged).await;
        return Err(e);
    }

    // Swap the tagged clip in without ever leaving no clip: the original
    // steps aside first and comes back if the tagged one cannot take its place.
    // `rename` refuses to overwrite on Windows, hence the two steps.
    let untagged = sibling(clip, "untagged");

    if let Err(e) = tokio::fs::rename(clip, &untagged).await {
        _ = tokio::fs::remove_file(&tagged).await;
        return Err(format!("Failed to replace '{}': {e}", clip.display()).into());
    }

    if let Err(e) = tokio::fs::rename(&tagged, clip).await {
        _ = tokio::fs::rename(&untagged, clip).await;
        _ = tokio::fs::remove_file(&tagged).await;
        return Err(format!("Failed to replace '{}': {e}", clip.display()).into());
    }

    _ = tokio::fs::remove_file(&untagged).await;

    Ok(())
}

/// An FFmpeg metadata file (`FFMETADATA1`) setting one global `key=value`.
///
/// The format reserves `=`, `;`, `#`, `\` and line breaks, so each is
/// preceded by a backslash wherever it appears in the key or the value. That
/// includes `\r`, which FFmpeg's own writer leaves bare but its reader ends a
/// line at: a stats file's CRLFs would cut it off after the first line.
fn ffmetadata(key: &str, value: &str) -> String {
    fn escape(text: &str) -> String {
        let mut escaped = String::with_capacity(text.len());

        for c in text.chars() {
            if matches!(c, '=' | ';' | '#' | '\\' | '\r' | '\n') {
                escaped.push('\\');
            }
            escaped.push(c);
        }

        escaped
    }

    format!(";FFMETADATA1\n{}={}\n", escape(key), escape(value))
}

/// Copies `clip` to `out_file` with the global tags of `metadata_file` added.
fn metadata_args(
    clip: &path::Path,
    metadata_file: &path::Path,
    out_file: &path::Path,
) -> Vec<String> {
    vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-nostats".into(),
        "-progress".into(),
        "pipe:1".into(),
        "-y".into(),
        "-i".into(),
        clip.to_string_lossy().into_owned(),
        "-f".into(),
        "ffmetadata".into(),
        "-i".into(),
        metadata_file.to_string_lossy().into_owned(),
        // Every stream the trim kept.
        "-map".into(),
        "0".into(),
        // The clip's own tags first, then the new one on top.
        "-map_metadata".into(),
        "0".into(),
        "-map_metadata".into(),
        "1".into(),
        "-c".into(),
        "copy".into(),
        // MP4 drops a key it has no name for unless told to keep it. Other
        // containers keep any key and ignore this.
        "-movflags".into(),
        "+use_metadata_tags".into(),
        out_file.to_string_lossy().into_owned(),
    ]
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
        // Without an explicit map FFmpeg picks one stream per type, which drops
        // every extra video and audio source a recording may carry.
        "-map".into(),
        "0?".into(),
        // Remux rather than re-encode: the trim should be cheap and lossless.
        "-c".into(),
        "copy".into(),
        out_file.to_string_lossy().into_owned(),
    ]
}

/// Builds `ffmpeg [global] [input] -i in [output] out` from the user's three
/// slots.
///
/// Only two options are imposed, and both are mechanically required rather than
/// a matter of taste: `-progress pipe:1` is what `run` reads to report progress,
/// and `-y` stops FFmpeg prompting on stdin when the output exists, which would
/// hang because nothing writes to stdin.
///
/// The log-shaping options that `trim_args` sets -- `-hide_banner`,
/// `-loglevel error`, `-nostats` -- are deliberately absent here. This pass is
/// the user's command, and its output is theirs to shape. The cost is measured:
/// on a rejected option, the stderr `run` captures and quotes back runs to 12
/// lines instead of 2, the extra 10 being the version banner and library list.
/// Anyone who wants that trimmed can put the same options in the global slot.
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
    // bundled one hasn't been downloaded -- and in tests, where there is no app.
    let program = APP_HANDLE
        .get()
        .and_then(|app_handle| get_ffmpeg_path(app_handle).ok())
        .unwrap_or_else(|| PathBuf::from("ffmpeg"));
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
    use super::{STATS_METADATA_KEY, add_metadata, display_command, ffmetadata, metadata_args};
    use chrono::TimeDelta;
    use std::path::Path;

    /// The shape of a KovaaK's stats file: line breaks, commas, colons, and
    /// the odd character the metadata file format reserves.
    const CSV: &str = "Kill #,Timestamp,Bot\r\n1,21:24:11.069,\"blink\" #1\r\n\r\nScore:,885.119141\r\nCrosshair Color:,#FF0000; a=b \\ c\r\n";

    #[test]
    fn a_metadata_file_escapes_what_the_format_reserves() {
        assert_eq!(
            ffmetadata("meta", "a=b;c#d\\e\r\nf"),
            ";FFMETADATA1\nmeta=a\\=b\\;c\\#d\\\\e\\\r\\\nf\n"
        );
    }

    /// Undoing the escaping gives back exactly the CSV that went in.
    #[test]
    fn a_metadata_file_holds_the_stats_losslessly() {
        let file = ffmetadata(STATS_METADATA_KEY, CSV);
        let line = file
            .strip_prefix(&format!(";FFMETADATA1\n{STATS_METADATA_KEY}="))
            .and_then(|rest| rest.strip_suffix('\n'))
            .expect("one key");

        let mut unescaped = String::new();
        let mut chars = line.chars();
        while let Some(c) = chars.next() {
            unescaped.push(if c == '\\' {
                chars.next().expect("an escaped char")
            } else {
                c
            });
        }

        assert_eq!(unescaped, CSV);
    }

    /// The clip's own tags are kept and the new one goes on top, every stream
    /// is copied, and MP4 is told to keep a key it has no name for.
    #[test]
    fn the_metadata_pass_keeps_the_clip_and_adds_the_tag() {
        let args = metadata_args(
            Path::new("clip.mp4"),
            Path::new("clip.ffmetadata"),
            Path::new("clip.tagging.mp4"),
        );
        let line = args.join(" ");

        assert!(line.contains("-i clip.mp4 -f ffmetadata -i clip.ffmetadata"));
        assert!(line.contains("-map 0 -map_metadata 0 -map_metadata 1 -c copy"));
        assert!(line.contains("-movflags +use_metadata_tags"));
        assert_eq!(args.last().map(String::as_str), Some("clip.tagging.mp4"));
    }

    /// The whole pass against a real FFmpeg, read back from the clip in place
    /// with FFprobe. Needs both on the PATH:
    /// `cargo test --manifest-path src-tauri/Cargo.toml --lib -- --ignored`
    #[test]
    #[ignore = "needs ffmpeg and ffprobe on the PATH"]
    fn real_ffmpeg_embeds_the_stats_readably() {
        let dir = std::env::temp_dir().join("kovobs_ffmpeg_metadata");
        _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("created folder");
        let runtime = tokio::runtime::Runtime::new().expect("runtime");

        for ext in ["mp4", "mkv"] {
            let clip = dir.join(format!("clip.{ext}"));

            let made = std::process::Command::new("ffmpeg")
                .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
                .args([
                    "-i",
                    "testsrc=d=1:s=160x120:r=10",
                    "-metadata",
                    "title=kept",
                ])
                .arg(&clip)
                .status()
                .expect("ffmpeg runs");
            assert!(made.success());

            runtime
                .block_on(add_metadata(
                    &clip,
                    STATS_METADATA_KEY,
                    CSV,
                    TimeDelta::seconds(1),
                ))
                .expect("tagged");

            let probe = std::process::Command::new("ffprobe")
                .args(["-v", "error", "-show_entries", "format_tags", "-of", "json"])
                .arg(&clip)
                .output()
                .expect("ffprobe runs");
            let probed: serde_json::Value =
                serde_json::from_slice(&probe.stdout).expect("ffprobe JSON");
            let tags = &probed["format"]["tags"];

            // Matroska stores tag names in capitals.
            let stats = tags[STATS_METADATA_KEY]
                .as_str()
                .or(tags[STATS_METADATA_KEY.to_uppercase()].as_str())
                .expect("the tag");
            assert_eq!(stats, CSV, "{ext}");
            assert_eq!(tags["title"], "kept", "{ext}");
        }

        // Only the clips themselves are left: no metadata file, no stage.
        let mut left: Vec<String> = std::fs::read_dir(&dir)
            .expect("listed")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        left.sort();
        assert_eq!(left, ["clip.mkv", "clip.mp4"]);

        std::fs::remove_dir_all(&dir).expect("cleaned up");
    }

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
