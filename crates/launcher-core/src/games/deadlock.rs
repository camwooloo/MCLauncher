//! Deadlock detection, launch and mod management, modelled on Deadlock Mod
//! Manager.
//!
//! **How Deadlock mods work.** A mod is one or more VPK files. The game loads
//! VPKs from `game/citadel/addons/` once `gameinfo.gi` lists that folder in its
//! `SearchPaths`, and only files named `pakNN_dir.vpk` (01–99). A lower number
//! wins when two mods change the same file.
//!
//! **What the launcher does.** Downloaded mods live in the launcher's own
//! library (`<data>/deadlock/mods/<id>/`), so disabling one never loses it.
//! [`ModLibrary::sync`] then places the enabled mods' VPKs into `addons/` as
//! `pakNN_dir.vpk` in library order, and records which files it wrote so VPKs
//! the player dropped in by hand are never touched.
//!
//! "Play without mods" takes the addons path back out of `gameinfo.gi`, so
//! the game starts vanilla while every mod stays installed.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::games::gamebanana;
use crate::{steam, Error, Result};

/// Deadlock's Steam app id.
pub const APP_ID: u32 = 1422450;

/// The search path line that makes the game load `citadel/addons`.
const ADDONS_PATH: &str = "citadel/addons";
/// Records which `pakNN_dir.vpk` files in `addons/` the launcher placed.
const MANAGED_FILE: &str = ".kawma-managed.json";
/// VPK slots the game reads.
pub const MAX_SLOTS: u32 = 99;

/// Detected state of the Deadlock install.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeadlockInfo {
    pub installed: bool,
    pub install_dir: Option<PathBuf>,
    /// `game/citadel`.
    pub citadel_dir: Option<PathBuf>,
    pub addons_dir: Option<PathBuf>,
    /// Whether `gameinfo.gi` currently loads `citadel/addons`.
    pub mods_active: bool,
    pub gameinfo_found: bool,
    pub has_autoexec: bool,
}

/// Find Deadlock through Steam.
pub fn detect() -> DeadlockInfo {
    let Some(dir) = steam::find_app_install_dir(APP_ID) else {
        return DeadlockInfo::default();
    };
    let citadel = dir.join("game").join("citadel");
    let gameinfo = std::fs::read_to_string(citadel.join("gameinfo.gi")).ok();
    DeadlockInfo {
        installed: true,
        mods_active: gameinfo.as_deref().is_some_and(has_addons_path),
        gameinfo_found: gameinfo.is_some(),
        has_autoexec: citadel.join("cfg").join("autoexec.cfg").exists(),
        addons_dir: Some(citadel.join("addons")),
        citadel_dir: Some(citadel),
        install_dir: Some(dir),
    }
}

fn citadel(info: &DeadlockInfo) -> Result<&PathBuf> {
    info.citadel_dir
        .as_ref()
        .ok_or_else(|| Error::other("Deadlock is not installed"))
}

/* ------------------------------ gameinfo.gi ------------------------------ */

fn tokens(line: &str) -> Vec<String> {
    line.split_whitespace()
        .map(|t| t.trim_matches('"').to_ascii_lowercase())
        .collect()
}

fn is_addons_line(line: &str) -> bool {
    let t = tokens(line);
    t.len() >= 2 && (t[0] == "game" || t[0] == "mod") && t[1] == ADDONS_PATH
}

