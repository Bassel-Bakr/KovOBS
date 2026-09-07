use crate::config::NotificationsConfig;
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
/// Takes the whole config rather than the settings it happens to read today, so
/// adding one does not ripple out to every call site. Whether to notify at all
/// is decided here for the same reason.
///
/// Failures are reported to the log panel rather than propagated: a notification
/// that didn't appear is never a good reason to fail the clip that was saved.
pub fn clip_saved(title: &str, body: &str, clip: &Path, config: &NotificationsConfig) {
    if !config.enabled {
        return;
    }

    show(
        title.to_owned(),
        body.to_owned(),
        clip.parent().map(Path::to_path_buf),
        config.sound,
        config.urgent_clips,
    );
}

/// Reports something that went wrong while KovOBS was running unattended.
///
/// Sent as urgent, which on Windows means the alarm scenario. That is the only
/// thing Do Not Disturb's full-screen rule lets past, and a game in full screen
/// is exactly when this fires -- a failure nobody sees is the same as no
/// failure notification at all. Measured: under that rule a normal toast and an
/// `urgent` one are both suppressed, and only `alarm` appears.
///
/// The cost is that it stays on screen until dismissed. That is the point: the
/// alternative is finding out after the session that clips stopped saving.
///
/// Deliberately has nothing to click beyond dismissing. The useful destination
/// would be the log panel, and there is no way to address it from a toast.
pub fn failed(what: &str, detail: &str, config: &NotificationsConfig) {
    if !config.failures {
        return;
    }

    show(
        format!("KovOBS: {what}"),
        detail.to_owned(),
        None,
        config.sound,
        true,
    );
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
fn show(title: String, body: String, folder: Option<PathBuf>, sound: bool, urgent: bool) {
    use windows::Data::Xml::Dom::XmlDocument;
    use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
    use windows::core::HSTRING;

    let xml = toast_xml(&title, &body, folder.as_deref(), sound, urgent);

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

/// Builds the toast XML.
///
/// Separate from showing it so the shape can be tested: every mistake here is
/// one WinRT reports as a flat rejection, or worse, accepts and renders wrong.
#[cfg(windows)]
fn toast_xml(title: &str, body: &str, folder: Option<&Path>, sound: bool, urgent: bool) -> String {
    // A toast with somewhere to go carries the folder on both the body and the
    // button; one without stays inert rather than pretending to be clickable.
    let (launch, mut buttons) = match folder {
        Some(folder) => {
            let target = file_url(folder);

            (
                format!("activationType='protocol' launch='{target}'"),
                format!(
                    "<action content='{}' activationType='protocol' arguments='{target}'/>",
                    escape(ACTION_LABEL)
                ),
            )
        }
        None => (String::new(), String::new()),
    };

    // The alarm scenario is rejected without at least one button, and an urgent
    // toast has no folder to offer, so give it a way to be dismissed.
    if urgent && buttons.is_empty() {
        buttons = "<action content='Dismiss' activationType='system' arguments='dismiss'/>".into();
    }

    let actions = if buttons.is_empty() {
        String::new()
    } else {
        format!("<actions>{buttons}</actions>")
    };

    let scenario = if urgent { "scenario='alarm'" } else { "" };

    let audio = match (sound, urgent) {
        (false, _) => "<audio silent='true'/>",
        // Alarms loop their sound until dismissed, which is intolerable over a
        // game. One pass is enough to be noticed.
        (true, true) => "<audio loop='false'/>",
        (true, false) => "",
    };

    format!(
        "<toast duration='long' {scenario} {launch}>\
           <visual>\
             <binding template='ToastGeneric'>\
               <text>{}</text>\
               <text>{}</text>\
             </binding>\
           </visual>\
           {actions}\
           {audio}\
         </toast>",
        escape(title),
        escape(body),
    )
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
fn show(title: String, body: String, folder: Option<PathBuf>, sound: bool, urgent: bool) {
    use notify_rust::{Notification, NotificationResponse, Urgency};

    // `wait_for_response` blocks until the notification is acted on or closes,
    // so it gets its own thread rather than a runtime worker.
    std::thread::spawn(move || {
        let mut notification = Notification::new();

        notification.summary(&title).body(&body);

        if urgent {
            // The XDG equivalent of the alarm scenario: daemons keep a critical
            // notification on screen instead of timing it out.
            notification.urgency(Urgency::Critical);
        }

        if folder.is_some() {
            notification.action(DEFAULT_ACTION, ACTION_LABEL);
        }

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

        // Nothing to wait for on a notification with no action.
        let Some(folder) = folder else {
            return;
        };

        let handler = move |response: &NotificationResponse| {
            let activated = matches!(response, NotificationResponse::Default)
                || matches!(response, NotificationResponse::Action(action) if action == DEFAULT_ACTION);

            if activated {
                reveal(&folder);
            }
        };

        // Ends when the notification is clicked, dismissed, or times out.
        if let Err(e) = handle.wait_for_response(handler) {
            ui_println!("👎 Failed to read the notification response: {e:?}");
        }
    });
}

#[cfg(not(windows))]
fn reveal(folder: &Path) {
    use tauri_plugin_opener::OpenerExt;

    let Some(app_handle) = APP_HANDLE.get() else {
        return;
    };

    if !folder.exists() {
        ui_println!(
            "👎 The clip folder is no longer there: {}",
            folder.display()
        );
        return;
    }

    if let Err(e) = app_handle
        .opener()
        .open_path(folder.to_string_lossy(), None::<&str>)
    {
        ui_println!("👎 Failed to open the clip folder: {e:?}");
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{escape, file_url, toast_xml};
    use std::path::Path;

    /// Alarm toasts are rejected outright by WinRT without a button, and a
    /// failure toast has no folder to offer one.
    #[test]
    fn an_urgent_toast_always_has_a_button() {
        let xml = toast_xml("failed", "detail", None, false, true);

        assert!(xml.contains("scenario='alarm'"), "{xml}");
        assert!(xml.contains("arguments='dismiss'"), "{xml}");
    }

    /// A saved clip must not interrupt a game unless asked to.
    #[test]
    fn a_normal_toast_has_no_scenario() {
        let xml = toast_xml("saved", "detail", None, true, false);

        assert!(!xml.contains("scenario"), "{xml}");
        assert!(!xml.contains("<actions>"), "{xml}");
    }

    /// The folder goes on the toast body and the button, so either opens it.
    #[test]
    fn a_folder_is_offered_twice() {
        let xml = toast_xml("saved", "detail", Some(Path::new(r"E:\Clips")), true, false);

        assert!(xml.contains("launch='file:///E:/Clips'"), "{xml}");
        assert!(xml.contains("arguments='file:///E:/Clips'"), "{xml}");
        assert!(xml.contains("activationType='protocol'"), "{xml}");
    }

    /// An alarm loops its sound until dismissed, which is intolerable mid-game.
    #[test]
    fn an_urgent_toast_never_loops_its_sound() {
        let loud = toast_xml("failed", "detail", None, true, true);
        let quiet = toast_xml("failed", "detail", None, false, true);

        assert!(loud.contains("loop='false'"), "{loud}");
        assert!(quiet.contains("silent='true'"), "{quiet}");
    }

    /// A scenario name carrying an apostrophe would otherwise close the
    /// attribute early and take the whole toast with it.
    #[test]
    fn body_text_cannot_break_out_of_the_xml() {
        let xml = toast_xml("saved", "Bardz's <b> & co", None, false, false);

        assert!(xml.contains("Bardz&apos;s &lt;b&gt; &amp; co"), "{xml}");
    }

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
        assert_eq!(
            escape("Ctrl<click> & 'go'"),
            "Ctrl&lt;click&gt; &amp; &apos;go&apos;"
        );
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
