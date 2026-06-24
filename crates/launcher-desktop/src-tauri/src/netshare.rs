//! Aurora LAN **presence + remote control** (increment 1).
//!
//! Per the user's choice, trust is **auto on the local network** — no pairing.
//! Each launcher:
//!   * broadcasts a small UDP discovery beacon and listens for others → a live
//!     peer list (shown as a top-right pill), and
//!   * runs a tiny HTTP control server so another Aurora can call a core set of
//!     its commands (`GET /aurora/ping`, `POST /aurora/invoke`).
//!
//! Only same-LAN reachability is assumed; the control server binds to the LAN.
//! This first cut proxies Minecraft server/instance management + game status —
//! enough to manage servers on another PC. Live event streaming and full-UI
//! proxying are later increments.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use tokio::sync::Mutex;

const DISCOVERY_PORT: u16 = 48999;
const CONTROL_PORT: u16 = 48998;
const BEACON_SECS: u64 = 3;
const PEER_TTL: Duration = Duration::from_secs(12);

/// A discovered Aurora PC on the LAN.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub id: String,
    pub name: String,
    pub ip: String,
    pub port: u16,
    #[serde(skip)]
    pub last_seen: Option<Instant>,
}

/// LAN presence state, held in `AppState`.
pub struct NetState {
    pub id: String,
    pub name: String,
    pub peers: Mutex<HashMap<String, Peer>>,
}

impl NetState {
    pub fn new() -> Self {
        // Computer name is unique enough on a home LAN for auto-trust.
        let name = std::env::var("COMPUTERNAME")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Aurora PC".to_string());
        Self {
            id: name.clone(),
            name,
            peers: Mutex::new(HashMap::new()),
        }
    }
}

/// Start discovery (broadcast + listen) and the control server.
pub fn start(app: AppHandle) {
    spawn_beacon(app.clone());
    spawn_listener(app.clone());
    spawn_control_server(app);
}

fn net(app: &AppHandle) -> tauri::State<'_, crate::state::AppState> {
    app.state::<crate::state::AppState>()
}

/// Broadcast "I'm here" every few seconds.
fn spawn_beacon(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let sock = match UdpSocket::bind(("0.0.0.0", 0)).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let _ = sock.set_broadcast(true);
        let (id, name) = {
            let st = net(&app);
            (st.net.id.clone(), st.net.name.clone())
        };
        loop {
            let beacon = serde_json::json!({ "k": "aurora", "id": id, "name": name, "port": CONTROL_PORT }).to_string();
            let _ = sock.send_to(beacon.as_bytes(), ("255.255.255.255", DISCOVERY_PORT)).await;
            tokio::time::sleep(Duration::from_secs(BEACON_SECS)).await;
        }
    });
}

/// Listen for beacons + prune stale peers; emit `net-peers` on change.
fn spawn_listener(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let sock = match UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)).await {
            Ok(s) => s,
            Err(_) => return,
        };
        let my_id = net(&app).net.id.clone();
        let mut buf = vec![0u8; 2048];
        loop {
            // Wait for a packet, but wake periodically to prune stale peers.
            let recv = tokio::time::timeout(Duration::from_secs(BEACON_SECS), sock.recv_from(&mut buf)).await;
            let mut changed = false;
            if let Ok(Ok((n, src))) = recv {
                if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&buf[..n]) {
                    if v.get("k").and_then(|x| x.as_str()) == Some("aurora") {
                        let id = v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("PC").to_string();
                        let port = v.get("port").and_then(|x| x.as_u64()).unwrap_or(CONTROL_PORT as u64) as u16;
                        if !id.is_empty() && id != my_id {
                            let st = net(&app);
                            let mut peers = st.net.peers.lock().await;
                            let existed = peers.contains_key(&id);
                            peers.insert(id.clone(), Peer { id, name, ip: src.ip().to_string(), port, last_seen: Some(Instant::now()) });
                            changed = !existed;
                        }
                    }
                }
            }
            // Prune.
            {
                let st = net(&app);
                let mut peers = st.net.peers.lock().await;
                let before = peers.len();
                peers.retain(|_, p| p.last_seen.map(|t| t.elapsed() < PEER_TTL).unwrap_or(false));
                if peers.len() != before {
                    changed = true;
                }
            }
            if changed {
                let list = current_peers(&app).await;
                let _ = app.emit("net-peers", list);
            }
        }
    });
}