/// Whether `gameinfo.gi` text loads the addons folder.
pub fn has_addons_path(gameinfo: &str) -> bool {
    gameinfo.lines().any(|l| tokens(l).first().is_some_and(|t| t == "game") && is_addons_line(l))
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Add `Game citadel/addons` as the first search path (after the language
/// path), so addons override the base game. Unchanged if already present or
/// there is no `SearchPaths` block.
pub fn add_addons_path(gameinfo: &str) -> String {
    if has_addons_path(gameinfo) {
        return gameinfo.to_string();
    }
    let nl = newline(gameinfo);
    let lines: Vec<&str> = gameinfo.split(nl).collect();
    let Some(sp) = lines.iter().position(|l| tokens(l).first().is_some_and(|t| t == "searchpaths")) else {
        return gameinfo.to_string();
    };
    let Some(open) = lines.iter().skip(sp).position(|l| l.trim_start().starts_with('{')).map(|i| i + sp) else {
        return gameinfo.to_string();
    };
    // Insert after Game_Language when it leads the block, else after `{`.
    let mut at = open + 1;
    if lines.get(at).is_some_and(|l| tokens(l).first().is_some_and(|t| t == "game_language")) {
        at += 1;
    }
    let indent: String = lines
        .get(at)
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('}'))
        .map(|l| l.chars().take_while(|c| c.is_whitespace()).collect())
        .unwrap_or_else(|| {
            let base: String = lines[open].chars().take_while(|c| c.is_whitespace()).collect();
            format!("{base}\t")
        });
    let mut out: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
    out.insert(at, format!("{indent}Game\t\t\t\t{ADDONS_PATH}"));
    out.join(nl)
}

/// Take every `citadel/addons` search path back out.
pub fn remove_addons_path(gameinfo: &str) -> String {
    let nl = newline(gameinfo);
    gameinfo
        .split(nl)
        .filter(|l| !is_addons_line(l))
        .collect::<Vec<_>>()
        .join(nl)
}

/// Turn the addons search path on or off, keeping a one-time backup of the
/// original `gameinfo.gi` in `backup_dir`. Returns whether the file changed.
pub fn set_mods_active(info: &DeadlockInfo, active: bool, backup_dir: &Path) -> Result<bool> {
    let path = citadel(info)?.join("gameinfo.gi");
    let text = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    let next = if active { add_addons_path(&text) } else { remove_addons_path(&text) };
    if next == text {
        if active && !has_addons_path(&next) {
            return Err(Error::other("gameinfo.gi has no SearchPaths block; verify Deadlock's files in Steam"));
        }
        return Ok(false);
    }
    std::fs::create_dir_all(backup_dir).map_err(|e| Error::io(backup_dir, e))?;
    let backup = backup_dir.join("gameinfo.gi.original");
    if !backup.exists() {
        std::fs::write(&backup, &text).map_err(|e| Error::io(&backup, e))?;
    }
    std::fs::write(&path, next).map_err(|e| Error::io(&path, e))?;
    Ok(true)
}

/// Put `gameinfo.gi` back the way it was before the launcher touched it (or,
/// without a backup, just strip the addons path).
pub fn reset_gameinfo(info: &DeadlockInfo, backup_dir: &Path) -> Result<String> {
    let path = citadel(info)?.join("gameinfo.gi");
    let backup = backup_dir.join("gameinfo.gi.original");
    if let Ok(original) = std::fs::read_to_string(&backup) {
        // A game update may have replaced gameinfo.gi since the backup was
        // taken; the backup is only right if it still matches apart from
        // our line.
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if remove_addons_path(&current) == remove_addons_path(&original) {
            std::fs::write(&path, &original).map_err(|e| Error::io(&path, e))?;
            return Ok("gameinfo.gi restored from the backup".into());
        }
    }
    let current = std::fs::read_to_string(&path).map_err(|e| Error::io(&path, e))?;
    std::fs::write(&path, remove_addons_path(&current)).map_err(|e| Error::io(&path, e))?;
    Ok("Mod paths removed from gameinfo.gi".into())
}

/* ------------------------------- autoexec -------------------------------- */

pub fn autoexec_path(info: &DeadlockInfo) -> Result<PathBuf> {
    Ok(citadel(info)?.join("cfg").join("autoexec.cfg"))
}

pub fn read_autoexec(info: &DeadlockInfo) -> Result<String> {
    Ok(std::fs::read_to_string(autoexec_path(info)?).unwrap_or_default())
}

pub fn write_autoexec(info: &DeadlockInfo, text: &str) -> Result<()> {
    let path = autoexec_path(info)?;
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| Error::io(p, e))?;
    }
    std::fs::write(&path, text).map_err(|e| Error::io(&path, e))
}

/* -------------------------------- launch --------------------------------- */

