//! A shared, cloneable HTTP client.
//!
//! `reqwest::Client` already wraps an `Arc` internally, so cloning is cheap and
//! connection-pooling is shared. We centralise construction here so every
//! subsystem uses the same user-agent and timeouts.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

/// User-agent sent with every request.
///
/// Must include a browser-like token: `api.minecraftservices.com` sits behind
/// Cloudflare, which returns **403 Forbidden** to `login_with_xbox` (and other
/// endpoints) when the UA looks like a bare script agent (e.g. "MCLauncher/0.1").
/// A `Mozilla/5.0 …` prefix clears the bot filter while staying identifiable.
pub const USER_AGENT: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AuroraLauncher/",
    env!("CARGO_PKG_VERSION")
);

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Returns the process-wide shared HTTP client, building it on first use.
pub fn client() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .user_agent(USER_AGENT)
            .connect_timeout(Duration::from_secs(30))
            // No global request timeout: large asset/library downloads can run
            // long. Per-request timeouts are applied where appropriate.
            .build()
            .expect("failed to build reqwest client")
    })
}
