//! Headless check of the game-tool installers (no game required).
//! Usage: cargo run -p launcher-core --example tools -- <skse|seamless|modengine|cet>
//! Installs into a temp dir and lists key files so we can verify layout.
use std::sync::Arc;

use launcher_core::games::{cyberpunk, eldenring, skyrim};
use launcher_core::progress::{CountingReporter, SharedReporter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let which = std::env::args().nth(1).unwrap_or_else(|| "skse".into());
    let dest = std::env::temp_dir().join(format!("aurora-tool-{which}"));
    let _ = std::fs::remove_dir_all(&dest);
    std::fs::create_dir_all(&dest)?;
    let reporter: SharedReporter = Arc::new(CountingReporter::default());

    let tag = match which.as_str() {
        "skse" => skyrim::install_skse(&dest, &reporter).await?,
        "seamless" => eldenring::install_seamless(&dest, &reporter).await?,
        "modengine" => eldenring::install_mod_engine(&dest, &reporter).await?,
        "cet" => cyberpunk::install_cet(&dest, &reporter).await?,
        // Local Skyrim Together zip: tools -- together <path-to-zip>
        "together" => {
            let zip = std::env::args().nth(2).ok_or("usage: together <zip>")?;
            skyrim::install_together_from_zip(&dest, std::path::Path::new(&zip))?;
            "local".to_string()
        }
        other => return Err(format!("unknown tool {other}").into()),
    };
    println!("installed tag: {tag}");

    // List the top two levels so we can confirm the layout.
    for entry in std::fs::read_dir(&dest)?.flatten().take(15) {
        let p = entry.path();
        println!("  {}", p.strip_prefix(&dest)?.display());
        if p.is_dir() {
            for sub in std::fs::read_dir(&p)?.flatten().take(6) {
                println!("    {}", sub.path().strip_prefix(&dest)?.display());
            }
        }
    }
    Ok(())
}
