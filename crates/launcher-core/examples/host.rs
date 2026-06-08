//! Headless server-hosting smoke test — reproduces the launcher's hosting path
//! and streams the server output so we can see exactly why a server starts/stops.
//!
//!   cargo run -p launcher-core --example host            # 1.21.1
//!   cargo run -p launcher-core --example host -- 1.20.1

use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use launcher_core::manifest::VersionManifest;
use launcher_core::paths::Paths;
use launcher_core::progress::{CountingReporter, SharedReporter};
use launcher_core::{java, server, Installer};
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version = std::env::args().nth(1).unwrap_or_else(|| "1.21.1".to_string());
    let loader = std::env::args().nth(2).unwrap_or_else(|| "vanilla".to_string());
    let base = std::env::temp_dir().join("mc-host-test");
    let paths = Paths::with_dirs(base.join("game"), base.join("data"));
    let dir = server::server_dir(&paths, &format!("test-{loader}"));

    let manifest = VersionManifest::fetch().await?;
    let installer = Installer::new(paths.clone());
    let vj = installer.resolve_version(&manifest, &version).await?;
    let reporter: SharedReporter = Arc::new(CountingReporter::default());

    let major = vj.java_version.as_ref().map(|j| j.major_version).unwrap_or(21);
    println!("== ensuring Java {major} ==");
    let java = java::ensure_java(&paths, major, &reporter).await?;

    // Client-install verification (no game launch): just install + resolve.
    if loader == "forge-client" || loader == "neoforge-client" {
        let id = if loader == "forge-client" {
            launcher_core::modloader::forge::install_client(&paths.game_dir, &version, &java, &reporter).await?
        } else {
            launcher_core::modloader::neoforge::install_client(&paths.game_dir, &version, &java, &reporter).await?
        };
        println!("== client profile id: {id} ==");
        let v = installer.resolve_version(&manifest, &id).await?;
        println!("== resolved OK: mainClass={:?}, {} libraries ==", v.main_class, v.libraries.len());
        return Ok(());
    }

    let launch_args: Vec<String> = match loader.as_str() {
        "forge" => {
            println!("== installing Forge server for {version} ==");
            let af = server::ensure_forge_server(&dir, &version, &java, &reporter).await?;
            println!("   forge args file: {af}");
            vec![format!("@{af}")]
        }
        "fabric" => {
            println!("== installing Fabric server for {version} ==");
            server::ensure_fabric_server_jar(&dir, &version, &reporter).await?;
            vec!["-jar".into(), "server.jar".into()]
        }
        _ => {
            println!("== downloading vanilla server jar for {version} ==");
            server::ensure_server_jar(&dir, &vj, &reporter).await?;
            vec!["-jar".into(), "server.jar".into()]
        }
    };
    server::accept_eula(&dir).await?;
    server::write_properties(&dir, 25599, 5, "Aurora test").await?;

    println!("== starting: java -Xmx2048M {} nogui ==", launch_args.join(" "));
    let mut cmd = tokio::process::Command::new(&java);
    cmd.current_dir(&dir).arg("-Xmx2048M");
    for a in &launch_args {
        cmd.arg(a);
    }
    let mut child = cmd
        .arg("nogui")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    tokio::spawn(async move {
        let mut l = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = l.next_line().await {
            println!("[srv] {line}");
        }
    });
    tokio::spawn(async move {
        let mut l = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = l.next_line().await {
            eprintln!("[err] {line}");
        }
    });

    tokio::select! {
        status = child.wait() => println!("\n== SERVER EXITED: {status:?} =="),
        _ = tokio::time::sleep(Duration::from_secs(45)) => {
            println!("\n== ran 45s without exiting (healthy); stopping ==");
            let _ = child.start_kill();
        }
    }
    Ok(())
}