/// The arguments a launch passes: `+exec autoexec` when there is one, then
/// the player's own launch options.
pub fn launch_args(info: &DeadlockInfo, options: &str) -> Vec<String> {
    let mut args = Vec::new();
    let custom = split_args(options);
    let execs_autoexec = custom.windows(2).any(|w| w[0] == "+exec" && w[1].starts_with("autoexec"));
    if info.has_autoexec && !execs_autoexec {
        args.push("+exec".to_string());
        args.push("autoexec".to_string());
    }
    args.extend(custom);
    args
}

/// Split launch options like a shell would, keeping quoted values together.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for ch in s.chars() {
        match ch {
            '"' => quote = !quote,
            c if c.is_whitespace() && !quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Launch through Steam with `args` (Steam has to own the launch for
/// Deadlock's anti-cheat and matchmaking). Returns 0: the game's pid isn't
/// known, so Now Playing watches for `deadlock.exe` instead.
pub fn launch(args: &[String]) -> Result<u32> {
    let exe = steam::steam_root().map(|r| r.join(if cfg!(windows) { "steam.exe" } else { "steam.sh" }));
    if let Some(exe) = exe.filter(|e| e.exists()) {
        let mut all = vec!["-applaunch".to_string(), APP_ID.to_string()];
        all.extend(args.iter().cloned());
        crate::games::launch_detached(&exe, &all, None, &[])?;
        return Ok(0);
    }
    Err(Error::other("Steam wasn't found; Deadlock needs Steam to launch"))
}

/* ------------------------------ mod library ------------------------------ */

/// One VPK inside a library mod.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VpkFile {
    /// File name inside the mod's folder.
    pub name: String,
    /// Path inside the downloaded archive, to tell variants apart.
    pub label: String,
    pub size: u64,
    pub enabled: bool,
}

/// A mod in the launcher's library.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryMod {
    /// `gb-<GameBanana id>` or `local-<n>`.
    pub id: String,
    pub gamebanana_id: Option<u64>,
    pub name: String,
    pub author: String,
    pub image: Option<String>,
    pub category: Option<String>,
    /// The hero GameBanana files it under.
    pub hero: Option<String>,
    /// A hero the player picked by hand; wins over `hero`.
    #[serde(default)]
    pub hero_override: Option<String>,
    #[serde(default)]
    pub nsfw: bool,
    pub enabled: bool,
    pub files: Vec<VpkFile>,
    /// GameBanana's updated stamp when this copy was downloaded (unix s).
    pub updated: i64,
    pub installed_at: i64,
    #[serde(default)]
    pub file_id: Option<u64>,
    /// Newer upload seen on GameBanana (set by an update check).
    #[serde(default)]
    pub update_available: bool,
}

impl LibraryMod {
    pub fn effective_hero(&self) -> Option<&str> {
        self.hero_override.as_deref().or(self.hero.as_deref())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryFile {
    #[serde(default)]
    mods: Vec<LibraryMod>,
    #[serde(default)]
    launch_options: String,
}

/// What a sync left in `addons/`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReport {
    /// VPKs the launcher placed.
    pub placed: u32,
    /// `pakNN_dir.vpk` files the launcher didn't place (added by hand or by
    /// another mod manager).
    pub unmanaged: Vec<String>,
}

/// The player's Deadlock mods, stored under `<data>/deadlock`.
pub struct ModLibrary {
    root: PathBuf,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `pak07_dir.vpk` → 7.
pub fn pak_slot(name: &str) -> Option<u32> {
    let n = name.to_ascii_lowercase();
    let digits = n.strip_prefix("pak")?.strip_suffix("_dir.vpk")?;
    (digits.len() == 2).then(|| digits.parse().ok()).flatten()
}

impl ModLibrary {
    pub fn new(data_dir: &Path) -> Self {
        Self { root: data_dir.join("deadlock") }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn mods_dir(&self) -> PathBuf {
        self.root.join("mods")
    }

    fn file(&self) -> PathBuf {
        self.root.join("library.json")
    }

    fn load(&self) -> LibraryFile {
        std::fs::read_to_string(self.file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self, lib: &LibraryFile) -> Result<()> {
        std::fs::create_dir_all(&self.root).map_err(|e| Error::io(&self.root, e))?;
        let path = self.file();
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(lib)?;
        std::fs::write(&tmp, json).map_err(|e| Error::io(&tmp, e))?;
        std::fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))
    }

