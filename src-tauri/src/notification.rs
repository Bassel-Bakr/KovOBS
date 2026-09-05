use crate::globals::APP_HANDLE;
use crate::ui_println;
use tauri_plugin_notification::NotificationExt;

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
        .show()
    {
        ui_println!("👎 Failed to show notification: {e:?}");
    }
}
