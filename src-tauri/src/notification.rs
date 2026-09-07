use crate::globals::APP_HANDLE;
use crate::ui_println;
use std::path::{Path, PathBuf};

/// XDG only makes the notification body clickable when an action is registered
/// for it, and the key has to be `default`.
#[cfg(not(windows))]
const DEFAULT_ACTION: &str = "default";

const ACTION_LABEL: &str = "Show in folder";

/// Shows a desktop notification for a saved clip. Clicking it opens the folder
/// the clip landed in.
///
/// Failures are reported to the log panel rather than propagated: a notification
/// that didn't appear is never a good reason to fail the clip that was saved.
pub fn clip_saved(title: &str, body: &str, clip: &Path, sound: bool) {
    show(title.to_owned(), body.to_owned(), clip.to_path_buf(), sound);
}

/// Windows keeps a toast in the Action Center long after it has left the
/// screen, so a click can arrive at any point -- including after KovOBS has
/// been closed.
///
/// That rules out handling the click ourselves. An `on_activated` handler only
/// exists while the process does, and a click on a toast belonging to a dead
/// process is routed through a COM activator, which means a registered CLSID
/// and an `INotificationActivationCallback` implementation.
///
/// Declaring the action `activationType="protocol"` hands the whole job to
/// Windows instead: it opens the `file://` URL with the default handler, which
/// is Explorer. Nothing has to be listening, so it works with KovOBS closed,
/// and there is no callback thread to get the COM apartment wrong on.
///
/// `tauri-winrt-notification` cannot express this -- its template is a fixed
/// `<toast {duration} {scenario}>` with nowhere to put `launch` or
/// `activationType` -- so the XML is built here.
#[cfg(windows)]
fn show(title: String, body: String, clip: PathBuf, sound: bool) {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    // The folder, not the clip: a file:// URL to a video would play it, which
    // is not what "Show in folder" says.
    let Some(folder) = clip.parent() else {
        return;
    };

    let target = file_url(folder);

    // Toasts play the default sound unless told not to.
    let audio = if sound {
        ""
    } else {
        "<audio silent='true'/>"
    };

    let xml = format!(
        "<toast duration='long' activationType='protocol' launch='{target}'>\
           <visual>\
             <binding template='ToastGeneric'>\
               <text>{}</text>\
               <text>{}</text>\
             </binding>\
           </visual>\
           <actions>\
             <action content='{}' activationType='protocol' arguments='{target}'/>\
           </actions>\
           {audio}\
         </toast>",
        escape(&title),
        escape(&body),
        escape(ACTION_LABEL),
    );

    let show = || -> windows::core::Result<()> {
        let document = XmlDocument::new()?;
        document.LoadXml(&HSTRING::from(&xml))?;

        let toast = ToastNotification::CreateToastNotification(&document)?;

        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id()))?.Show(&toast)
    };

    if let Err(e) = show() {
        ui_println!("👎 Failed to show notification: {e}");
    }
}

/// Windows only attributes a toast to an app that owns an AppUserModelID, which
/// the installer registers on the Start Menu shortcut. Running from `target/`
/// there is no id to claim, so the toast borrows PowerShell's: it still shows,
/// but it is not attributed to KovOBS.
#[cfg(windows)]
fn app_id() -> String {
    const POWERSHELL: &str =
        "{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\\WindowsPowerShell\\v1.0\\powershell.exe";

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
        _ => POWERSHELL.to_owned(),
    }
}

/// A `file://` URL Windows will hand to Explorer.
///
/// Percent-encoding is the part that matters: a clips folder with a space in it
/// otherwise produces a URL that opens the wrong place, or nothing at all, with
/// no error either way.
#[cfg(windows)]
fn file_url(folder: &Path) -> String {
    let mut url = String::from("file:///");

    for byte in folder.to_string_lossy().replace('\\', "/").bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => {
                url.push(byte as char)
            }
            _ => url.push_str(&format!("%{byte:02X}")),
        }
    }

    url
}

/// XML-escapes text going into an attribute or element. Scenario names come
/// from the game by way of the user, so they cannot be assumed XML-safe.
#[cfg(windows)]
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
        .replace('"', "&quot;")
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

#[cfg(not(windows))]
fn reveal(clip: &Path) {
    use tauri_plugin_opener::OpenerExt;

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
                ui_println!(
                    "👎 The clip is no longer where it was saved: {}",
                    clip.display()
                );
                return;
            }
        }
    };

    if let Err(e) = app_handle.opener().reveal_item_in_dir(target) {
        ui_println!("👎 Failed to open the clip folder: {e:?}");
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{escape, file_url};
    use std::path::Path;

    #[test]
    fn backslashes_become_forward_slashes() {
        assert_eq!(
            file_url(Path::new(r"E:\OBS\KovOBS")),
            "file:///E:/OBS/KovOBS"
        );
    }

    /// A space is the case that quietly opens the wrong folder.
    #[test]
    fn spaces_are_percent_encoded() {
        assert_eq!(
            file_url(Path::new(r"E:\OBS\My Clips")),
            "file:///E:/OBS/My%20Clips"
        );
    }

    /// An unescaped ampersand would end the attribute and break the whole toast,
    /// so it has to survive both encoding and escaping.
    #[test]
    fn ampersands_survive() {
        assert_eq!(
            file_url(Path::new(r"E:\Clips\A & B")),
            "file:///E:/Clips/A%20%26%20B"
        );
        assert_eq!(escape("Ctrl<click> & 'go'"), "Ctrl&lt;click&gt; &amp; &apos;go&apos;");
    }

    /// Scenario names are frequently non-ASCII; those must not reach the XML raw.
    #[test]
    fn non_ascii_is_encoded() {
        assert_eq!(
            file_url(Path::new(r"E:\Clips\né")),
            "file:///E:/Clips/n%C3%A9"
        );
    }
}
