//! Aurora Launcher — Tauri entry point.
//!
//! The window is frameless (`decorations: false`) so the web UI can draw its
//! own liquid-glass title bar. Commands bridging `launcher-core` are registered
//! here; their implementations live in the `commands` module.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod content;
mod discord;
mod firewall;
mod instances;
mod inventory;
mod nexus;
mod progress;
mod secrets;
mod settings;
mod state;
mod stats;
mod vpn;

use state::AppState;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,launcher_core=info".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState::new())
        .setup(|app| {
            use tauri::Manager;
            // Apply saved startup preferences (Discord RPC + start-minimized).
            let path = app.state::<AppState>().paths.settings_file();
            if let Ok(bytes) = std::fs::read(&path) {
                if let Ok(s) = serde_json::from_slice::<settings::Settings>(&bytes) {
                    discord::set_enabled(s.discord_rpc);
                    if s.start_minimized {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.hide();
                        }
                    }
                }
            }

            // System tray: closing the window hides to tray (so hosted servers
            // keep running); the tray menu shows it again or quits for real.
            use tauri::menu::{Menu, MenuItem};
            use tauri::tray::{TrayIconBuilder, TrayIconEvent};
            let show = MenuItem::with_id(app, "show", "Open Aurora", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit (stops servers)", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .tooltip("Aurora Launcher")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        if let Some(w) = tray.app_handle().get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.unminimize();
                            let _ = w.set_focus();
                        }
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            // Close button hides to tray (keeping servers alive) unless the user
            // turned that off in Settings — then it quits for real.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                use tauri::Manager;
                let close_to_tray = std::fs::read(window.app_handle().state::<AppState>().paths.settings_file())
                    .ok()
                    .and_then(|b| serde_json::from_slice::<settings::Settings>(&b).ok())
                    .map(|s| s.close_to_tray)
                    .unwrap_or(true);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::open_url,
            commands::system_memory_mb,
            commands::paths_info,
            commands::get_settings,
            commands::save_settings,
            commands::set_launch_at_login,
            commands::list_accounts,
            commands::add_offline_account,
            commands::set_active_account,
            commands::remove_account,
            commands::microsoft_login,
            commands::microsoft_login_code,
            commands::minecraft_versions,
            commands::play_minecraft,
            commands::detect_games,
            commands::launch_skyrim,
            commands::skyrim_server_config,
            commands::save_skyrim_server_config,
            commands::start_skyrim_server,
            commands::launch_elden_ring,
            commands::launch_cyberpunk,
            commands::set_elden_ring_password,
            commands::install_game_tool,
            commands::install_skyrim_together,
            commands::install_skyrim_mod,
            commands::nexus_config,
            commands::nexus_set_key,
            commands::skyrim_catalog,
            commands::open_together_page,
            commands::install_address_library,
            commands::open_address_library_page,
            commands::install_seamless_update,
            commands::open_seamless_page,
            commands::open_path,
            commands::list_servers,
            commands::save_server,
            commands::delete_server,
            commands::servers_status,
            commands::server_start,
            commands::server_stop,
            commands::server_command,
            commands::server_log_history,
            commands::open_server_console,
            content::modrinth_search,
            content::content_install,
            content::list_installed,
            content::content_remove,
            content::check_updates,
            content::apply_update,
            content::set_skin,
            content::set_skin_from_url,
            commands::vpn_status,
            commands::vpn_install,
            commands::vpn_login,
            commands::vpn_disconnect,
            commands::vpn_config,
            commands::vpn_set_token,
            commands::vpn_join,
            commands::vpn_share,
            commands::vpn_peers,
            commands::vpn_friend_code,
            commands::repair_aurora_net,
            instances::list_backups,
            instances::create_backup,
            instances::restore_backup,
            instances::delete_backup,
            instances::list_config_files,
            instances::read_config_file,
            instances::write_config_file,
            instances::server_access,
            instances::access_add,
            instances::access_remove,
            instances::export_instance,
            instances::import_mrpack,
            instances::analyze_crash,
            instances::disable_mod,
            commands::check_app_update,
            commands::apply_app_update,
            commands::list_releases,
            commands::host_addresses,
            commands::play_stats,
            instances::list_instances,
            instances::save_instance,
            instances::delete_instance,
            instances::instance_play,
            instances::popular_modpacks,
            instances::create_instance_from_modpack,
            instances::pack_search,
            instances::create_instance_from_pack,
            instances::create_server_from_pack,
            instances::open_instance_folder,
            instances::open_server_folder,
            inventory::list_worlds,
            inventory::list_players,
            inventory::get_inventory,
            inventory::save_inventory,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Aurora Launcher")
        .run(|app, event| {
            // Never leave hosted servers running after the launcher closes.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                commands::kill_all_servers(app);
            }
        });
}
