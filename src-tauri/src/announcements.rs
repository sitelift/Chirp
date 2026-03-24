use crate::settings::config_dir;
use serde::{Deserialize, Serialize};

const ANNOUNCEMENTS_URL: &str =
    "https://raw.githubusercontent.com/sitelift/chirp-meta/main/announcements.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub min_version: Option<String>,
    #[serde(default)]
    pub max_version: Option<String>,
}

fn cache_path() -> std::path::PathBuf {
    config_dir().join("announcements_cache.json")
}

fn seen_path() -> std::path::PathBuf {
    config_dir().join("announcements_seen.json")
}

pub fn load_seen() -> Vec<String> {
    match std::fs::read_to_string(seen_path()) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_seen(ids: &[String]) -> Result<(), String> {
    let dir = config_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create config dir: {e}"))?;
    let data =
        serde_json::to_string_pretty(ids).map_err(|e| format!("Failed to serialize: {e}"))?;
    std::fs::write(seen_path(), data).map_err(|e| format!("Failed to write seen list: {e}"))
}

/// Returns true if `app_version` falls within the optional semver range of the announcement.
pub fn version_matches(announcement: &Announcement, app_version: &str) -> bool {
    let Ok(version) = semver::Version::parse(app_version) else {
        // If the app version can't be parsed, show the announcement anyway
        return true;
    };

    if let Some(ref min) = announcement.min_version {
        if let Ok(min_ver) = semver::Version::parse(min) {
            if version < min_ver {
                return false;
            }
        }
    }

    if let Some(ref max) = announcement.max_version {
        if let Ok(max_ver) = semver::Version::parse(max) {
            if version > max_ver {
                return false;
            }
        }
    }

    true
}

/// Fetch announcements from GitHub, falling back to cache on failure.
/// Filters by semver version range and already-seen ids.
pub async fn fetch_announcements(app_version: &str) -> Vec<Announcement> {
    let all: Vec<Announcement> = match fetch_remote().await {
        Ok(items) => {
            // Persist to cache on success
            if let Ok(data) = serde_json::to_string_pretty(&items) {
                let dir = config_dir();
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::fs::write(cache_path(), data);
            }
            items
        }
        Err(e) => {
            log::warn!("Failed to fetch announcements: {e}, falling back to cache");
            load_cache()
        }
    };

    let seen = load_seen();

    all.into_iter()
        .filter(|a| !seen.contains(&a.id))
        .filter(|a| version_matches(a, app_version))
        .collect()
}

async fn fetch_remote() -> Result<Vec<Announcement>, String> {
    let response = reqwest::get(ANNOUNCEMENTS_URL)
        .await
        .map_err(|e| format!("Request failed: {e}"))?;

    response
        .json::<Vec<Announcement>>()
        .await
        .map_err(|e| format!("Failed to parse announcements JSON: {e}"))
}

fn load_cache() -> Vec<Announcement> {
    match std::fs::read_to_string(cache_path()) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|e| {
            log::warn!("Corrupted announcements cache, ignoring: {e}");
            Vec::new()
        }),
        Err(_) => Vec::new(),
    }
}
