//! Version and update checks.

#[tauri::command]
pub async fn about_info() -> crate::update::AboutInfo {
    crate::update::about()
}

#[tauri::command]
pub async fn check_for_update() -> Result<crate::update::UpdateInfo, String> {
    crate::update::check().await
}