    /// Mods in priority order (first wins conflicts).
    pub fn mods(&self) -> Vec<LibraryMod> {
        self.load().mods
    }

    pub fn launch_options(&self) -> String {
        self.load().launch_options
    }

    pub fn set_launch_options(&self, options: &str) -> Result<()> {
        let mut lib = self.load();
        lib.launch_options = options.trim().to_string();
        self.save(&lib)
    }

    fn edit<T>(&self, id: &str, f: impl FnOnce(&mut LibraryMod) -> T) -> Result<T> {
        let mut lib = self.load();
        let m = lib
            .mods
            .iter_mut()
            .find(|m| m.id == id)
            .ok_or_else(|| Error::other("That mod isn't in your library"))?;
        let out = f(m);
        self.save(&lib)?;
        Ok(out)
    }

    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.edit(id, |m| m.enabled = enabled)
    }

    pub fn set_file_enabled(&self, id: &str, file: &str, enabled: bool) -> Result<()> {
        self.edit(id, |m| {
            for f in m.files.iter_mut().filter(|f| f.name == file) {
                f.enabled = enabled;
            }
        })
    }

    pub fn set_hero_override(&self, id: &str, hero: Option<String>) -> Result<()> {
        let hero = hero.filter(|h| !h.trim().is_empty());
        self.edit(id, |m| m.hero_override = hero)
    }

    /// Make `id` the one skin enabled for `hero` (None = the default look):
    /// every other skin for that hero is switched off.
    pub fn set_active_skin(&self, hero: &str, id: Option<&str>) -> Result<()> {
        let mut lib = self.load();
        for m in lib.mods.iter_mut().filter(|m| m.effective_hero() == Some(hero)) {
            m.enabled = Some(m.id.as_str()) == id;
        }
        self.save(&lib)
    }

    /// Move a mod one place up (`-1`) or down (`1`) in priority.
    pub fn reorder(&self, id: &str, delta: i32) -> Result<()> {
        let mut lib = self.load();
        let Some(i) = lib.mods.iter().position(|m| m.id == id) else { return Ok(()) };
        let j = (i as i64 + delta as i64).clamp(0, lib.mods.len() as i64 - 1) as usize;
        let m = lib.mods.remove(i);
        lib.mods.insert(j, m);
        self.save(&lib)
    }

    pub fn remove(&self, id: &str) -> Result<()> {
        let mut lib = self.load();
        lib.mods.retain(|m| m.id != id);
        self.save(&lib)?;
        let dir = self.mods_dir().join(id);
        if dir.exists() {
            std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        }
        Ok(())
    }

    pub fn mark_updates(&self, ids: &[String]) -> Result<()> {
        let mut lib = self.load();
        for m in lib.mods.iter_mut() {
            m.update_available = ids.contains(&m.id);
        }
        self.save(&lib)
    }

    /// Add (or replace, keeping its place and settings) a mod whose VPKs were
    /// extracted into `staging`. Takes ownership of the staged files.
    pub fn add_from_staging(&self, mut entry: LibraryMod, staging: &Path) -> Result<LibraryMod> {
        let vpks = collect_vpks(staging)?;
        if vpks.is_empty() {
            return Err(Error::other("That download has no .vpk files in it, so it isn't a Deadlock mod the launcher can install"));
        }
        let dir = self.mods_dir().join(&entry.id);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
        let mut files = Vec::new();
        for (i, (path, label)) in vpks.iter().enumerate() {
            let name = format!("{:02}_{}", i + 1, file_name(path));
            let dest = dir.join(&name);
            move_file(path, &dest)?;
            files.push(VpkFile {
                size: std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0),
                name,
                label: label.clone(),
                // A pack of variants: only the first starts switched on.
                enabled: i == 0 || !looks_like_variants(&vpks),
            });
        }

