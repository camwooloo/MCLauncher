//! Windows Firewall + Tailscale-interface helpers so hosted servers are
//! actually reachable over **Aurora Net**.
//!
//! The problem: Windows puts the Tailscale adapter on the **Public** firewall
//! profile, and the per-app inbound rules games create on first run are usually
//! **Private-only**. So co-op works over the LAN or a router VPN (UniFi etc.)
//! but silently fails over Tailscale. We fix it by adding an **all-profiles**
//! inbound allow rule for the server, and marking the Tailscale adapter
//! Private.
//!
//! Adding firewall rules needs admin, so the work runs through a single
//! elevated PowerShell (one UAC prompt). It's guarded by a rule-exists check so
//! a returning host isn't prompted again. All of this is best-effort: if the
//! user declines UAC, hosting still proceeds (just maybe not over Tailscale).

/// Escape a string for embedding inside a PowerShell single-quoted literal.
#[cfg(windows)]
fn psq(s: &str) -> String {
    s.replace('\'', "''")
}

/// Mark every Tailscale adapter Private so its traffic isn't filtered as Public.
#[cfg(windows)]
const TS_PRIVATE: &str = "Get-NetConnectionProfile | Where-Object { $_.InterfaceAlias -like '*Tailscale*' } | Set-NetConnectionProfile -NetworkCategory Private";

#[cfg(windows)]
fn rule_exists(name: &str) -> bool {
    std::process::Command::new("netsh")
        .args(["advfirewall", "firewall", "show", "rule", &format!("name={name}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a PowerShell script elevated (UAC), waiting for it to finish.
#[cfg(windows)]
fn run_elevated(script: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;

    let path = std::env::temp_dir().join("aurora-net-firewall.ps1");
    std::fs::File::create(&path)
        .and_then(|mut f| f.write_all(script.as_bytes()))
        .map_err(|e| e.to_string())?;

    let inner = format!(
        "Start-Process powershell -Verb RunAs -Wait -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','{}'",
        psq(&path.display().to_string())
    );
    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-WindowStyle", "Hidden", "-Command", &inner])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW — no console flash
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("Firewall setup needs admin approval — Aurora Net may not reach this server until it's allowed.".into())
    }
}

/// Allow inbound to a specific program (used for native game servers like the
/// Skyrim Together dedicated server, which uses UDP under the hood).
pub fn ensure_program_allowed(label: &str, exe: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        if rule_exists(label) {
            return Ok(());
        }
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'\r\n\
             Remove-NetFirewallRule -DisplayName '{name}'\r\n\
             New-NetFirewallRule -DisplayName '{name}' -Direction Inbound -Action Allow -Profile Any -Program '{exe}' | Out-Null\r\n\
             {ts}\r\n",
            name = psq(label),
            exe = psq(&exe.to_string_lossy()),
            ts = TS_PRIVATE,
        );
        run_elevated(&script)
    }
    #[cfg(not(windows))]
    {
        let _ = (label, exe);
        Ok(())
    }
}

/// Allow inbound on a specific TCP (and optionally UDP) port — used for the
/// Minecraft server, which runs under `java.exe` (so a port rule is cleaner
/// than a program rule).
pub fn ensure_port_allowed(label: &str, port: u16, udp: bool) -> Result<(), String> {
    #[cfg(windows)]
    {
        if rule_exists(label) {
            return Ok(());
        }
        let proto = if udp { "UDP" } else { "TCP" };
        let script = format!(
            "$ErrorActionPreference='SilentlyContinue'\r\n\
             Remove-NetFirewallRule -DisplayName '{name}'\r\n\
             New-NetFirewallRule -DisplayName '{name}' -Direction Inbound -Action Allow -Profile Any -Protocol {proto} -LocalPort {port} | Out-Null\r\n\
             {ts}\r\n",
            name = psq(label),
            ts = TS_PRIVATE,
        );
        run_elevated(&script)
    }
    #[cfg(not(windows))]
    {
        let _ = (label, port, udp);
        Ok(())
    }
}
