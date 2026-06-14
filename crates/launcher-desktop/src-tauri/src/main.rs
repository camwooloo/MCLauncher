//! Aurora Launcher — Tauri entry point.
//!
//! The window is frameless (`decorations: false`) so the web UI can draw its
//! own liquid-glass title bar. Commands bridging `launcher-core` are registered
//! here; their implementations live in the `commands` module.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod content;
mod instances;
mod inventory;
mod progress;
mod secrets;
mod settings;
mod state;

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
        .invoke_handler(tauri::generate_handler![
            commands::app_version,
            commands::open_url,
            commands::system_memory_mb,
            commands::paths_info,
            commands::get_settings,
            commands::save_settings,
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
            commands::launch_elden_ring,
            commands::launch_cyberpunk,
            commands::set_elden_ring_password,
            commands::install_game_tool,
            commands::install_skyrim_together,
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
            commands::open_server_console,
            content::modrinth_search,
            content::content_install,
            content::list_installed,
            content::content_remove,
            content::check_updates,
            content::apply_update,
            content::set_skin,
            content::set_skin_from_url,
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