        let mut lib = self.load();
        let previous = lib.mods.iter().position(|m| m.id == entry.id);
        if let Some(i) = previous {
            let old = &lib.mods[i];
            entry.enabled = old.enabled;
            entry.hero_override = old.hero_override.clone();
            // Keep variant choices when the same files are still there.
            for f in files.iter_mut() {
                if let Some(o) = old.files.iter().find(|o| o.label == f.label) {
                    f.enabled = o.enabled;
                }
            }
        }
        entry.files = files;
        entry.installed_at = now();
        entry.update_available = false;
        match previous {
            Some(i) => lib.mods[i] = entry.clone(),
            None => lib.mods.push(entry.clone()),
        }
        // A new skin replaces whatever that hero wore before.
        if previous.is_none() && entry.enabled {
            if let Some(hero) = entry.effective_hero().map(str::to_string) {
                for m in lib.mods.iter_mut() {
                    if m.id != entry.id && m.effective_hero() == Some(hero.as_str()) {
                        m.enabled = false;
                    }
                }
            }
        }
        self.save(&lib)?;
        Ok(entry)
    }

    /// Place every enabled VPK into `addons_dir` as `pakNN_dir.vpk`, in
    /// library order, around any VPKs the launcher didn't put there.
    pub fn sync(&self, addons_dir: &Path) -> Result<SyncReport> {
        std::fs::create_dir_all(addons_dir).map_err(|e| Error::io(addons_dir, e))?;
        let managed_path = addons_dir.join(MANAGED_FILE);
        let managed: Vec<String> = std::fs::read_to_string(&managed_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        for name in &managed {
            let p = addons_dir.join(name);
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| Error::io(&p, e))?;
            }
        }

        let mut unmanaged: Vec<String> = std::fs::read_dir(addons_dir)
            .map_err(|e| Error::io(addons_dir, e))?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| pak_slot(n).is_some())
            .collect();
        unmanaged.sort();
        let taken: Vec<u32> = unmanaged.iter().filter_map(|n| pak_slot(n)).collect();

        let lib = self.load();
        let wanted: Vec<PathBuf> = lib
            .mods
            .iter()
            .filter(|m| m.enabled)
            .flat_map(|m| {
                m.files
                    .iter()
                    .filter(|f| f.enabled)
                    .map(move |f| self.mods_dir().join(&m.id).join(&f.name))
            })
            .filter(|p| p.exists())
            .collect();
        let free: Vec<u32> = (1..=MAX_SLOTS).filter(|n| !taken.contains(n)).collect();
        if wanted.len() > free.len() {
            return Err(Error::other(format!(
                "Deadlock loads at most {MAX_SLOTS} addon files and {} are enabled; switch some mods off",
                wanted.len() + taken.len()
            )));
        }

        let mut placed = Vec::new();
        for (src, slot) in wanted.iter().zip(free) {
            let name = format!("pak{slot:02}_dir.vpk");
            let dest = addons_dir.join(&name);
            if std::fs::hard_link(src, &dest).is_err() {
                std::fs::copy(src, &dest).map_err(|e| Error::io(&dest, e))?;
            }
            placed.push(name);
        }
        std::fs::write(&managed_path, serde_json::to_vec(&placed)?).map_err(|e| Error::io(&managed_path, e))?;
        Ok(SyncReport { placed: placed.len() as u32, unmanaged })
    }

    /// Move VPKs the launcher didn't place into the library as local mods, so
    /// they can be switched on and off like the rest.
    pub fn adopt_unmanaged(&self, addons_dir: &Path) -> Result<u32> {
        let mut adopted = 0;
        for name in self.unmanaged(addons_dir) {
            let staging = self.root.join(".staging").join(format!("adopt-{}", now()));
            let _ = std::fs::remove_dir_all(&staging);
            std::fs::create_dir_all(&staging).map_err(|e| Error::io(&staging, e))?;
            move_file(&addons_dir.join(&name), &staging.join(&name))?;
            let id = format!("local-{}-{}", now(), adopted);
            self.add_from_staging(
                LibraryMod {
                    id,
                    gamebanana_id: None,
                    name: format!("Local mod ({name})"),
                    author: "Added by hand".into(),
                    image: None,
                    category: None,
                    hero: None,
                    hero_override: None,
                    nsfw: false,
                    enabled: true,
                    files: vec![],
                    updated: 0,
                    installed_at: 0,
                    file_id: None,
                    update_available: false,
                },
                &staging,
            )?;
            let _ = std::fs::remove_dir_all(&staging);
            adopted += 1;
        }
        Ok(adopted)
    }

    /// `pakNN_dir.vpk` files in `addons_dir` the launcher didn't place.
    pub fn unmanaged(&self, addons_dir: &Path) -> Vec<String> {
        let managed: Vec<String> = std::fs::read_to_string(addons_dir.join(MANAGED_FILE))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut names: Vec<String> = std::fs::read_dir(addons_dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| pak_slot(n).is_some() && !managed.contains(n))
            .collect();
        names.sort();
        names
    }

    /// Remove every VPK the launcher placed in `addons/` (mods stay in the
    /// library).
    pub fn clear_addons(&self, addons_dir: &Path) -> Result<u32> {
        let managed_path = addons_dir.join(MANAGED_FILE);
        let managed: Vec<String> = std::fs::read_to_string(&managed_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut n = 0;
        for name in &managed {
            if std::fs::remove_file(addons_dir.join(name)).is_ok() {
                n += 1;
            }
        }
        let _ = std::fs::remove_file(&managed_path);
        Ok(n)
    }
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "mod.vpk".into())
}

