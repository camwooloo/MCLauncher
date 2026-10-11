//! Deadlock mod manager commands: the GameBanana store, the mod library,
//! hero skins, launching with or without mods, and the Extras page.
//!
//! Every library change is applied to the game's `addons/` folder straight
//! away (when Deadlock is installed), like Deadlock Mod Manager does.

use launcher_core::games::deadlock::{self, AutoFix, DeadlockInfo, LibraryMod, ModLibrary};
use launcher_core::games::gamebanana::{self, Category, ModDetail, Sort, StoreMod, StorePage};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn library(state: &State<'_, AppState>) -> ModLibrary {
    ModLibrary::new(&state.paths.data_dir)
}

/// The library as the UI shows it.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryState {
    pub mods: Vec<LibraryMod>,
    /// Addon files the launcher didn't place.
    pub unmanaged: Vec<String>,
    /// Addon slots in use (of 99).
    pub slots_used: u32,
    /// Set when applying the library to the game failed (e.g. the game is
    /// running and holds the files open).
    pub sync_error: Option<String>,
    /// Mods the launcher switched off on its own after a bad start.
    pub auto_fix: Option<AutoFix>,
}

fn snapshot(lib: &ModLibrary, info: &DeadlockInfo, sync_error: Option<String>) -> LibraryState {
    let mods = lib.mods();
    let unmanaged = info.addons_dir.as_deref().map(|a| lib.unmanaged(a)).unwrap_or_default();
    let placed: usize = mods
        .iter()
        .filter(|m| m.enabled)
        .map(|m| m.files.iter().filter(|f| f.enabled).count())
        .sum();
    LibraryState {
        slots_used: (placed + unmanaged.len()) as u32,
        mods,
        unmanaged,
        sync_error,
        auto_fix: lib.auto_fix(),
    }
}

/// Apply the library to `addons/` and report the result.
fn apply(lib: &ModLibrary) -> LibraryState {
    let info = deadlock::detect();
    let error = match info.addons_dir.as_deref() {
        Some(addons) => lib.sync(addons).err().map(|e| {
            let msg = e.to_string();
            if msg.contains("os error 32") || msg.to_lowercase().contains("being used") {
                "Close Deadlock to apply mod changes".to_string()
            } else {
                msg
            }
        }),
        None => None,
    };
    snapshot(lib, &info, error)
}

/* --------------------------------- store --------------------------------- */

#[tauri::command]
pub async fn deadlock_store(sort: Sort, search: String, category: Option<u64>, page: u32) -> Result<StorePage, String> {
    gamebanana::browse(sort, &search, category, page, 30).await.map_err(err)
}

#[tauri::command]
pub async fn deadlock_trending() -> Result<Vec<StoreMod>, String> {
    gamebanana::trending().await.map_err(err)
}

#[tauri::command]
pub async fn deadlock_categories() -> Result<Vec<Category>, String> {
    gamebanana::categories().await.map_err(err)
}

#[tauri::command]
pub async fn deadlock_mod_detail(id: u64) -> Result<ModDetail, String> {
    gamebanana::detail(id).await.map_err(err)
}

/// Download a GameBanana mod into the library and switch it on. Progress goes
/// out as `deadlock-progress` events (`{ id, fraction }`).
#[tauri::command]
pub async fn deadlock_install(
    app: AppHandle,
    state: State<'_, AppState>,
    id: u64,
    file_id: Option<u64>,
) -> Result<LibraryState, String> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let reporter = Arc::new(crate::progress::EventReporter::default());
    let done = Arc::new(AtomicBool::new(false));
    {
        let (rep, done, app) = (reporter.clone(), done.clone(), app.clone());
        tokio::spawn(async move {
            while !done.load(Ordering::Relaxed) {
                let snap = rep.snapshot();
                let _ = app.emit("deadlock-progress", serde_json::json!({ "id": id, "fraction": snap.fraction }));
                tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            }
        });
    }
    let lib = library(&state);
    let shared: launcher_core::progress::SharedReporter = reporter.clone();
    let result = deadlock::install_from_gamebanana(&lib, id, file_id, &shared).await;
    done.store(true, Ordering::Relaxed);
    result.map_err(err)?;
    Ok(apply(&lib))
}

/* -------------------------------- library -------------------------------- */

#[tauri::command]
pub fn deadlock_library(state: State<'_, AppState>) -> LibraryState {
    snapshot(&library(&state), &deadlock::detect(), None)
}

