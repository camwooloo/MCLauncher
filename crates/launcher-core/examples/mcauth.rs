//! Does our reqwest client reach login_with_xbox, or is it edge-blocked?
//! A dummy token should yield 401 if the request reaches the API, or 403 (with
//! a Cloudflare body) if our client is blocked at the edge.
use serde_json::json;

#[tokio::main]
async fn main() {
    let body = json!({ "identityToken": "XBL3.0 x=foo;bar" });
    let resp = launcher_core::http::client()
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .header("Accept", "application/json")
        .json(&body)
        .send()
        .await
        .expect("request failed");
    println!("status: {}", resp.status());
    println!("server: {:?}", resp.headers().get("server"));
    println!("cf-ray: {:?}", resp.headers().get("cf-ray"));
    println!("cf-mitigated: {:?}", resp.headers().get("cf-mitigated"));
    let t = resp.text().await.unwrap_or_default();
    println!("body: {}", t.chars().take(500).collect::<String>());
}