fn move_file(from: &Path, to: &Path) -> Result<()> {
    if std::fs::rename(from, to).is_err() {
        std::fs::copy(from, to).map_err(|e| Error::io(to, e))?;
        let _ = std::fs::remove_file(from);
    }
    Ok(())
}

/// Several VPKs that each sit in their own folder are usually alternative
/// versions of one mod ("Blue", "Red"…), not parts meant together.
fn looks_like_variants(vpks: &[(PathBuf, String)]) -> bool {
    let mut parents: Vec<&str> = vpks.iter().map(|(_, l)| l.rsplit_once('/').map_or("", |(d, _)| d)).collect();
    parents.sort();
    parents.dedup();
    vpks.len() > 1 && parents.len() == vpks.len()
}

/// Every loadable VPK under `root`, with its path relative to `root`.
/// Multi-part VPKs (`_000.vpk` chunks) can't be renamed into addon slots, so
/// they are refused.
fn collect_vpks(root: &Path) -> Result<Vec<(PathBuf, String)>> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let name = file_name(&p).to_ascii_lowercase();
            if !name.ends_with(".vpk") {
                continue;
            }
            let stem = &name[..name.len() - 4];
            if stem.len() > 4 && stem.as_bytes()[stem.len() - 4] == b'_' && stem[stem.len() - 3..].bytes().all(|b| b.is_ascii_digit()) {
                return Err(Error::other("This mod is a multi-part VPK, which can't be installed as an addon"));
            }
            let label = p
                .strip_prefix(root)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| file_name(&p));
            out.push((p, label));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// Unpack a downloaded mod into `dest`: `.zip`, `.7z` and `.rar` archives,
/// or a bare `.vpk`. Blocking.
pub fn unpack(download: &Path, original_name: &str, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest).map_err(|e| Error::io(dest, e))?;
    let lower = original_name.to_ascii_lowercase();
    if lower.ends_with(".vpk") {
        return move_file(download, &dest.join(original_name));
    }
    if lower.ends_with(".rar") {
        return unrar_into(download, dest);
    }
    // zip and 7z by extension; the extractor picks by the file's extension.
    let ext = if lower.ends_with(".7z") { "7z" } else { "zip" };
    let named = download.with_extension(ext);
    if named != download {
        std::fs::rename(download, &named).map_err(|e| Error::io(&named, e))?;
    }
    crate::games::install::extract_archive_into(&named, dest)
}