#[tauri::command]
pub fn deadlock_set_enabled(state: State<'_, AppState>, id: String, enabled: bool) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.set_enabled(&id, enabled).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub fn deadlock_set_file_enabled(
    state: State<'_, AppState>,
    id: String,
    file: String,
    enabled: bool,
) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.set_file_enabled(&id, &file, enabled).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub fn deadlock_set_hero(state: State<'_, AppState>, id: String, hero: Option<String>) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.set_hero_override(&id, hero).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub fn deadlock_set_active_skin(state: State<'_, AppState>, hero: String, id: Option<String>) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.set_active_skin(&hero, id.as_deref()).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub fn deadlock_reorder(state: State<'_, AppState>, id: String, delta: i32) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.reorder(&id, delta).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub fn deadlock_remove(state: State<'_, AppState>, id: String) -> Result<LibraryState, String> {
    let lib = library(&state);
    // Take its files out of addons first, so Windows isn't asked to delete
    // a file that is still linked there.
    lib.set_enabled(&id, false).map_err(err)?;
    let _ = apply(&lib);
    lib.remove(&id).map_err(err)?;
    Ok(apply(&lib))
}

#[tauri::command]
pub async fn deadlock_check_updates(state: State<'_, AppState>) -> Result<LibraryState, String> {
    let lib = library(&state);
    deadlock::check_updates(&lib).await.map_err(err)?;
    Ok(snapshot(&lib, &deadlock::detect(), None))
}

/// Bring hand-placed addon files into the library.
#[tauri::command]
pub fn deadlock_adopt(state: State<'_, AppState>) -> Result<LibraryState, String> {
    let lib = library(&state);
    let info = deadlock::detect();
    let addons = info.addons_dir.ok_or("Deadlock is not installed")?;
    lib.adopt_unmanaged(&addons).map_err(err)?;
    Ok(apply(&lib))
}

/* ---------------------------- launch & extras ---------------------------- */

/// Launch Deadlock with the library's mods, or vanilla. Vanilla only takes
/// the addons path out of `gameinfo.gi`; every mod stays installed.
///
/// A modded launch also writes the `Mod`/`Write` search paths the game needs
/// to boot with addons first (see `deadlock::add_addons_path`). The launch is
/// then watched: if Deadlock dies on start, mods that override game configs
/// (or failing that, the UI) are switched off and the home page says which
/// (`deadlock-library` carries the new state).
#[tauri::command]
pub fn deadlock_launch(app: AppHandle, state: State<'_, AppState>, modded: bool) -> Result<u32, String> {
    let info = deadlock::detect();
    if !info.installed {
        return Err("Deadlock is not installed".into());
    }
    let lib = library(&state);
    let build = deadlock::game_build();
    if modded {
        if lib.last_good_build().is_some_and(|good| Some(good) != build) {
            // The gameinfo.gi backup belongs to the old build.
            let _ = std::fs::remove_file(lib.root().join("backup").join("gameinfo.gi.original"));
        }
        if let Some(addons) = info.addons_dir.as_deref() {
            lib.sync(addons).map_err(err)?;
        }
    }
    deadlock::set_mods_active(&info, modded, &lib.root().join("backup")).map_err(err)?;
    let info = deadlock::detect();
    let started = std::time::SystemTime::now();
    let pid = deadlock::launch(&deadlock::launch_args(&info, &lib.launch_options())).map_err(err)?;
    if modded {
        watch_start(app.clone(), state.paths.data_dir.clone(), build, started);
    }
    crate::stats::record_session(&state.paths.data_dir, "game:deadlock", "Deadlock", "deadlock", None, pid);
    crate::updates::game_launched("deadlock", pid);
    crate::discord::set_playing("Playing Deadlock", "via Kawma Launcher");
    let detail = Some(if modded { "With mods" } else { "Without mods" }.to_string());
    crate::activity::game_started(&app, "deadlock", "Deadlock", detail, pid);
    Ok(pid)
}

/// How long Deadlock has to stay up for a modded start to count as good.
const START_OK: std::time::Duration = std::time::Duration::from_secs(10 * 60);
/// An exit sooner than this after the window appears is a failed start even
/// without a crash dump (the fatal error box closes the game on OK).
const QUICK_EXIT: std::time::Duration = std::time::Duration::from_secs(60);

fn deadlock_running(sys: &mut sysinfo::System) -> bool {
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| p.name().eq_ignore_ascii_case("deadlock.exe"))
}

