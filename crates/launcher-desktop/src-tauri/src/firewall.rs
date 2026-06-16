//! Windows Firewall + Tailscale-interface helper so hosted servers are
//! reachable over **Aurora Net**.
//!
//! The problem: Windows puts the Tailscale adapter on the **Public** firewall
//! profile, and the per-app inbound rules games create on first run are usually
//! **Private-only** — so co-op works over the LAN or a router VPN (UniFi etc.)
//! but silently fails over Tailscale.
//!
//! The fix is deliberately broad and **one-time**: a single inbound allow rule
//! scoped to the Tailscale address range (`100.64.0.0/10`, the CGNAT block every
//! tailnet IP falls in) on **all profiles**, plus marking the Tailscale adapter
//! Private. This covers every current and future hosted server (Minecraft,
//! Skyrim Together, …) over Aurora Net, while only accepting traffic from Aurora
//! Net peers — never the public internet. Only the **host** needs it; joiners
//! make outbound connections, which Windows already allows.
//!
//! Adding a rule needs admin, so it runs through a single, windowless, elevated
//! PowerShell (one UAC consent). It's guarded by a rule-exists check, so once
//! it's in place a returning host is never prompted again. Best-effort: if the
//! user declines, hosting still proceeds.

/// The display name of our single Aurora Net firewall rule.
#[cfg(windows)]
const RULE_NAME: &str = "Aurora Net (co-op)";
/// Tailscale's CGNAT range — every tailnet peer IP lives here.
#[cfg(windows)]
const TS_RANGE: &str = "100.64.0.0/10";

/// Escape a string for embedding inside a PowerShell single-quoted literal.
#[cfg(windows)]
fn psq(s: &str) -> String {
    s.replace('\'', "''")
}

#[cfg(windows)]
fn rule_exists(name: &str) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={name}")])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a PowerShell script elevated (one UAC consent), windowless, and wait.
#[cfg(windows)]
fn run_elevated(script: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;

    let path = std::env::temp_dir().join("aurora-net-firewall.ps1");
    std::fs::File::create(&path)
        .and_then(|mut f| f.write_all(script.as_bytes()))
        .map_err(|e| e.to_string())?;

    // Inner elevated process is launched hidden + non-interactive so no console
    // window flashes; only the (unavoidable) UAC consent dialog appears.
    let inner = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList \
         '-NoProfile','-NonInteractive','-WindowStyle','Hidden','-ExecutionPolicy','Bypass','-File','{}'",
        psq(&path.display().to_string())
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-WindowStyle", "Hidden", "-Command", &inner])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW — hide the outer shell
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Couldn't set up the Aurora Net firewall rule (admin approval needed).".into())
    }
}

/// Ensure Aurora Net traffic can reach servers hosted on this PC. Idempotent —
/// only prompts (once) if the rule isn't already present. `force` re-applies
/// even if the rule exists (used by the manual "repair" button).
///
/// Returns `true` if it ran the elevated step, `false` if nothing was needed.
pub fn ensure_aurora_net(force: bool) -> Result<bool, String> {
    #[cfg(windows)]
    {
        if !force && rule_exists(RULE_NAME) {
            return Ok(false);
        }
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'\r\n\
             Remove-NetFirewallRule -DisplayName '{name}'\r\n\
             New-NetFirewallRule -DisplayName '{name}' -Direction Inbound -Action Allow \
             -Profile Any -RemoteAddress {range} | Out-Null\r\n\
             Get-NetConnectionProfile | Where-Object {{ $_.InterfaceAlias -like '*Tailscale*' }} | \
             Set-NetConnectionProfile -NetworkCategory Private\r\n",
            name = psq(RULE_NAME),
            range = TS_RANGE,
        );
        run_elevated(&script)?;
        Ok(true)
    }
    #[cfg(not(windows))]
    {
        let _ = force;
        Ok(false)
    }
}
