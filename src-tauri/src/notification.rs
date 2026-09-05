use crate::globals::APP_HANDLE;
use crate::ui_println;
use tauri_plugin_notification::NotificationExt;

// The platforms don't share a sound vocabulary. Windows expects one of the WinRT
// toast names ("Default", "IM", "Mail", "Reminder", "SMS"); Linux expects a name
// from the XDG sound naming spec, passed through as the `sound-name` hint.
#[cfg(windows)]
const SOUND: &str = "Default";
#[cfg(not(windows))]
const SOUND: &str = "complete";

/// Shows a desktop notification, reporting failures to the log panel instead of
/// propagating them: a notification that didn't appear is never a good reason to
/// fail the clip that was just saved.
pub fn notify(title: &str, body: &str) {
    let Some(app_handle) = APP_HANDLE.get() else {
        return;
    };

    if let Err(e) = app_handle
        .notification()
        .builder()
        .title(title)
        .body(body)
        .sound(SOUND)
        .show()
    {
        ui_println!("👎 Failed to show notification: {e:?}");
    }
}
