// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::globals::APP_STATE;
use std::collections::BTreeSet;

#[tauri::command]
pub async fn get_obs_sources() -> Result<Vec<String>, String> {
    let state = &APP_STATE.wait().await.lock().await;

    let Some(client) = state.client.as_ref() else {
        return Err(String::from("Not connected to OBS"));
    };

    let sources: BTreeSet<String> = client
        .inputs()
        .list(None)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|input| input.id.name)
        .collect();

    Ok(sources.into_iter().collect())
}
