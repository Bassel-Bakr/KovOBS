use crate::consts;
use crate::globals::APP_HANDLE;
use serde::Deserialize;

/// What the About page shows. `latest` is absent when the check has not run.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub release_url: String,
    pub update_available: bool,
}

#[derive(Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
}

pub fn current_version() -> String {
    APP_HANDLE
        .get()
        .map(|app_handle| app_handle.package_info().version.to_string())
        .unwrap_or_default()
}

/// Asks GitHub for the newest published release and compares it with this
/// build. Only ever run when the user asks, so the app makes no network request
/// of its own accord.
pub async fn check() -> Result<UpdateInfo, String> {
    let current = current_version();

    let url = format!(
        "https://api.github.com/repos/{}/releases/latest",
        consts::GITHUB_REPO
    );

    let client = reqwest::Client::builder()
        // GitHub rejects API requests that don't identify themselves.
        .user_agent(concat!("KovOBS/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| e.to_string())?;

    let body = client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Could not reach GitHub: {e}"))?
        .error_for_status()
        .map_err(|e| format!("GitHub returned an error: {e}"))?
        .text()
        .await
        .map_err(|e| e.to_string())?;

    let release: GitHubRelease =
        serde_json::from_str(&body).map_err(|e| format!("Unexpected response: {e}"))?;

    let latest = release.tag_name.trim_start_matches('v').to_owned();

    Ok(UpdateInfo {
        update_available: is_newer(&latest, &current),
        current,
        latest,
        release_url: release.html_url,
    })
}

/// Plain MAJOR.MINOR.PATCH comparison, matching the versions the release
/// workflow publishes. Anything unparsable counts as "not newer" so a malformed
/// tag can never nag the user.
fn is_newer(candidate: &str, current: &str) -> bool {
    match (parse(candidate), parse(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.');

    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;

    if parts.next().is_some() {
        return None;
    }

    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_a_newer_release() {
        assert!(is_newer("0.12.1", "0.12.0"));
        assert!(is_newer("0.13.0", "0.12.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
    }

    #[test]
    fn ignores_same_or_older_releases() {
        assert!(!is_newer("0.12.0", "0.12.0"));
        assert!(!is_newer("0.11.9", "0.12.0"));
    }

    #[test]
    fn treats_unparsable_versions_as_not_newer() {
        assert!(!is_newer("nightly", "0.12.0"));
        assert!(!is_newer("0.12", "0.12.0"));
        assert!(!is_newer("0.12.0.1", "0.12.0"));
    }
}
