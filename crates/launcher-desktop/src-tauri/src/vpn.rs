//! Built-in **Aurora Net** — a thin, auto-managed wrapper around Tailscale so
//! friends can reach each other's game servers (Minecraft, Skyrim Together,
//! Elden Ring co-op, …) with **no port forwarding**.
//!
//! Design:
//! * **Phase 1 – foundation.** Detect/install the official Tailscale client,
//!   report status, sign in (interactive, once), connect/disconnect.
//! * **Phase 2 – join.** A friend pastes a *join code* (an encoded blob holding
//!   an ephemeral auth key + the host's address). We bring them onto the host's
//!   tailnet as a throwaway guest and hand back the in-game address.
//! * **Phase 3 – share.** A host turns on sharing for a server: we mint a
//!   tagged, ephemeral, pre-authorised auth key (Tailscale API) and, optionally,
//!   set access rules so guests can reach **only that server** — then produce a
//!   join code.
//!
//! We can't unit-test the live tailnet here; everything shells out to the
//! `tailscale` CLI / Tailscale API and is exercised on a real machine.

use std::path::PathBuf;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// Arch-specific MSI (the plain `-latest.msi` 404s); redirects to the current
// versioned MSI, which reqwest follows.
const MSI_URL: &str = "https://pkgs.tailscale.com/stable/tailscale-setup-latest-amd64.msi";
const API_BASE: &str = "https://api.tailscale.com/api/v2";
/// Tag applied to guest devices so access rules can target them.
const GUEST_TAG: &str = "tag:aurora-guest";
/// Current join-code format version.
const CODE_VERSION: u8 = 1;

fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}

/// Locate the Tailscale CLI (`tailscale.exe`) if it's installed.
pub fn tailscale_exe() -> Option<PathBuf> {
    for c in [
        r"C:\Program Files\Tailscale\tailscale.exe",
        r"C:\Program Files (x86)\Tailscale\tailscale.exe",
    ] {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Snapshot of the local Tailscale state for the UI.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VpnStatus {
    pub installed: bool,
    /// Backend is up and connected.
    pub running: bool,
    /// Has an account (logged in), even if currently stopped.
    pub logged_in: bool,
    /// This device's Aurora Net (Tailscale) IPv4, e.g. `100.x.y.z`.
    pub ip: Option<String>,
    pub hostname: Option<String>,
}

/// Read the current Tailscale status (never fails — returns a best-effort
/// snapshot, with `installed: false` if the CLI is absent).
pub async fn status() -> VpnStatus {
    let Some(exe) = tailscale_exe() else {
        return VpnStatus::default();
    };
    let mut st = VpnStatus {
        installed: true,
        ..Default::default()
    };
    let Ok(out) = Command::new(&exe).args(["status", "--json"]).output().await else {
        return st;
    };
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
        let backend = v.get("BackendState").and_then(|s| s.as_str()).unwrap_or("");
        st.running = backend == "Running";
        st.logged_in = matches!(backend, "Running" | "Stopped");
        if let Some(me) = v.get("Self") {
            st.hostname = me.get("HostName").and_then(|s| s.as_str()).map(String::from);
            st.ip = me
                .get("TailscaleIPs")
                .and_then(|a| a.as_array())
                .and_then(|a| a.iter().find_map(|ip| ip.as_str()))
                .map(String::from);
        }
    }
    st
}

/// A device on your Aurora Net (a "friend"), from `tailscale status`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Peer {
    pub name: String,
    pub ip: Option<String>,
    pub online: bool,
    /// This device (you).
    pub me: bool,
}

fn node_to_peer(node: &serde_json::Value, me: bool) -> Peer {
    Peer {
        name: node.get("HostName").and_then(|s| s.as_str()).unwrap_or("device").to_string(),
        ip: node
            .get("TailscaleIPs")
            .and_then(|a| a.as_array())
            .and_then(|a| a.iter().find_map(|x| x.as_str()))
            .map(String::from),
        online: node.get("Online").and_then(|b| b.as_bool()).unwrap_or(false),
        me,
    }
}