pub async fn current_peers(app: &AppHandle) -> Vec<Peer> {
    let mut list: Vec<Peer> = net(app).net.peers.lock().await.values().cloned().collect();
    list.sort_by(|a, b| a.name.cmp(&b.name));
    list
}

// --- Control server ------------------------------------------------------

fn spawn_control_server(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind(("0.0.0.0", CONTROL_PORT)).await {
            Ok(l) => l,
            Err(_) => return,
        };
        loop {
            if let Ok((stream, _)) = listener.accept().await {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let _ = handle_conn(app, stream).await;
                });
            }
        }
    });
}

fn find_sub(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

async fn handle_conn(app: AppHandle, mut stream: TcpStream) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find_sub(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 1 << 20 {
            return Ok(());
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut hlines = head.lines();
    let req_line = hlines.next().unwrap_or("");
    let mut p = req_line.split_whitespace();
    let method = p.next().unwrap_or("").to_string();
    let path = p.next().unwrap_or("").to_string();
    let mut content_len = 0usize;
    for l in hlines {
        if let Some(v) = l.to_ascii_lowercase().strip_prefix("content-length:") {
            content_len = v.trim().parse().unwrap_or(0);
        }
    }
    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_len {
        let n = stream.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }

    let resp = route(&app, &method, &path, &body).await;
    stream.write_all(resp.as_bytes()).await?;
    Ok(())
}

fn http(status: &str, json: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {len}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\nAccess-Control-Allow-Methods: GET,POST,OPTIONS\r\nConnection: close\r\n\r\n{json}",
        len = json.as_bytes().len()
    )
}

async fn route(app: &AppHandle, method: &str, path: &str, body: &[u8]) -> String {
    if method == "OPTIONS" {
        return http("204 No Content", "");
    }
    if method == "GET" && path.starts_with("/aurora/ping") {
        let st = net(app);
        let id = st.net.id.clone();
        let name = st.net.name.clone();
        let v = serde_json::json!({ "id": id, "name": name, "version": env!("CARGO_PKG_VERSION") });
        return http("200 OK", &v.to_string());
    }
    if method == "POST" && path.starts_with("/aurora/invoke") {
        let req: serde_json::Value = match serde_json::from_slice(body) {
            Ok(v) => v,
            Err(_) => return http("400 Bad Request", r#"{"ok":false,"error":"bad json"}"#),
        };
        let cmd = req.get("cmd").and_then(|x| x.as_str()).unwrap_or("");
        let args = req.get("args").cloned().unwrap_or(serde_json::Value::Null);
        let out = match dispatch(app, cmd, &args).await {
            Ok(data) => serde_json::json!({ "ok": true, "data": data }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        };
        return http("200 OK", &out.to_string());
    }
    http("404 Not Found", r#"{"ok":false,"error":"not found"}"#)
}

fn jv<T: Serialize>(v: T) -> Result<serde_json::Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}
fn sarg(args: &serde_json::Value, key: &str) -> Result<String, String> {
    args.get(key).and_then(|x| x.as_str()).map(String::from).ok_or_else(|| format!("missing arg: {key}"))
}

/// Execute a proxied command on behalf of a remote controller. Increment 1
/// covers Minecraft server/instance management + game status.
async fn dispatch(app: &AppHandle, cmd: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
    use crate::commands;
    let st = || net(app);
    match cmd {
        "servers_status" => jv(commands::servers_status(st()).await?),
        "list_servers" => jv(commands::list_servers(st()).await?),
        "server_log_history" => jv(commands::server_log_history(st(), sarg(args, "id")?).await?),
        "server_start" => {
            commands::server_start(app.clone(), st(), sarg(args, "id")?).await?;
            Ok(serde_json::Value::Null)
        }
        "server_stop" => {
            commands::server_stop(app.clone(), st(), sarg(args, "id")?).await?;
            Ok(serde_json::Value::Null)
        }
        "server_command" => {
            commands::server_command(st(), sarg(args, "id")?, sarg(args, "line")?).await?;
            Ok(serde_json::Value::Null)
        }
        "list_instances" => jv(crate::instances::list_instances(st()).await?),
        "host_addresses" => jv(commands::host_addresses().await?),
        "detect_games" => jv(commands::detect_games()),
        "paths_info" => jv(commands::paths_info(st())),
        other => Err(format!("'{other}' isn't available remotely yet")),
    }
}
