//! Starting, stopping and quitting.

use crate::events::AppEvent;
use crate::globals::{APP_HANDLE, APP_STATE};
use crate::{events, kovobs, ui_println};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tauri::command]
pub async fn is_ready() -> bool {
    let state = &APP_STATE.wait().await.lock().await;
    state.is_ready
}

#[tauri::command]
pub async fn is_running() -> bool {
    let state = &APP_STATE.wait().await.lock().await;
    state.is_running
}

#[tauri::command]
pub async fn init_app() -> Result<(), String> {
    kovobs::init().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_app() -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    // If it's still running, do nothing
    if state.is_running {
        return Err(String::from("Already running"));
    }
    // TODO: refactor this part
    // Set true here to avoid 2+ threads running 2+ instances of the app
    // because the other place for setting is_running runs after releasing the lock
    state.is_running = true;

    // If it's closed, reopen it
    if state.task_tracker.is_closed() {
        state.task_tracker.reopen();
    }

    let cancellation_token = Arc::new(CancellationToken::new());
    state.cancellation_token.replace(cancellation_token.clone());

    let startup_task = async move {
        let res = tokio::select! {
            res = cancellation_token.cancelled() => Ok(res),
            res = kovobs::start() => res,
        };

        if let Err(e) = res {
            ui_println!("{e:?}");
        }
    };

    let handle = tauri::async_runtime::handle();
    state.task_tracker.spawn_on(startup_task, handle.inner());

    Ok(())
}

#[tauri::command]
pub async fn stop_app() -> Result<(), String> {
    let state = &mut APP_STATE.wait().await.lock().await;

    if let Some(cancellation_token) = state.cancellation_token.take() {
        cancellation_token.cancel();
    }

    state.stop();
    state.task_tracker.close();
    state.task_tracker.wait().await;
    events::emit(AppEvent::Running(false))?;
    ui_println!("⛔ Stopped");

    Ok(())
}

/// Exits for real, rather than hiding to the tray the way closing the window
/// does.
///
/// Asks first. Quitting stops clipping silently, and the window closing to the
/// tray trains you to expect that a close is harmless -- so the one action that
/// isn't should say so. Confirming here rather than in the UI covers the tray's
/// Quit and the window's context menu at the same time.
///
/// Stops before exiting: `kovobs::run` saves the cache on its way out, and
/// exiting straight away skips that, losing whatever personal bests this
/// session recorded.
#[tauri::command]
pub async fn quit_app() {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let Some(app_handle) = APP_HANDLE.get() else {
        return;
    };

    let handle = app_handle.clone();

    app_handle
        .dialog()
        .message("Clips will stop being saved until you start KovOBS again.")
        .title("Quit KovOBS?")
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Quit".into(),
            "Cancel".into(),
        ))
        .show(move |confirmed| {
            if !confirmed {
                return;
            }

            tauri::async_runtime::spawn(async move {
                _ = stop_app().await;
                handle.exit(0);
            });
        });
}
