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

/// Rich detail for one mod, incl. a screenshot gallery parsed from its
/// description (Nexus' API exposes only the main image, but mod descriptions
/// embed their screenshots as `[img]…[/img]`).
#[derive(Serialize, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NexusDetail {
    pub name: String,
    pub summary: String,
    pub description: String,
    pub images: Vec<String>,
    pub downloads: u64,
    pub endorsements: u64,
    pub version: Option<String>,
    pub author: Option<String>,
    pub updated: Option<String>,
    pub adult: bool,
}

/// Collect image URLs from BBCode `[img]…[/img]` and HTML `<img src=…>`,
/// skipping donate/badge images. http→https, deduped.
fn collect_images(desc: &str, main: &str, out: &mut Vec<String>) {
    let add = |raw: &str, out: &mut Vec<String>| {
        let u = raw.trim();
        let url = match u.strip_prefix("http://") {
            Some(rest) => format!("https://{rest}"),
            None => u.to_string(),
        };
        let low = url.to_lowercase();
        let is_img = [".jpg", ".jpeg", ".png", ".webp", ".gif"].iter().any(|e| low.ends_with(e));
        let is_badge = [
            "ko-fi", "kofi", "ko_fi", "paypal", "patreon", "donate", "discord", "buymeacoffee",
            "/logo", "banner", "button", "divider", "separator", "youtube",
        ]
        .iter()
        .any(|b| low.contains(b));
        if is_img && !is_badge && url.starts_with("https://") && !out.iter().any(|x| x == &url) {
            out.push(url);
        }
    };

    if !main.is_empty() {
        add(main, out);
    }
    // ASCII-lowercase preserves byte length, so indices stay valid against `desc`.
    let lower = desc.to_ascii_lowercase();
    // BBCode [img]URL[/img]
    let mut i = 0;
    while let Some(rel) = lower[i..].find("[img]") {
        let start = i + rel + 5;
        match lower[start..].find("[/img]") {
            Some(end) => {
                add(&desc[start..start + end], out);
                i = start + end + 6;
            }
            None => break,
        }
    }
    // HTML <img src="URL">
    let mut j = 0;
    while let Some(rel) = lower[j..].find("src=") {
        let mut k = j + rel + 4;
        let bytes = desc.as_bytes();
        match bytes.get(k) {
            Some(&q) if q == b'"' || q == b'\'' => {
                k += 1;
                match desc[k..].find(q as char) {
                    Some(end) => {
                        add(&desc[k..k + end], out);
                        j = k + end + 1;
                    }
                    None => break,
                }
            }
            _ => j = k,
        }
    }
}

/// Strip BBCode/HTML to readable plain text (drops `[img]`/`<img>` blocks).
fn strip_markup(s: &str) -> String {
    // Remove [img]…[/img] blocks first so their URLs don't leak into the text.
    // ASCII-lowercase keeps byte length identical, so find() indices line up.
    let lower = s.to_ascii_lowercase();
    let mut s2 = String::with_capacity(s.len());
    let mut pos = 0;
    loop {
        match lower[pos..].find("[img]") {
            Some(rel) => {
                let start = pos + rel;
                s2.push_str(&s[pos..start]);
                match lower[start + 5..].find("[/img]") {
                    Some(erel) => pos = start + 5 + erel + 6,
                    None => {
                        s2.push_str(&s[start..]);
                        break;
                    }
                }
            }
            None => {
                s2.push_str(&s[pos..]);
                break;
            }
        }
    }
    // Drop remaining [bbcode] and <html> tags.
    let mut out = String::with_capacity(s2.len());
    let mut depth = 0i32;
    let mut in_tag = false;
    for ch in s2.chars() {
        match ch {
            '[' => depth += 1,
            ']' => depth = (depth - 1).max(0),
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if depth == 0 && !in_tag => out.push(ch),
            _ => {}
        }
    }
    let out = out
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ");
    // Collapse runs of blank lines / spaces.
    let mut clean = String::with_capacity(out.len());
    let mut blank = 0;
    for line in out.lines() {
        let t = line.trim();
        if t.is_empty() {
            blank += 1;
            if blank <= 1 {
                clean.push('\n');
            }
        } else {
            blank = 0;
            clean.push_str(t);
            clean.push('\n');
        }
    }
    clean.trim().to_string()
}

/// Fetch full detail (incl. screenshot gallery) for one mod.
pub async fn fetch_detail(api_key: &str, id: u32) -> Option<NexusDetail> {
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
    let desc = v.get("description").and_then(|x| x.as_str()).unwrap_or("");
    let main = v.get("picture_url").and_then(|x| x.as_str()).unwrap_or("");
    let mut images = Vec::new();
    collect_images(desc, main, &mut images);
    images.truncate(12);
    Some(NexusDetail {
        name: v.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string(),
        summary: v.get("summary").and_then(|x| x.as_str()).map(clean_summary).unwrap_or_default(),
        description: strip_markup(desc),
        images,
        downloads: v.get("mod_downloads").and_then(|x| x.as_u64()).unwrap_or(0),
        endorsements: v.get("endorsement_count").and_then(|x| x.as_u64()).unwrap_or(0),
        version: v.get("version").and_then(|x| x.as_str()).map(String::from),
        author: v.get("author").and_then(|x| x.as_str()).map(String::from),
        updated: v.get("updated_time").and_then(|x| x.as_str()).map(|s| s.chars().take(10).collect()),
        adult: v.get("contains_adult_content").and_then(|x| x.as_bool()).unwrap_or(false),
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
