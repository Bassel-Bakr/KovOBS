//! The test notification, and finding a clip for it to point at.

use crate::globals::APP_STATE;
use crate::notification;
use std::path::{Path, PathBuf};

/// Fires the same notification a saved clip would, so the whole chain -- the
/// toast appearing, the click reaching us, the folder opening -- can be checked
/// on demand instead of only after a run that has already been lost.
#[tauri::command]
pub async fn test_notification() -> Result<(), String> {
    let config = {
        let state = &APP_STATE.wait().await.lock().await;
        state.config.as_ref().cloned().unwrap_or_default()
    };

    let folder = [&config.clips_folder, &config.aimbeast.clips_folder]
        .into_iter()
        .map(Path::new)
        .find(|folder| folder.is_dir())
        .ok_or("Set a clips folder first, otherwise there is nothing to open.")?;

    // Point at a real clip when there is one. Revealing a folder selects it in
    // its parent, so a test against the folder itself would open the level
    // above the one a real notification opens.
    let target = newest_clip(folder).unwrap_or_else(|| folder.to_path_buf());

    notification::clip_saved(
        "KovOBS test",
        "If clicking this opens your clips folder, notifications are working.",
        &target,
        &config.notifications,
    );

    Ok(())
}

/// The most recently written clip, or `None` if nothing has been saved yet.
///
/// Looks one level down as well as in the folder itself, because clips are
/// filed under a directory per scenario -- the top level holds those
/// directories, not clips. Extensions are not filtered: what ends up there is
/// whatever the user's own FFmpeg args produced.
fn newest_clip(folder: &Path) -> Option<PathBuf> {
    fn files_in(folder: &Path) -> impl Iterator<Item = PathBuf> {
        std::fs::read_dir(folder)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
    }

    files_in(folder)
        .flat_map(|path| {
            if path.is_dir() {
                files_in(&path).collect::<Vec<_>>()
            } else {
                vec![path]
            }
        })
        .filter(|path| path.is_file())
        .max_by_key(|path| path.metadata().and_then(|meta| meta.modified()).ok())
}

#[cfg(test)]
mod tests {
    use super::newest_clip;
    use std::path::{Path, PathBuf};

    /// Builds `<root>/<scenario>/<file>` for each entry, oldest first, touching
    /// them in order so modification time follows the argument order.
    fn tree(name: &str, entries: &[(&str, &str)]) -> PathBuf {
        let root = std::env::temp_dir().join(format!("kovobs-clips-{name}"));
        _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        for (folder, file) in entries {
            let dir = if folder.is_empty() {
                root.clone()
            } else {
                root.join(folder)
            };

            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(file), b"x").unwrap();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }

        root
    }

    /// The layout that matters: clips live under a folder per scenario, so a
    /// top-level scan finds only directories and would report nothing at all.
    #[test]
    fn finds_a_clip_one_level_down() {
        let root = tree("nested", &[("Bounce", "old.mp4"), ("Reactive", "new.mp4")]);

        assert_eq!(
            newest_clip(&root),
            Some(root.join("Reactive").join("new.mp4"))
        );
    }

    /// The screenshot feature writes .png beside the clips, and the user's own
    /// FFmpeg args decide the container, so extension is not a filter.
    #[test]
    fn any_extension_counts() {
        let root = tree("mixed", &[("Bounce", "clip.mp4"), ("Bounce", "shot.png")]);

        assert_eq!(
            newest_clip(&root),
            Some(root.join("Bounce").join("shot.png"))
        );
    }

    #[test]
    fn finds_a_clip_at_the_top_level_too() {
        let root = tree("flat", &[("", "loose.mp4")]);

        assert_eq!(newest_clip(&root), Some(root.join("loose.mp4")));
    }

    /// A folder holding only scenario directories has nothing to reveal, and
    /// the caller falls back to the folder itself.
    #[test]
    fn an_empty_tree_finds_nothing() {
        let root = tree("empty", &[]);
        std::fs::create_dir_all(root.join("Bounce")).unwrap();

        assert_eq!(newest_clip(&root), None);
    }

    #[test]
    fn a_missing_folder_finds_nothing() {
        assert_eq!(newest_clip(Path::new("Z:/nope/nothing/here")), None);
    }
}