/// Everyone on your tailnet (you + peers) with online state.
pub async fn peers() -> Vec<Peer> {
    let Some(exe) = tailscale_exe() else {
        return vec![];
    };
    let Ok(out) = Command::new(&exe).args(["status", "--json"]).output().await else {
        return vec![];
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return vec![];
    };
    let mut peers = Vec::new();
    if let Some(me) = v.get("Self") {
        peers.push(node_to_peer(me, true));
    }
    if let Some(map) = v.get("Peer").and_then(|p| p.as_object()) {
        for node in map.values() {
            peers.push(node_to_peer(node, false));
        }
    }
    // Online first, then by name.
    peers.sort_by(|a, b| b.online.cmp(&a.online).then(a.name.cmp(&b.name)));
    peers
}

/// Download and run the official Tailscale installer (shows a UAC prompt).
pub async fn install() -> Result<(), String> {
    let bytes = launcher_core::http::client()
        .get(MSI_URL)
        .send()
        .await
        .map_err(e)?
        .error_for_status()
        .map_err(|_| "Couldn't download the Tailscale installer".to_string())?
        .bytes()
        .await
        .map_err(e)?;
    let msi = std::env::temp_dir().join("aurora-tailscale-setup.msi");
    tokio::fs::write(&msi, &bytes).await.map_err(e)?;

    let status = Command::new("msiexec")
        .arg("/i")
        .arg(&msi)
        .args(["/qb", "/norestart"])
        .status()
        .await
        .map_err(e)?;
    if !status.success() {
        return Err("Tailscale installation was cancelled or failed".into());
    }
    Ok(())
}

/// Bring the node up interactively. Returns a login URL to open in the browser
/// if the user still needs to authenticate; `None` if already signed in (the
/// node is simply brought back up). The background `up` process is left running
/// so authentication can complete when the user approves in the browser.
pub async fn login() -> Result<Option<String>, String> {
    let exe = tailscale_exe().ok_or("Tailscale isn't installed yet")?;
    let mut child = Command::new(&exe)
        .args(["up", "--accept-routes=false", "--reset"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(e)?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut lines: Vec<String> = Vec::new();
    let scan = async {
        // Merge stdout+stderr line readers; the auth URL is printed to stderr.
        let mut outs = stdout.map(|s| BufReader::new(s).lines());
        let mut errs = stderr.map(|s| BufReader::new(s).lines());
        loop {
            let next = async {
                if let Some(r) = errs.as_mut() {
                    if let Ok(Some(l)) = r.next_line().await {
                        return Some(l);
                    }
                }
                if let Some(r) = outs.as_mut() {
                    if let Ok(Some(l)) = r.next_line().await {
                        return Some(l);
                    }
                }
                None
            };
            match next.await {
                Some(line) => {
                    if let Some(url) = line
                        .split_whitespace()
                        .find(|w| w.starts_with("https://login.tailscale.com/"))
                    {
                        return Some(url.to_string());
                    }
                    lines.push(line);
                }
                None => return None,
            }
        }
    };

    let found = tokio::time::timeout(std::time::Duration::from_secs(20), scan)
        .await
        .ok()
        .flatten();

    // Let `up` keep running in the background to finish auth, then reap it.
    tokio::spawn(async move {
        let _ = child.wait().await;
    });
    Ok(found)
}

/// Join a tailnet using a pre-shared auth key (the guest side of a join code).
///
/// Crucially we **log out first**. If the joiner is already signed into their
/// *own* tailnet (e.g. they connected Aurora Net before being invited), a plain
/// `up --authkey` does not switch networks — they'd stay on their tailnet and
/// never see the host. Logging out clears any prior identity so the key joins
/// the host's tailnet cleanly as a fresh guest. `--reset` drops any leftover
/// prefs (exit nodes, routes) that could otherwise interfere.
pub async fn up_with_authkey(authkey: &str) -> Result<(), String> {
    let exe = tailscale_exe().ok_or("Tailscale isn't installed yet")?;
    // Best-effort: ignore "not logged in" / already-down errors.
    let _ = Command::new(&exe).arg("logout").output().await;
    let out = Command::new(&exe)
        .args(["up", "--authkey", authkey, "--reset", "--accept-routes=false"])
        .output()
        .await
        .map_err(e)?;
    if !out.status.success() {
        let msg = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "Couldn't join the network: {}",
            msg.lines().next().unwrap_or("unknown error").trim()
        ));
    }
    Ok(())
}