fn unrar_into(archive: &Path, dest: &Path) -> Result<()> {
    let mut a = unrar::Archive::new(archive)
        .open_for_processing()
        .map_err(|e| Error::other(format!("couldn't open the .rar: {e}")))?;
    while let Some(header) = a.read_header().map_err(|e| Error::other(format!("couldn't read the .rar: {e}")))? {
        let entry = header.entry();
        let rel = entry.filename.to_string_lossy().replace('\\', "/");
        // Never let an archive write outside its folder.
        let safe = !rel.split('/').any(|c| c == "..") && !rel.starts_with('/') && !rel.contains(':');
        a = if entry.is_file() && safe {
            let out = dest.join(&rel);
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p).map_err(|e| Error::io(p, e))?;
            }
            header.extract_to(&out).map_err(|e| Error::other(format!("couldn't extract the .rar: {e}")))?
        } else {
            header.skip().map_err(|e| Error::other(format!("couldn't read the .rar: {e}")))?
        };
    }
    Ok(())
}

/// Download a GameBanana mod (its newest file, or `file_id`) and add it to
/// the library.
pub async fn install_from_gamebanana(
    lib: &ModLibrary,
    gamebanana_id: u64,
    file_id: Option<u64>,
    reporter: &crate::progress::SharedReporter,
) -> Result<LibraryMod> {
    let detail = gamebanana::detail(gamebanana_id).await?;
    let file = match file_id {
        Some(fid) => detail.files.iter().find(|f| f.id == fid),
        None => detail.files.iter().max_by_key(|f| f.added),
    }
    .cloned()
    .ok_or_else(|| Error::other("That mod has no files to download"))?;

    let staging = lib.root().join(".staging").join(format!("gb-{gamebanana_id}-{}", now()));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| Error::io(&staging, e))?;
    let archive = staging.join("download.bin");
    crate::download::download_all(
        vec![crate::download::Download::new(file.url.clone(), archive.clone())],
        1,
        reporter.clone(),
    )
    .await?;

    let extracted = staging.join("files");
    let (a, x, n) = (archive.clone(), extracted.clone(), file.name.clone());
    tokio::task::spawn_blocking(move || unpack(&a, &n, &x))
        .await
        .map_err(|e| Error::other(format!("extract task panicked: {e}")))??;

    let s = &detail.summary;
    let entry = LibraryMod {
        id: format!("gb-{gamebanana_id}"),
        gamebanana_id: Some(gamebanana_id),
        name: s.name.clone(),
        author: s.author.clone(),
        image: s.image.clone(),
        category: s.category.clone(),
        hero: s.hero.clone(),
        hero_override: None,
        nsfw: s.nsfw,
        enabled: true,
        files: vec![],
        updated: s.updated,
        installed_at: 0,
        file_id: Some(file.id),
        update_available: false,
    };
    let out = lib.add_from_staging(entry, &extracted);
    let _ = std::fs::remove_dir_all(&staging);
    out
}

