//! "Share to the internet" tunnels via **bore** (github.com/ekzhang/bore).
//!
//! Host-only, no port forwarding, no accounts: the host runs a bore client that
//! makes an *outbound* connection to the public `bore.pub` relay, which hands
//! back a public `bore.pub:<port>`. Anyone — any Minecraft launcher, any game,
//! no VPN, no signup — connects to that address. Game-agnostic: it just forwards
//! a local port, so it works for Minecraft, Skyrim Together, and anything else.

use std::collections::HashMap;

use serde::Serialize;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

/// The public bore relay everyone can reach.
const RELAY: &str = "bore.pub";

/// One active tunnel: the assigned public port + the task running the forward.
pub struct Tunnel {
    pub port: u16,
    pub task: JoinHandle<()>,
}

/// All active tunnels, keyed by an arbitrary caller id (e.g. a server id).
pub struct Tunnels {
    pub active: Mutex<HashMap<String, Tunnel>>,
}
impl Tunnels {
    pub fn new() -> Self {
        Self { active: Mutex::new(HashMap::new()) }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInfo {
    pub host: String,
    pub port: u16,
}

/// Open a public tunnel for `local_port` and return the public `host:port`.
/// `community` is the share's "Post to my Kawma community" choice (unset =
/// the Settings default); it only applies to hosted Minecraft servers.
#[tauri::command]
pub async fn tunnel_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::state::AppState>,
    key: String,
    local_port: u16,
    community: Option<bool>,
) -> Result<TunnelInfo, String> {
    {
        let map = state.tunnels.active.lock().await;
        if let Some(t) = map.get(&key) {
            return Ok(TunnelInfo { host: RELAY.into(), port: t.port });
        }
    }
    let client = bore_cli::client::Client::new("localhost", local_port, RELAY, 0, None)
        .await
        .map_err(|e| format!("Couldn't open an internet tunnel: {e}"))?;
    let port = client.remote_port();
    let task = tokio::spawn(async move {
        let _ = client.listen().await;
    });
    state.tunnels.active.lock().await.insert(key.clone(), Tunnel { port, task });
    crate::activity::share_started(&app, key, format!("{RELAY}:{port}"), community);
    Ok(TunnelInfo { host: RELAY.into(), port })
}

/// Stop a tunnel (the address stops working immediately).
#[tauri::command]
pub async fn tunnel_stop(
    state: tauri::State<'_, crate::state::AppState>,
    key: String,
) -> Result<(), String> {
    if let Some(t) = state.tunnels.active.lock().await.remove(&key) {
        t.task.abort();
    }
    crate::activity::share_stopped(&key);
    Ok(())
}

/// Currently-active tunnels, so the UI can show which servers are shared.
#[tauri::command]
pub async fn tunnel_status(
    state: tauri::State<'_, crate::state::AppState>,
) -> Result<HashMap<String, TunnelInfo>, String> {
    let map = state.tunnels.active.lock().await;
    Ok(map
        .iter()
        .map(|(k, t)| (k.clone(), TunnelInfo { host: RELAY.into(), port: t.port }))
        .collect())
}