/// Disconnect from the tailnet (stays installed and logged in).
pub async fn down() -> Result<(), String> {
    let exe = tailscale_exe().ok_or("Tailscale isn't installed")?;
    Command::new(&exe).arg("down").status().await.map_err(e)?;
    Ok(())
}

// --- Phase 3: hosting (Tailscale API) ------------------------------------

/// Mint a single-use, ephemeral, pre-authorised auth key tagged for guests.
/// Requires an API access token with key-create scope. Tags require the tailnet
/// ACL to own `tag:aurora-guest` (see [`ensure_access_rules`]).
pub async fn mint_join_key(api_token: &str) -> Result<String, String> {
    let body = serde_json::json!({
        "capabilities": { "devices": { "create": {
            "reusable": false,
            "ephemeral": true,
            "preauthorized": true,
            "tags": [GUEST_TAG],
        }}},
        "expirySeconds": 86_400, // 24h
        "description": "Aurora Net guest",
    });
    let resp = launcher_core::http::client()
        .post(format!("{API_BASE}/tailnet/-/keys"))
        .bearer_auth(api_token)
        .json(&body)
        .send()
        .await
        .map_err(e)?;
    if !resp.status().is_success() {
        let code = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        return Err(format!(
            "Couldn't create a join key ({code}). Check your Tailscale access token. {}",
            txt.chars().take(200).collect::<String>()
        ));
    }
    let v: serde_json::Value = resp.json().await.map_err(e)?;
    v.get("key")
        .and_then(|s| s.as_str())
        .map(String::from)
        .ok_or_else(|| "Tailscale returned no key".into())
}

/// Ensure the tailnet's access policy (a) owns the guest tag (so keys can be
/// tagged) and (b) lets guests reach **only** `host_ip` on the given ports.
///
/// To make "only this server" actually hold, a default *allow-all* base rule
/// (`*` → `*:*`) is narrowed to `autogroup:member` so tagged guests fall
/// outside it. If the policy is already customised, we leave the base rules
/// alone (to avoid breaking a hand-tuned tailnet) and only add the guest rule.
pub async fn ensure_access_rules(
    api_token: &str,
    host_ip: &str,
    ports: &[u16],
) -> Result<(), String> {
    let client = launcher_core::http::client();
    let resp = client
        .get(format!("{API_BASE}/tailnet/-/acl"))
        .header("Accept", "application/json")
        .bearer_auth(api_token)
        .send()
        .await
        .map_err(e)?;
    if !resp.status().is_success() {
        return Err(format!(
            "Couldn't read your Tailscale access rules ({}). The token needs ACL scope.",
            resp.status()
        ));
    }
    let mut policy: serde_json::Value = resp.json().await.map_err(e)?;
    let obj = policy
        .as_object_mut()
        .ok_or("Unexpected access-policy format")?;

    // (a) tag ownership for the guest tag.
    let owners = obj
        .entry("tagOwners")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(map) = owners.as_object_mut() {
        map.entry(GUEST_TAG.to_string())
            .or_insert_with(|| serde_json::json!(["autogroup:admin"]));
    }

    // (b) access rules.
    let acls = obj.entry("acls").or_insert_with(|| serde_json::json!([]));
    if let Some(arr) = acls.as_array_mut() {
        // Narrow a default allow-all so guests aren't swept in by it.
        for rule in arr.iter_mut() {
            let is_default_allow = rule
                .get("src")
                .and_then(|s| s.as_array())
                .map(|a| a.len() == 1 && a[0] == "*")
                .unwrap_or(false)
                && rule
                    .get("dst")
                    .and_then(|d| d.as_array())
                    .map(|a| a.iter().any(|x| x == "*:*"))
                    .unwrap_or(false);
            if is_default_allow {
                rule["src"] = serde_json::json!(["autogroup:member"]);
            }
        }
        // Drop any previous Aurora guest rule, then add a fresh one.
        arr.retain(|r| r.get("src") != Some(&serde_json::json!([GUEST_TAG])));
        let dst: Vec<String> = ports.iter().map(|p| format!("{host_ip}:{p}")).collect();
        arr.push(serde_json::json!({
            "action": "accept",
            "src": [GUEST_TAG],
            "dst": dst,
        }));
    }

    let put = client
        .post(format!("{API_BASE}/tailnet/-/acl"))
        .header("Content-Type", "application/json")
        .bearer_auth(api_token)
        .json(&policy)
        .send()
        .await
        .map_err(e)?;
    if !put.status().is_success() {
        let txt = put.text().await.unwrap_or_default();
        return Err(format!(
            "Couldn't update access rules: {}",
            txt.chars().take(200).collect::<String>()
        ));
    }
    Ok(())
}

