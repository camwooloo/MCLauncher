//! Discord Rich Presence — shows "Playing … via Aurora Launcher" with a button
//! linking to the project. Entirely optional: if `secrets::DISCORD_CLIENT_ID`
//! is empty, or Discord isn't running, this is a no-op.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use discord_rich_presence::{
    activity::{Activity, Button},
    DiscordIpc, DiscordIpcClient,
};

use crate::secrets::{DISCORD_CLIENT_ID, GITHUB_URL};

/// Connected client, kept alive so the presence persists. `Mutex::new` is const
/// so this needs no lazy-init crate.
static CLIENT: Mutex<Option<DiscordIpcClient>> = Mutex::new(None);

/// User preference (Settings toggle). Defaults on; updated from settings.
static ENABLED: AtomicBool = AtomicBool::new(true);

/// Apply the user's on/off preference. Turning it off clears any active
/// presence.
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
    if !on {
        clear();
    }
}

/// Clear the current Rich Presence (if connected).
pub fn clear() {
    if let Ok(mut guard) = CLIENT.lock() {
        if let Some(client) = guard.as_mut() {
            let _ = client.clear_activity();
        }
    }
}

/// Set the presence to "Playing <details>" with a sub-line of `state`. Connects
/// lazily on first use. Errors (Discord closed, etc.) are swallowed.
pub fn set_playing(details: &str, state: &str) {
    if DISCORD_CLIENT_ID.is_empty() || !ENABLED.load(Ordering::Relaxed) {
        return;
    }
    let Ok(mut guard) = CLIENT.lock() else { return };
    if guard.is_none() {
        if let Ok(mut c) = DiscordIpcClient::new(DISCORD_CLIENT_ID) {
            if c.connect().is_ok() {
                *guard = Some(c);
            }
        }
    }
    if let Some(client) = guard.as_mut() {
        let activity = Activity::new()
            .details(details)
            .state(state)
            .buttons(vec![Button::new("Get Aurora Launcher", GITHUB_URL)]);
        if client.set_activity(activity).is_err() {
            // Connection dropped (Discord closed/restarted) — reset so the next
            // call reconnects.
            *guard = None;
        }
    }
}
