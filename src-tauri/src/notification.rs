use crate::globals::APP_HANDLE;
use crate::ui_println;
use notify_rust::{Notification, NotificationResponse};
use std::path::Path;
use tauri_plugin_opener::OpenerExt;

// The platforms don't share a sound vocabulary. Windows expects one of the WinRT
// toast names ("Default", "IM", "Mail", "Reminder", "SMS"); Linux expects a name
// from the XDG sound naming spec, passed through as the `sound-name` hint.
#[cfg(windows)]
const SOUND: &str = "Default";
#[cfg(not(windows))]
const SOUND: &str = "complete";

/// XDG only makes the notification body clickable when an action is registered
/// for it, and the key has to be `default`. Windows reports a body click as
/// `NotificationResponse::Default` regardless.
const DEFAULT_ACTION: &str = "default";

/// Shows a desktop notification for a saved clip. Clicking it reveals the clip
/// in the system file manager.
///
/// Failures are reported to the log panel rather than propagated: a notification
/// that didn't appear is never a good reason to fail the clip that was saved.
pub fn clip_saved(title: &str, body: &str, clip: &Path) {
    let title = title.to_owned();
    let body = body.to_owned();
    let clip = clip.to_path_buf();

    // `wait_for_response` blocks until the notification is acted on or closes,
    // so it gets its own thread rather than a runtime worker.
    std::thread::spawn(move || {
        let mut notification = Notification::new();

        notification
            .summary(&title)
            .body(&body)
            .sound_name(SOUND)
            .action(DEFAULT_ACTION, "Show in folder");

        set_app_id(&mut notification);

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

/// Windows only routes toast activation back to an app that owns an
/// AppUserModelID, which only an installed build has. Running from `target/`
/// there is no id to claim, and the toast is shown without one.
fn set_app_id(notification: &mut Notification) {
    #[cfg(windows)]
    {
        let Some(app_handle) = APP_HANDLE.get() else {
            return;
        };

        let Ok(exe) = tauri::utils::platform::current_exe() else {
            return;
        };

        let Some(dir) = exe.parent() else {
            return;
        };

        let dir = dir.display().to_string();
        let sep = std::path::MAIN_SEPARATOR;

        if !(dir.ends_with(&format!("{sep}target{sep}debug"))
            || dir.ends_with(&format!("{sep}target{sep}release")))
        {
            notification.app_id(&app_handle.config().identifier);
        }
    }

    #[cfg(not(windows))]
    let _ = notification;
}

fn reveal(clip: &Path) {
    let Some(app_handle) = APP_HANDLE.get() else {
        return;
    };

    if let Err(e) = app_handle.opener().reveal_item_in_dir(clip) {
        ui_println!("👎 Failed to open the clip folder: {e:?}");
    }
}
