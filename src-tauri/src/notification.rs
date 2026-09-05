use crate::globals::APP_HANDLE;
use crate::ui_println;
use std::path::{Path, PathBuf};
use tauri_plugin_opener::OpenerExt;

/// XDG only makes the notification body clickable when an action is registered
/// for it, and the key has to be `default`. Windows reports a body click as no
/// argument at all, and the button click as whatever key it was given.
const DEFAULT_ACTION: &str = "default";

const ACTION_LABEL: &str = "Show in folder";

/// Shows a desktop notification for a saved clip. Clicking it reveals the clip
/// in the system file manager.
///
/// Failures are reported to the log panel rather than propagated: a notification
/// that didn't appear is never a good reason to fail the clip that was saved.
pub fn clip_saved(title: &str, body: &str, clip: &Path, sound: bool) {
    show(title.to_owned(), body.to_owned(), clip.to_path_buf(), sound);
}

/// Windows keeps a toast in the Action Center long after it has left the
/// screen, and a click there activates it just like a click on the toast
/// itself. The handler therefore has to outlive the toast being displayed,
/// which rules out `notify-rust`: its `wait_for_response` takes a single
/// message from a channel fed by *both* activation and dismissal, so the
/// timeout that moves the toast to the Action Center ends the wait and drops
/// the receiver. Every later click is then sent to nobody.
///
/// Driving the toast directly leaves the activation handler registered for as
/// long as the notification exists, so clicking it in the Action Center works
/// too. It also needs no thread: the callback is invoked by the system.
#[cfg(windows)]
fn show(title: String, body: String, clip: PathBuf, sound: bool) {
    use tauri_winrt_notification::{Duration, Sound, Toast};

    let toast = Toast::new(&app_id())
        .title(&title)
        .text1(&body)
        .duration(Duration::Short)
        .sound(sound.then_some(Sound::Default))
        .add_button(ACTION_LABEL, DEFAULT_ACTION)
        .on_activated(move |action| {
            // A click on the toast body carries no argument; the button carries
            // its key. Both mean the same thing here.
            if action.is_none() || action.as_deref() == Some(DEFAULT_ACTION) {
                reveal(&clip);
            }

            Ok(())
        });

    if let Err(e) = toast.show() {
        ui_println!("👎 Failed to show notification: {e:?}");
    }
}

/// Windows only routes toast activation back to an app that owns an
/// AppUserModelID, which only an installed build has. Running from `target/`
/// there is no id to claim, so the toast borrows PowerShell's: it still shows,
/// and the in-process handler still fires, but it is not attributed to KovOBS.
#[cfg(windows)]
fn app_id() -> String {
    use tauri_winrt_notification::Toast;

    let installed = || {
        let exe = tauri::utils::platform::current_exe().ok()?;
        let dir = exe.parent()?.display().to_string();
        let sep = std::path::MAIN_SEPARATOR;

        Some(
            !(dir.ends_with(&format!("{sep}target{sep}debug"))
                || dir.ends_with(&format!("{sep}target{sep}release"))),
        )
    };

    match (APP_HANDLE.get(), installed()) {
        (Some(app_handle), Some(true)) => app_handle.config().identifier.clone(),
        _ => Toast::POWERSHELL_APP_ID.to_owned(),
    }
}

#[cfg(not(windows))]
fn show(title: String, body: String, clip: PathBuf, sound: bool) {
    use notify_rust::{Notification, NotificationResponse};

    // `wait_for_response` blocks until the notification is acted on or closes,
    // so it gets its own thread rather than a runtime worker.
    std::thread::spawn(move || {
        let mut notification = Notification::new();

        notification
            .summary(&title)
            .body(&body)
            .action(DEFAULT_ACTION, ACTION_LABEL);

        if sound {
            // A name from the XDG sound naming spec, passed through as the
            // `sound-name` hint.
            notification.sound_name("complete");
        }

        let handle = match notification.show() {
            Ok(handle) => handle,
            Err(e) => {
                ui_println!("👎 Failed to show notification: {e:?}");
                return;
            }
        };

        let handler = move |response: &NotificationResponse| {
            let activated = matches!(response, NotificationResponse::Default)
                || matches!(response, NotificationResponse::Action(action) if action == DEFAULT_ACTION);

            if activated {
                reveal(&clip);
            }
        };

        // Ends when the notification is clicked, dismissed, or times out.
        if let Err(e) = handle.wait_for_response(handler) {
            ui_println!("👎 Failed to read the notification response: {e:?}");
        }
    });
}

fn reveal(clip: &Path) {
    let Some(app_handle) = APP_HANDLE.get() else {
        return;
    };

    // Reveal needs something to select. If the clip has since been moved or
    // deleted, fall back to the folder so the click still does something.
    let target = if clip.exists() {
        clip
    } else {
        match clip.parent() {
            Some(folder) if folder.exists() => folder,
            _ => {
                ui_println!("👎 The clip is no longer where it was saved: {}", clip.display());
                return;
            }
        }
    };

    if let Err(e) = app_handle.opener().reveal_item_in_dir(target) {
        ui_println!("👎 Failed to open the clip folder: {e:?}");
    }
}