// --- Join code encoding --------------------------------------------------

/// A Minecraft modpack reference carried in an invite, so the guest's launcher
/// can build a matching client instance with one click.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackRef {
    /// "modrinth" | "curseforge" | "ftb" | "technic".
    pub source: String,
    pub project_id: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
}

/// The payload encoded into a join code and shared with friends.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinPayload {
    pub v: u8,
    /// Ephemeral Tailscale auth key the guest uses to join.
    pub key: String,
    /// Host's Aurora Net IP.
    pub ip: String,
    pub port: u16,
    pub name: String,
    /// "minecraft" | "skyrim" | "eldenring" | "cyberpunk".
    pub game: String,
    /// Optional modpack to auto-install on the guest (Minecraft co-op).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<PackRef>,
}

/// Encode a payload into an opaque, copy-pasteable join code.
pub fn encode_code(p: &JoinPayload) -> Result<String, String> {
    use base64::Engine;
    let json = serde_json::to_vec(p).map_err(e)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json))
}

/// Decode a join code (tolerates an `aurora-net:` prefix and whitespace).
pub fn decode_code(code: &str) -> Result<JoinPayload, String> {
    use base64::Engine;
    let trimmed = code.trim().trim_start_matches("aurora-net:").trim();
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(trimmed.as_bytes())
        .map_err(|_| "That doesn't look like a valid join code".to_string())?;
    let p: JoinPayload = serde_json::from_slice(&bytes)
        .map_err(|_| "That join code is malformed or from a newer version".to_string())?;
    if p.v != CODE_VERSION {
        return Err("That join code is from a different version of Aurora".into());
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_code_roundtrips() {
        let p = JoinPayload {
            v: CODE_VERSION,
            key: "tskey-auth-abc123".into(),
            ip: "100.101.102.103".into(),
            port: 25565,
            name: "Cam's SMP".into(),
            game: "minecraft".into(),
            pack: Some(PackRef {
                source: "modrinth".into(),
                project_id: "1KVo5zza".into(),
                title: "Fabulously Optimized".into(),
                icon: None,
            }),
        };
        let code = encode_code(&p).unwrap();
        // Survives the user-facing prefix + stray whitespace.
        let decoded = decode_code(&format!("  aurora-net:{code}\n")).unwrap();
        assert_eq!(decoded.key, p.key);
        assert_eq!(decoded.ip, p.ip);
        assert_eq!(decoded.port, 25565);
        assert_eq!(decoded.name, "Cam's SMP");
    }

    #[test]
    fn rejects_garbage_code() {
        assert!(decode_code("not-a-code!!!").is_err());
    }
}
