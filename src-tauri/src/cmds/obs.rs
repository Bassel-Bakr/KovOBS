// Learn more about Tauri cmds at https://tauri.app/develop/calling-rust/

use crate::globals::APP_STATE;
use obws::Client;
use std::collections::BTreeSet;

/// The OBS sources the user can pick from.
///
/// Works whether or not a session is running. With one, the session's client
/// answers. Without one, this connects with the saved settings, asks, and
/// disconnects -- so the source dropdowns can be filled in before ever pressing
/// Start, which is when someone is actually configuring them.
#[tauri::command]
pub async fn get_obs_sources() -> Result<Vec<String>, String> {
    let (client, config) = {
        let state = APP_STATE.wait().await.lock().await;
        (state.client.clone(), state.config.clone())
    };

    if let Some(client) = client {
        return list_sources(&client).await;
    }

    let config = config.ok_or("No settings loaded yet")?;

    let mut client = Client::connect(
        &config.obs.host,
        config.obs.port,
        Some(&config.obs.password),
    )
    .await
    .map_err(|e| format!("Could not reach OBS: {e}"))?;

    let sources = list_sources(&client).await;

    // Asked for on its own, so it must not leave a connection behind.
    client.disconnect().await;

    sources
}

async fn list_sources(client: &Client) -> Result<Vec<String>, String> {
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
