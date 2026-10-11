//! GameBanana client for Deadlock mods — the same public catalogue Deadlock
//! Mod Manager browses.
//!
//! Only GameBanana's public `apiv11` JSON endpoints are used, and every field
//! is read defensively: GameBanana's records are loosely typed and fields come
//! and go between endpoints, so a missing field leaves a value empty instead
//! of failing the whole page.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{Error, Result};

const API: &str = "https://gamebanana.com/apiv11";

/// GameBanana's id for Deadlock.
pub const GAME_ID: u64 = 20948;

/// One mod as the store lists it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreMod {
    pub id: u64,
    pub name: String,
    pub author: String,
    pub image: Option<String>,
    /// Top-level category, e.g. "Skins", "Gameplay Modifications", "HUD".
    pub category: Option<String>,
    /// The hero a skin is for, when GameBanana files it under one.
    pub hero: Option<String>,
    pub likes: u64,
    pub views: u64,
    pub downloads: u64,
    pub nsfw: bool,
    /// Unix seconds.
    pub added: i64,
    /// Unix seconds; the newest of GameBanana's modified/updated stamps.
    pub updated: i64,
    pub url: String,
    /// True for sound-only mods (the store shows a play button for these).
    pub audio: bool,
}

/// A downloadable file on a mod's page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub url: String,
    pub added: i64,
    pub downloads: u64,
    pub description: String,
}

/// A mod's full page: the listing fields plus its files and description.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDetail {
    #[serde(flatten)]
    pub summary: StoreMod,
    pub description: String,
    pub images: Vec<String>,
    pub files: Vec<ModFile>,
}

/// One page of store results.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorePage {
    pub mods: Vec<StoreMod>,
    pub page: u32,
    pub total: u64,
    pub complete: bool,
}

/// A store category with its GameBanana id.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: u64,
    pub name: String,
    pub count: u64,
    pub icon: Option<String>,
}

/// How the store sorts its results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    Newest,
    Updated,
    Liked,
    Viewed,
}

impl Sort {
    fn param(self) -> &'static str {
        match self {
            Sort::Newest => "Generic_Newest",
            Sort::Updated => "Generic_LatestUpdated",
            Sort::Liked => "Generic_MostLiked",
            Sort::Viewed => "Generic_MostViewed",
        }
    }
}

async fn get(url: &str) -> Result<Value> {
    let resp = crate::http::client()
        .get(url)
        .header("accept", "application/json")
        .header("user-agent", "KawmaLauncher")
        .send()
        .await?
        .error_for_status()?;
    Ok(resp.json().await?)
}