/// Watch a modded launch: a clean run marks this build as good; a death on
/// start switches off the likely culprits.
fn watch_start(app: AppHandle, data_dir: std::path::PathBuf, build: Option<String>, started: std::time::SystemTime) {
    use std::time::{Duration, Instant};
    tauri::async_runtime::spawn(async move {
        let mut sys = sysinfo::System::new();
        // Steam can take a while (updates, the anti-cheat) to start the game.
        let wait_until = Instant::now() + Duration::from_secs(180);
        while !deadlock_running(&mut sys) {
            if Instant::now() > wait_until {
                return;
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let seen = Instant::now();
        while deadlock_running(&mut sys) {
            if seen.elapsed() > START_OK {
                let _ = ModLibrary::new(&data_dir).set_last_good_build(build);
                return;
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        let info = deadlock::detect();
        let crashed = seen.elapsed() < QUICK_EXIT || deadlock::crash_dumps_since(&info, started) > 0;
        if !crashed {
            return;
        }
        let lib = ModLibrary::new(&data_dir);
        match lib.repair(info.addons_dir.as_deref(), true) {
            Ok(Some(_)) => {
                let _ = app.emit("deadlock-library", apply(&lib));
            }
            Ok(None) => {}
            Err(e) => eprintln!("[deadlock] auto fix: {e}"),
        }
    });
}

#[tauri::command]
pub fn deadlock_dismiss_fix(state: State<'_, AppState>) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.dismiss_fix().map_err(err)?;
    Ok(snapshot(&lib, &deadlock::detect(), None))
}

/// Switch back on what the last auto fix turned off.
#[tauri::command]
pub fn deadlock_undo_fix(state: State<'_, AppState>) -> Result<LibraryState, String> {
    let lib = library(&state);
    lib.undo_fix().map_err(err)?;
    // The player vouched for these mods on this build: don't check again.
    lib.set_last_good_build(deadlock::game_build()).map_err(err)?;
    Ok(apply(&lib))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extras {
    pub autoexec: String,
    pub autoexec_path: Option<String>,
    pub launch_options: String,
    /// The full argument line a launch uses.
    pub effective_args: String,
    pub mods_active: bool,
    pub gameinfo_found: bool,
    pub backup_exists: bool,
    pub install_dir: Option<String>,
    pub addons_dir: Option<String>,
    pub library_dir: String,
}

fn extras_for(state: &State<'_, AppState>) -> Extras {
    let info = deadlock::detect();
    let lib = library(state);
    let options = lib.launch_options();
    let path = |p: &Option<std::path::PathBuf>| p.as_ref().map(|p| p.display().to_string());
    Extras {
        autoexec: deadlock::read_autoexec(&info).unwrap_or_default(),
        autoexec_path: deadlock::autoexec_path(&info).ok().map(|p| p.display().to_string()),
        effective_args: deadlock::launch_args(&info, &options).join(" "),
        launch_options: options,
        mods_active: info.mods_active,
        gameinfo_found: info.gameinfo_found,
        backup_exists: lib.root().join("backup").join("gameinfo.gi.original").exists(),
        install_dir: path(&info.install_dir),
        addons_dir: path(&info.addons_dir),
        library_dir: lib.mods_dir().display().to_string(),
    }
}

#[tauri::command]
pub fn deadlock_extras(state: State<'_, AppState>) -> Extras {
    extras_for(&state)
}

#[tauri::command]
pub fn deadlock_save_autoexec(state: State<'_, AppState>, text: String) -> Result<Extras, String> {
    deadlock::write_autoexec(&deadlock::detect(), &text).map_err(err)?;
    Ok(extras_for(&state))
}

#[tauri::command]
pub fn deadlock_set_launch_options(state: State<'_, AppState>, options: String) -> Result<Extras, String> {
    library(&state).set_launch_options(&options).map_err(err)?;
    Ok(extras_for(&state))
}

#[tauri::command]
pub fn deadlock_reset_gameinfo(state: State<'_, AppState>) -> Result<String, String> {
    let lib = library(&state);
    deadlock::reset_gameinfo(&deadlock::detect(), &lib.root().join("backup")).map_err(err)
}

#[tauri::command]
pub fn deadlock_set_mods_active(state: State<'_, AppState>, active: bool) -> Result<Extras, String> {
    let lib = library(&state);
    deadlock::set_mods_active(&deadlock::detect(), active, &lib.root().join("backup")).map_err(err)?;
    Ok(extras_for(&state))
}

/// Take every launcher-placed file out of `addons/` (mods stay in the library
/// and come back on the next change or modded launch).
#[tauri::command]
pub fn deadlock_clear_addons(state: State<'_, AppState>) -> Result<String, String> {
    let info = deadlock::detect();
    let addons = info.addons_dir.ok_or("Deadlock is not installed")?;
    let n = library(&state).clear_addons(&addons).map_err(err)?;
    Ok(format!("Removed {n} addon file{} from the game", if n == 1 { "" } else { "s" }))
}