/// Library mods with a newer upload on GameBanana.
pub async fn check_updates(lib: &ModLibrary) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for m in lib.mods() {
        let Some(gid) = m.gamebanana_id else { continue };
        if let Ok(d) = gamebanana::detail(gid).await {
            if d.summary.updated > m.updated {
                out.push(m.id.clone());
            }
        }
    }
    lib.mark_updates(&out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const GAMEINFO: &str = "\"GameInfo\"\r\n{\r\n\tFileSystem\r\n\t{\r\n\t\tSearchPaths\r\n\t\t{\r\n\t\t\tGame_Language\t\tcitadel_*LANGUAGE*\r\n\t\t\tMod\t\t\t\t\tcitadel\r\n\t\t\tWrite\t\t\t\tcitadel\r\n\t\t\tGame\t\t\t\tcitadel\r\n\t\t}\r\n\t}\r\n}\r\n";

    #[test]
    fn gameinfo_round_trip() {
        assert!(!has_addons_path(GAMEINFO));
        let on = add_addons_path(GAMEINFO);
        assert!(has_addons_path(&on));
        assert!(on.contains("citadel_*LANGUAGE*\r\n\t\t\tGame\t\t\t\tcitadel/addons\r\n\t\t\tMod"));
        // Idempotent.
        assert_eq!(add_addons_path(&on), on);
        assert_eq!(remove_addons_path(&on), GAMEINFO);
    }

    #[test]
    fn gameinfo_without_search_paths_is_left_alone() {
        assert_eq!(add_addons_path("\"GameInfo\"\n{\n}\n"), "\"GameInfo\"\n{\n}\n");
    }

    #[test]
    fn launch_args_add_autoexec_once() {
        let info = DeadlockInfo { has_autoexec: true, ..Default::default() };
        assert_eq!(launch_args(&info, "-novid  +fps_max 240"), ["+exec", "autoexec", "-novid", "+fps_max", "240"]);
        assert_eq!(launch_args(&info, "+exec autoexec"), ["+exec", "autoexec"]);
        assert_eq!(split_args("-x \"a b\""), ["-x", "a b"]);
    }

    #[test]
    fn slots() {
        assert_eq!(pak_slot("pak07_dir.vpk"), Some(7));
        assert_eq!(pak_slot("PAK99_DIR.VPK"), Some(99));
        assert_eq!(pak_slot("pak7_dir.vpk"), None);
        assert_eq!(pak_slot("pak01_000.vpk"), None);
    }

    fn entry(id: &str, hero: Option<&str>) -> LibraryMod {
        LibraryMod {
            id: id.into(),
            gamebanana_id: None,
            name: id.into(),
            author: String::new(),
            image: None,
            category: None,
            hero: hero.map(String::from),
            hero_override: None,
            nsfw: false,
            enabled: true,
            files: vec![],
            updated: 0,
            installed_at: 0,
            file_id: None,
            update_available: false,
        }
    }

    #[test]
    fn library_syncs_around_unmanaged_files() {
        let base = std::env::temp_dir().join(format!("kawma-dl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let lib = ModLibrary::new(&base.join("data"));
        let addons = base.join("addons");
        std::fs::create_dir_all(&addons).unwrap();
        // The player's own file in slot 1.
        std::fs::write(addons.join("pak01_dir.vpk"), "mine").unwrap();

        for (id, hero) in [("a", Some("Lash")), ("b", Some("Lash")), ("c", None)] {
            let staging = base.join(format!("stage-{id}"));
            std::fs::create_dir_all(staging.join("sub")).unwrap();
            std::fs::write(staging.join("sub/pak01_dir.vpk"), id).unwrap();
            lib.add_from_staging(entry(id, hero), &staging).unwrap();
        }
        // Installing "b" switched off the other Lash skin.
        let mods = lib.mods();
        assert!(!mods[0].enabled && mods[1].enabled && mods[2].enabled);

        let r = lib.sync(&addons).unwrap();
        assert_eq!(r.placed, 2);
        assert_eq!(r.unmanaged, ["pak01_dir.vpk"]);
        assert_eq!(std::fs::read_to_string(addons.join("pak01_dir.vpk")).unwrap(), "mine");
        assert_eq!(std::fs::read_to_string(addons.join("pak02_dir.vpk")).unwrap(), "b");
        assert_eq!(std::fs::read_to_string(addons.join("pak03_dir.vpk")).unwrap(), "c");

        lib.set_active_skin("Lash", Some("a")).unwrap();
        lib.reorder("c", -2).unwrap();
        lib.sync(&addons).unwrap();
        assert_eq!(std::fs::read_to_string(addons.join("pak02_dir.vpk")).unwrap(), "c");
        assert_eq!(std::fs::read_to_string(addons.join("pak03_dir.vpk")).unwrap(), "a");
        assert!(!addons.join("pak04_dir.vpk").exists());

        assert_eq!(lib.adopt_unmanaged(&addons).unwrap(), 1);
        assert!(!addons.join("pak01_dir.vpk").exists());
        assert_eq!(lib.mods().len(), 4);

        assert_eq!(lib.clear_addons(&addons).unwrap(), 2);
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn variant_packs_start_with_one_file_on() {
        let base = std::env::temp_dir().join(format!("kawma-dlv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let lib = ModLibrary::new(&base.join("data"));
        let staging = base.join("stage");
        for v in ["Blue", "Red"] {
            std::fs::create_dir_all(staging.join(v)).unwrap();
            std::fs::write(staging.join(v).join("pak01_dir.vpk"), v).unwrap();
        }
        let m = lib.add_from_staging(entry("v", None), &staging).unwrap();
        assert_eq!(m.files.len(), 2);
        assert!(m.files[0].enabled && !m.files[1].enabled);
        let _ = std::fs::remove_dir_all(&base);
    }
}