fn enc(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

/// Browse Deadlock mods: a search when `search` is non-empty, else the full
/// catalogue in `sort` order, optionally inside one category.
pub async fn browse(
    sort: Sort,
    search: &str,
    category: Option<u64>,
    page: u32,
    per_page: u32,
) -> Result<StorePage> {
    let page = page.max(1);
    let per = per_page.clamp(1, 50);
    let search = search.trim();
    if !search.is_empty() {
        let url = format!(
            "{API}/Util/Search/Results?_sModelName=Mod&_sOrder=best_match&_idGameRow={GAME_ID}&_sSearchString={}&_nPage={page}&_nPerpage={per}",
            enc(search)
        );
        return Ok(parse_page(&get(&url).await?, page));
    }
    let mut url = format!(
        "{API}/Mod/Index?_nPage={page}&_nPerpage={per}&_aFilters%5BGeneric_Game%5D={GAME_ID}"
    );
    if let Some(c) = category {
        url.push_str(&format!("&_aFilters%5BGeneric_Category%5D={c}"));
    }
    let sorted = format!("{url}&_sSort={}", sort.param());
    match get(&sorted).await {
        Ok(v) => Ok(parse_page(&v, page)),
        // An unrecognised sort shouldn't empty the store: fall back to
        // GameBanana's default order.
        Err(_) => Ok(parse_page(&get(&url).await?, page)),
    }
}

/// The week's most popular Deadlock submissions, topped up with the most
/// liked mods so the home page always has something to show.
pub async fn trending() -> Result<Vec<StoreMod>> {
    let mut out: Vec<StoreMod> = Vec::new();
    if let Ok(v) = get(&format!("{API}/Game/{GAME_ID}/TopSubs")).await {
        for r in v.as_array().into_iter().flatten() {
            if r.get("_sModelName").and_then(Value::as_str).is_some_and(|m| m != "Mod") {
                continue;
            }
            let m = parse_record(r);
            if m.id != 0 && !out.iter().any(|x| x.id == m.id) {
                out.push(m);
            }
        }
    }
    if out.len() < 24 {
        for sort in [Sort::Liked, Sort::Updated] {
            let Ok(page) = browse(sort, "", None, 1, 40).await else { continue };
            for m in page.mods {
                if !out.iter().any(|x| x.id == m.id) {
                    out.push(m);
                }
            }
        }
    }
    if out.is_empty() {
        return Err(Error::other("GameBanana didn't return any mods"));
    }
    Ok(out)
}

/// Deadlock's mod categories (Skins, HUD, Sounds…).
pub async fn categories() -> Result<Vec<Category>> {
    let v = get(&format!(
        "{API}/Mod/Categories?_idGameRow={GAME_ID}&_sSort=a_to_z&_bShowEmpty=false"
    ))
    .await?;
    let list = v.as_array().or_else(|| v.get("_aRecords").and_then(Value::as_array));
    Ok(list
        .into_iter()
        .flatten()
        .map(|c| Category {
            id: num(c, "_idRow"),
            name: string(c, "_sName"),
            count: num(c, "_nItemCount"),
            icon: c.get("_sIconUrl").and_then(Value::as_str).filter(|s| !s.is_empty()).map(String::from),
        })
        .filter(|c| c.id != 0 && !c.name.is_empty())
        .collect())
}

/// A mod's page, with its downloadable files.
pub async fn detail(id: u64) -> Result<ModDetail> {
    let v = get(&format!("{API}/Mod/{id}/ProfilePage")).await?;
    let mut d = parse_detail(&v);
    if d.summary.id == 0 {
        d.summary.id = id;
    }
    Ok(d)
}

/* ------------------------------ parsing -------------------------------- */

fn num(v: &Value, key: &str) -> u64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_u64().or_else(|| n.as_f64().map(|f| f.max(0.0) as u64)).unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn int(v: &Value, key: &str) -> i64 {
    match v.get(key) {
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0),
        Some(Value::String(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_string()
}

fn truthy(v: &Value, key: &str) -> bool {
    match v.get(key) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_i64().unwrap_or(0) != 0,
        Some(Value::String(s)) => s == "true" || s == "1",
        _ => false,
    }
}

/// Every preview image of a record, largest useful size first in each.
fn images(r: &Value) -> Vec<String> {
    let list = r
        .get("_aPreviewMedia")
        .and_then(|m| m.get("_aImages"))
        .and_then(Value::as_array);
    let mut out = Vec::new();
    for img in list.into_iter().flatten() {
        let base = string(img, "_sBaseUrl");
        let file = ["_sFile530", "_sFile800", "_sFile"]
            .iter()
            .map(|k| string(img, k))
            .find(|s| !s.is_empty());
        if let (false, Some(file)) = (base.is_empty(), file) {
            out.push(format!("{}/{}", base.trim_end_matches('/'), file));
        }
    }
    if out.is_empty() {
        // TopSubs carries a ready-made URL instead of preview media.
        let direct = string(r, "_sImageUrl");
        if !direct.is_empty() {
            out.push(direct);
        }
    }
    out
}

fn has_audio(r: &Value) -> bool {
    r.get("_aPreviewMedia")
        .and_then(|m| m.get("_aMetadata"))
        .and_then(|m| m.get("_sAudioUrl"))
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

fn parse_record(r: &Value) -> StoreMod {
    let category = r
        .get("_aRootCategory")
        .map(|c| string(c, "_sName"))
        .filter(|s| !s.is_empty());
    let sub = r.get("_aCategory").map(|c| string(c, "_sName")).unwrap_or_default();
    let name = string(r, "_sName");
    let hero = hero_for(&sub, category.as_deref(), &name);
    let updated = int(r, "_tsDateModified").max(int(r, "_tsDateUpdated")).max(int(r, "_tsDateAdded"));
    StoreMod {
        id: num(r, "_idRow"),
        author: r.get("_aSubmitter").map(|s| string(s, "_sName")).unwrap_or_default(),
        image: images(r).into_iter().next(),
        audio: has_audio(r) || category.as_deref().is_some_and(|c| c.to_lowercase().contains("sound")),
        category,
        hero,
        likes: num(r, "_nLikeCount"),
        views: num(r, "_nViewCount"),
        downloads: num(r, "_nDownloadCount"),
        nsfw: truthy(r, "_bIsNsfw") || truthy(r, "_bHasContentRatings"),
        added: int(r, "_tsDateAdded"),
        updated,
        url: {
            let u = string(r, "_sProfileUrl");
            if u.is_empty() { format!("https://gamebanana.com/mods/{}", num(r, "_idRow")) } else { u }
        },
        name,
    }
}

fn parse_page(v: &Value, page: u32) -> StorePage {
    let meta = v.get("_aMetadata").cloned().unwrap_or(Value::Null);
    let mods: Vec<StoreMod> = v
        .get("_aRecords")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|r| r.get("_sModelName").and_then(Value::as_str).map_or(true, |m| m == "Mod"))
        .map(parse_record)
        .filter(|m| m.id != 0)
        .collect();
    StorePage {
        complete: truthy(&meta, "_bIsComplete") || mods.is_empty(),
        total: num(&meta, "_nRecordCount"),
        page,
        mods,
    }
}

fn parse_detail(v: &Value) -> ModDetail {
    let mut summary = parse_record(v);
    // The profile page names the parent category separately from the
    // (hero) sub-category.
    if let Some(sup) = v.get("_aSuperCategory").map(|c| string(c, "_sName")).filter(|s| !s.is_empty()) {
        summary.category = Some(sup);
    }
    let sub = v.get("_aCategory").map(|c| string(c, "_sName")).unwrap_or_default();
    summary.hero = hero_for(&sub, summary.category.as_deref(), &summary.name);
    let files: Vec<ModFile> = v
        .get("_aFiles")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|f| ModFile {
            id: num(f, "_idRow"),
            name: string(f, "_sFile"),
            size: num(f, "_nFilesize"),
            url: string(f, "_sDownloadUrl"),
            added: int(f, "_tsDateAdded"),
            downloads: num(f, "_nDownloadCount"),
            description: string(f, "_sDescription"),
        })
        .filter(|f| f.id != 0 && !f.url.is_empty())
        .collect();
    if summary.downloads == 0 {
        summary.downloads = files.iter().map(|f| f.downloads).sum();
    }
    ModDetail {
        images: images(v),
        description: strip_html(&string(v, "_sText")),
        files,
        summary,
    }
}

/// Plain text from GameBanana's HTML descriptions.
fn strip_html(s: &str) -> String {
    let s = s.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n").replace("</p>", "\n");
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/* ------------------------------- heroes -------------------------------- */

/// Deadlock's heroes, as the Hero Skins page lists them.
pub const HEROES: &[&str] = &[
    "Abrams", "Apollo", "Bebop", "Billy", "Calico", "Celeste", "Doorman", "Drifter", "Dynamo",
    "Graves", "Grey Talon", "Haze", "Holliday", "Infernus", "Ivy", "Kelvin", "Lady Geist", "Lash",
    "McGinnis", "Mina", "Mirage", "Mo & Krill", "Paige", "Paradox", "Pocket", "Rem", "Seven",
    "Shiv", "Silver", "Sinclair", "Venator", "Victor", "Vindicta", "Viscous", "Vyper", "Warden",
    "Wraith", "Yamato",
];

fn norm(s: &str) -> String {
    s.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The hero a hero name, alias or category refers to.
pub fn match_hero(s: &str) -> Option<&'static str> {
    let n = norm(s);
    if n.is_empty() {
        return None;
    }
    let alias = match n.as_str() {
        "mokrill" | "moandkrill" | "mo" => Some("Mo & Krill"),
        "geist" | "ladygeist" => Some("Lady Geist"),
        "greytalon" | "graytalon" | "talon" => Some("Grey Talon"),
        "mcginnis" | "mcg" => Some("McGinnis"),
        _ => None,
    };
    alias.or_else(|| HEROES.iter().copied().find(|h| norm(h) == n))
}

/// The hero a mod is for: GameBanana files skins under a per-hero
/// sub-category; failing that, a skin whose title names exactly one hero.
pub fn hero_for(sub_category: &str, category: Option<&str>, name: &str) -> Option<String> {
    if let Some(h) = match_hero(sub_category) {
        return Some(h.to_string());
    }
    let is_skin = category.is_some_and(|c| {
        let c = c.to_lowercase();
        c.contains("skin") || c.contains("model")
    }) || sub_category.to_lowercase().contains("skin");
    if !is_skin {
        return None;
    }
    let words: Vec<String> = name
        .split(|c: char| !c.is_alphanumeric() && c != '&')
        .filter(|w| !w.is_empty())
        .map(norm)
        .collect();
    let joined = norm(name);
    let found: Vec<&str> = HEROES
        .iter()
        .copied()
        .filter(|h| {
            let hn = norm(h);
            if h.contains(' ') { joined.contains(&hn) } else { words.iter().any(|w| *w == hn) }
        })
        .collect();
    (found.len() == 1).then(|| found[0].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_index_page() {
        let v: Value = serde_json::from_str(
            r#"{"_aMetadata":{"_nRecordCount":2,"_bIsComplete":true,"_nPerpage":15},
            "_aRecords":[
              {"_idRow":512345,"_sModelName":"Mod","_sName":"Yamato remodel","_sProfileUrl":"https://gamebanana.com/mods/512345",
               "_tsDateAdded":1759000000,"_tsDateModified":1759500000,"_nLikeCount":"18","_nViewCount":4000,
               "_aPreviewMedia":{"_aImages":[{"_sType":"screenshot","_sBaseUrl":"https://images.gamebanana.com/img/ss/mods","_sFile":"a.jpg","_sFile530":"530-90_a.jpg"}]},
               "_aSubmitter":{"_sName":"thiagolima13D"},"_aRootCategory":{"_sName":"Skins"},"_bIsNsfw":false},
              {"_idRow":1,"_sModelName":"Wip","_sName":"not a mod"}
            ]}"#,
        )
        .unwrap();
        let p = parse_page(&v, 1);
        assert_eq!(p.mods.len(), 1);
        let m = &p.mods[0];
        assert_eq!(m.id, 512345);
        assert_eq!(m.author, "thiagolima13D");
        assert_eq!(m.likes, 18);
        assert_eq!(m.updated, 1759500000);
        assert_eq!(m.hero.as_deref(), Some("Yamato"));
        assert_eq!(m.image.as_deref(), Some("https://images.gamebanana.com/img/ss/mods/530-90_a.jpg"));
        assert!(p.complete);
    }

    #[test]
    fn parses_a_profile_page() {
        let v: Value = serde_json::from_str(
            r#"{"_idRow":7,"_sName":"Shadow (Cid Kagenou)","_sText":"Hello<br>world &amp; more",
               "_aSuperCategory":{"_sName":"Skins"},"_aCategory":{"_sName":"Apollo"},
               "_aFiles":[{"_idRow":99,"_sFile":"shadow.zip","_nFilesize":1024,"_sDownloadUrl":"https://gamebanana.com/dl/99","_nDownloadCount":5}]}"#,
        )
        .unwrap();
        let d = parse_detail(&v);
        assert_eq!(d.summary.hero.as_deref(), Some("Apollo"));
        assert_eq!(d.summary.category.as_deref(), Some("Skins"));
        assert_eq!(d.files.len(), 1);
        assert_eq!(d.summary.downloads, 5);
        assert_eq!(d.description, "Hello\nworld & more");
    }

    #[test]
    fn matches_heroes() {
        assert_eq!(match_hero("mo and krill"), Some("Mo & Krill"));
        assert_eq!(match_hero("Grey Talon"), Some("Grey Talon"));
        assert_eq!(match_hero("Gameplay"), None);
        assert_eq!(hero_for("", Some("Skins"), "Spiderman Lash (UPDATED)").as_deref(), Some("Lash"));
        // Two heroes named: ambiguous, so no hero.
        assert_eq!(hero_for("", Some("Skins"), "Hank Wimbleton Drifter and Lash"), None);
        // Not a skin: titles aren't guessed.
        assert_eq!(hero_for("", Some("Gameplay"), "Lash buff"), None);
    }
}
