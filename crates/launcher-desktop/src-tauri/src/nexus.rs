//! Minimal Nexus Mods API client — fetches **public mod metadata** (cover
//! image, summary, download/endorsement counts) for the curated Skyrim catalog.
//!
//! Uses a FREE personal API key (the `apikey` header). Only metadata endpoints
//! are used; actual file downloads are Premium-gated on Nexus, so installs stay
//! guided (open the page → download → Aurora drops it in).

use serde::Serialize;

const BASE: &str = "https://api.nexusmods.com/v1";
const GAME: &str = "skyrimspecialedition";

/// Live info pulled from Nexus for one mod (any field may be missing).
#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexusInfo {
    pub name: Option<String>,
    pub summary: Option<String>,
    pub image_url: Option<String>,
    pub downloads: Option<u64>,
    pub endorsements: Option<u64>,
    pub author: Option<String>,
}

fn clean_summary(s: &str) -> String {
    s.replace("<br />", " ")
        .replace("<br/>", " ")
        .replace("<br>", " ")
        .replace("&nbsp;", " ")
        .trim()
        .to_string()
}

/// Fetch one mod's metadata. Returns `None` on any error (bad key, network,
/// missing mod) so the catalog falls back to its curated text.
pub async fn fetch_mod(api_key: &str, id: u32) -> Option<NexusInfo> {
    let resp = launcher_core::http::client()
        .get(format!("{BASE}/games/{GAME}/mods/{id}.json"))
        .header("apikey", api_key)
        .header("accept", "application/json")
        .header("user-agent", "AuroraLauncher")
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let v: serde_json::Value = resp.json().await.ok()?;
    Some(NexusInfo {
        name: v.get("name").and_then(|x| x.as_str()).map(String::from),
        summary: v.get("summary").and_then(|x| x.as_str()).map(clean_summary),
        image_url: v.get("picture_url").and_then(|x| x.as_str()).map(String::from),
        downloads: v.get("mod_downloads").and_then(|x| x.as_u64()),
        endorsements: v.get("endorsement_count").and_then(|x| x.as_u64()),
        author: v.get("author").and_then(|x| x.as_str()).map(String::from),
    })
}

/// Validate a key by hitting the authenticated `users/validate` endpoint.
pub async fn validate_key(api_key: &str) -> bool {
    launcher_core::http::client()
        .get(format!("{BASE}/users/validate.json"))
        .header("apikey", api_key)
        .header("accept", "application/json")
        .header("user-agent", "AuroraLauncher")
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}
