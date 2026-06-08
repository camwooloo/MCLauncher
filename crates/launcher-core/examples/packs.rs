//! Headless modpack-platform search check.
//! Usage: cargo run -p launcher-core --example packs -- <source> <query> [cfKey]
use launcher_core::modpacks::{curseforge, ftb, technic};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let source = args.get(1).map(|s| s.as_str()).unwrap_or("ftb");
    let query = args.get(2).cloned().unwrap_or_default();
    let key = args.get(3).cloned().unwrap_or_default();

    let hits = match source {
        "ftb" => ftb::search(&query).await?,
        "technic" => technic::search(&query).await?,
        "curseforge" => curseforge::search(&query, &key).await?,
        other => {
            eprintln!("unknown source {other}");
            return Ok(());
        }
    };
    for h in hits.iter().take(8) {
        let s: String = h.summary.chars().take(60).collect();
        println!("[{}] {} (dl={}) — {}", h.id, h.name, h.downloads, s);
    }
    println!("== {} results from {source} ==", hits.len());
    Ok(())
}
