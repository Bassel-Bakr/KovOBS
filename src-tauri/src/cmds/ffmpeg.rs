// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::events::AppEvent;
use crate::ffmpeg::FFmpegDownloadProgress;
use crate::globals::APP_HANDLE;
use crate::{events, ffmpeg};
use ffmpeg_sidecar;
use ffmpeg_sidecar::download::FfmpegDownloadProgressEvent;

#[tauri::command]
pub async fn is_ffmpeg_downloaded() -> bool {
    ffmpeg_sidecar::paths::ffmpeg_path().exists()
}

#[tauri::command]
pub async fn remove_ffmpeg() -> Result<(), String> {
    let path = ffmpeg_sidecar::paths::sidecar_dir().map_err(|e| e.to_string())?;
    tokio::fs::remove_dir_all(path)
        .await
        .map_err(|e| e.to_string())?;

    _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
        state: "NotDone",
        progress: 0f32,
    }));

    Ok(())
}

#[tauri::command]
pub async fn download_ffmpeg() -> Result<(), String> {
    let download_path = {
        let app_handle = APP_HANDLE.get().unwrap();
        ffmpeg::get_ffmpeg_folder_path(app_handle).map_err(|e| e.to_string())?
    };

    let url = ffmpeg_sidecar::download::ffmpeg_download_url().map_err(|e| e.to_string())?;
    tokio::fs::create_dir_all(&download_path)
        .await
        .map_err(|e| e.to_string())?;

    let res = ffmpeg_sidecar::download::download_ffmpeg_package_with_progress(
        url,
        &download_path,
        |progress: FfmpegDownloadProgressEvent| match progress {
            FfmpegDownloadProgressEvent::Starting => {
                _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                    state: "Starting",
                    progress: 0f32,
                }))
            }
            FfmpegDownloadProgressEvent::Downloading {
                total_bytes: total,
                downloaded_bytes: downloaded,
            } => {
                _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                    state: "Downloading",
                    progress: (downloaded as f32 / total as f32) * 100f32,
                }))
            }
            _ => (),
        },
    )
    .map_err(|e| e.to_string());

    match res {
        Ok(archive_path) => {
            _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                state: "Unpacking",
                progress: 100f32,
            }));

            let res = ffmpeg_sidecar::download::unpack_ffmpeg(&archive_path, &download_path);

            if let Err(err) = res {
                _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                    state: "NotDone",
                    progress: 0f32,
                }));
                Err(err.to_string())
            } else {
                _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                    state: "Done",
                    progress: 0f32,
                }));

                Ok(())
            }
        }
        Err(e) => {
            _ = events::emit(AppEvent::FFmpegDownloadProgress(FFmpegDownloadProgress {
                state: "NotDone",
                progress: 0f32,
            }));
            Err(e.to_string())
        }
    }
}
