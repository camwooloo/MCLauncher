//! Microsoft → Xbox Live → Minecraft authentication.
//!
//! Mojang accounts are gone; logging in is a five-step token chain:
//!
//! 1. **Microsoft OAuth2** — two interactive flows are offered:
//!    * [`Auth::login_auth_code`] — the "no-code" flow other launchers use: we
//!      spin up a throwaway `http://localhost:<port>` listener, open the system
//!      browser to the Microsoft sign-in page, and capture the redirect with
//!      the authorization code (PKCE-protected). The user just signs in and
//!      approves — nothing to copy.
//!    * [`Auth::login_device_code`] — the fallback: the user visits a URL and
//!      types a short code. Needs no redirect URI registered.
//!    Either yields an MS access token + refresh token.
//! 2. **Xbox Live** — exchange the MS token for an XBL token + user hash.
//! 3. **XSTS** — authorize the XBL token for the Minecraft relying party;
//!    also surfaces the XUID and common "no Xbox account / child account"
//!    errors.
//! 4. **Minecraft** — `login_with_xbox` yields the Minecraft access token.
//! 5. **Profile** — fetch the player's UUID and name.
//!
//! ### Azure setup required
//! Register a free Azure AD application (Entra ID) to obtain a **client id**,
//! configure it as a *public client*, and enable the Minecraft API scope. The
//! device-code flow needs only "Allow public client flows". The no-code
//! ([`login_auth_code`]) flow additionally needs a **redirect URI** of
//! `http://localhost` registered under *Mobile and desktop applications* (the
//! loopback port is wildcarded by Microsoft, so any port works). Pass the id to
//! [`Auth::new`].

use serde::Deserialize;
use serde_json::json;

use crate::account::Account;
use crate::{Error, Result};

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
const AUTHORIZE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize";
const TOKEN_URL: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0/token";
const XBL_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MC_LOGIN_URL: &str = "https://api.minecraftservices.com/authentication/login_with_xbox";
const MC_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";

/// Scopes: Xbox Live sign-in + refresh token, plus OIDC scopes so the token
/// response carries an `id_token` we can decode to show *which* account signed
/// in (helps when a browser SSOs the wrong Microsoft account).
const SCOPE: &str = "XboxLive.signin offline_access openid email profile";

/// Pull a human identifier (email / username) out of an OIDC id_token's payload
/// — unverified decode is fine, it's only shown to the user for confirmation.
fn account_hint(id_token: &str) -> Option<String> {
    use base64::Engine;
    let payload = id_token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let email = json
        .get("email")
        .or_else(|| json.get("preferred_username"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    // The issuer tells us whether this is a personal Microsoft account (which can
    // own Minecraft) or an org/Entra identity (which cannot — and 403s here).
    let iss = json.get("iss").and_then(|v| v.as_str()).unwrap_or("");
    let kind = if iss.contains("9188040d-6c67-4c5b-b112-36a304b66dad") || iss.contains("login.live.com") {
        "personal MSA"
    } else if iss.is_empty() {
        "unknown type"
    } else {
        "ORG/Entra account (cannot own Minecraft)"
    };
    Some(format!("{email} ({kind})"))
}

/// The authentication client, bound to an Azure application client id.
#[derive(Debug, Clone)]
pub struct Auth {
    client_id: String,
}

/// The result of a successful login: a usable account plus the refresh token
/// to persist for silent re-login.
#[derive(Debug, Clone)]
pub struct AuthResult {
    pub account: Account,
    pub refresh_token: String,
}

/// Device-code prompt to show the user while they authenticate in a browser.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceCode {
    /// The short code the user types.
    pub user_code: String,
    /// Where the user enters it (e.g. https://microsoft.com/link).
    pub verification_uri: String,
    /// Human-readable instructions from Microsoft.
    pub message: String,
    device_code: String,
    interval: u64,
    #[allow(dead_code)]
    expires_in: u64,
}

impl Auth {
    pub fn new(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
        }
    }

    /// High-level login. Calls `on_prompt` with the device code for display,
    /// then blocks (polling) until the user completes sign-in, then runs the
    /// full token chain.
    pub async fn login_device_code<F>(&self, on_prompt: F) -> Result<AuthResult>
    where
        F: FnOnce(&DeviceCode),
    {
        let code = self.request_device_code().await?;
        on_prompt(&code);
        let ms = self.poll_for_token(&code).await?;
        self.complete_chain(ms).await
    }

    /// Silent re-login using a stored refresh token.
    pub async fn login_with_refresh(&self, refresh_token: &str) -> Result<AuthResult> {
        let ms = self.refresh(refresh_token).await?;
        self.complete_chain(ms).await
    }

    /// "No-code" login (authorization-code + PKCE over a loopback redirect).
    ///
    /// Opens a local listener on `127.0.0.1:<random port>`, calls `open_browser`
    /// with the Microsoft sign-in URL (the caller launches the system browser),
    /// then waits for the browser to redirect back with the authorization code.
    /// The user never copies anything — they just sign in and approve.
    ///
    /// Requires `http://localhost` to be a registered redirect URI on the Azure
    /// app (*Mobile and desktop applications* platform).
    pub async fn login_auth_code<F>(&self, open_browser: F) -> Result<AuthResult>
    where
        F: FnOnce(&str),
    {
        use base64::Engine;

        // --- PKCE: high-entropy verifier + its S256 challenge ---
        let verifier = random_b64url(32)?;
        let challenge = {
            use sha2::{Digest, Sha256};
            let digest = Sha256::digest(verifier.as_bytes());
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
        };
        // CSRF guard echoed back in the redirect.
        let state = random_b64url(16)?;

        // --- Loopback listener on an ephemeral port ---
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|e| Error::Auth(format!("couldn't start the local sign-in server: {e}")))?;
        let port = listener
            .local_addr()
            .map_err(|e| Error::Auth(e.to_string()))?
            .port();
        let redirect_uri = format!("http://localhost:{port}");

        // --- Build the authorize URL and hand it to the browser ---
        let mut url = url::Url::parse(AUTHORIZE_URL)
            .map_err(|e| Error::Auth(format!("bad authorize URL: {e}")))?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", &redirect_uri)
            .append_pair("response_mode", "query")
            .append_pair("scope", SCOPE)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("state", &state)
            .append_pair("prompt", "select_account");
        open_browser(url.as_str());

        // --- Wait (with a 5-minute cap) for the redirect ---
        let code = tokio::time::timeout(
            std::time::Duration::from_secs(300),
            accept_redirect(&listener, &state),
        )
        .await
        .map_err(|_| Error::Auth("sign-in timed out — no response from the browser".into()))??;

        let ms = self.exchange_code(&code, &verifier, &redirect_uri).await?;
        self.complete_chain(ms).await
    }

    // --- Step 1: Microsoft OAuth2 (device code) --------------------------

    async fn request_device_code(&self) -> Result<DeviceCode> {
        let resp = crate::http::client()
            .post(DEVICE_CODE_URL)
            .form(&[("client_id", self.client_id.as_str()), ("scope", SCOPE)])
            .send()
            .await?
            .error_for_status()?
            .json::<DeviceCode>()
            .await?;
        Ok(resp)
    }

    async fn poll_for_token(&self, code: &DeviceCode) -> Result<MsToken> {
        let mut interval = code.interval.max(1);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;

            let resp = crate::http::client()
                .post(TOKEN_URL)
                .form(&[
                    ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                    ("client_id", self.client_id.as_str()),
                    ("device_code", code.device_code.as_str()),
                ])
                .send()
                .await?;

            if resp.status().is_success() {
                return Ok(resp.json::<MsToken>().await?);
            }

            let err = resp.json::<OAuthError>().await.unwrap_or(OAuthError {
                error: "unknown_error".into(),
                error_description: None,
            });
            match err.error.as_str() {
                "authorization_pending" => continue,
                "slow_down" => {
                    interval += 5;
                    continue;
                }
                other => {
                    return Err(Error::Auth(format!(
                        "device-code login failed: {other}{}",
                        err.error_description
                            .map(|d| format!(" — {d}"))
                            .unwrap_or_default()
                    )))
                }
            }
        }
    }

    /// Exchange a loopback authorization code (+ PKCE verifier) for tokens.
    async fn exchange_code(
        &self,
        code: &str,
        verifier: &str,
        redirect_uri: &str,
    ) -> Result<MsToken> {
        let resp = crate::http::client()
            .post(TOKEN_URL)
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", redirect_uri),
                ("code_verifier", verifier),
                ("scope", SCOPE),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            let snippet: String = text.chars().take(300).collect();
            return Err(Error::Auth(format!(
                "token exchange failed: {}",
                snippet.trim()
            )));
        }
        Ok(resp.json::<MsToken>().await?)
    }

    async fn refresh(&self, refresh_token: &str) -> Result<MsToken> {
        let resp = crate::http::client()
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "refresh_token"),
                ("client_id", self.client_id.as_str()),
                ("refresh_token", refresh_token),
                ("scope", SCOPE),
            ])
            .send()
            .await?;
        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(Error::Auth(format!("token refresh failed: {err}")));
        }
        Ok(resp.json::<MsToken>().await?)
    }

    // --- Steps 2-5: Xbox → XSTS → Minecraft → profile --------------------

    async fn complete_chain(&self, ms: MsToken) -> Result<AuthResult> {
        let who = account_hint(&ms.id_token);
        let (xbl_token, _uhs) = self
            .xbox_authenticate(&ms.access_token)
            .await
            .map_err(|e| step_err("Xbox Live sign-in", e))?;
        let xsts = self
            .xsts_authorize(&xbl_token)
            .await
            .map_err(|e| step_err("Xbox authorization", e))?;
        let mc_token = self
            .minecraft_login(&xsts.uhs, &xsts.token, who.as_deref())
            .await
            .map_err(|e| step_err("Minecraft services sign-in", e))?;
        let profile = self
            .minecraft_profile(&mc_token)
            .await
            .map_err(|e| step_err("Minecraft profile", e))?;

        let account = Account {
            username: profile.name,
            uuid: profile.id,
            access_token: mc_token,
            xuid: xsts.xuid,
            user_type: "msa".to_string(),
        };
        Ok(AuthResult {
            account,
            refresh_token: ms.refresh_token,
        })
    }

    async fn xbox_authenticate(&self, ms_access_token: &str) -> Result<(String, String)> {
        let body = json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access_token}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        });
        let resp = crate::http::client()
            .post(XBL_URL)
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<XboxResponse>()
            .await?;
        let uhs = resp
            .display_claims
            .xui
            .first()
            .map(|c| c.uhs.clone())
            .ok_or_else(|| Error::Auth("Xbox response missing user hash".into()))?;
        Ok((resp.token, uhs))
    }

    async fn xsts_authorize(&self, xbl_token: &str) -> Result<Xsts> {
        let body = json!({
            "Properties": {
                "SandboxId": "RETAIL",
                "UserTokens": [xbl_token],
            },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        });
        let resp = crate::http::client().post(XSTS_URL).json(&body).send().await?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            let err = resp.json::<XstsError>().await.ok();
            let message = err
                .and_then(|e| e.friendly_message())
                .unwrap_or_else(|| "Xbox account not authorized".into());
            return Err(Error::Auth(message));
        }

        let resp = resp.error_for_status()?.json::<XboxResponse>().await?;
        let claim = resp
            .display_claims
            .xui
            .first()
            .ok_or_else(|| Error::Auth("XSTS response missing claims".into()))?;
        Ok(Xsts {
            token: resp.token,
            uhs: claim.uhs.clone(),
            xuid: claim.xid.clone().unwrap_or_default(),
        })
    }

    async fn minecraft_login(&self, uhs: &str, xsts_token: &str, who: Option<&str>) -> Result<String> {
        let body = json!({ "identityToken": format!("XBL3.0 x={uhs};{xsts_token}") });
        let resp = crate::http::client()
            .post(MC_LOGIN_URL)
            .header("Accept", "application/json")
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let acct = who.map(|w| format!("[{w}] ")).unwrap_or_default();
            let text = resp.text().await.unwrap_or_default();
            let snippet: String = text.chars().take(400).collect();
            let body = if snippet.trim().is_empty() { "<empty body>".to_string() } else { snippet.trim().to_string() };
            return Err(Error::Auth(format!(
                "{acct}Minecraft sign-in failed: HTTP {} from login_with_xbox. Server said: {}",
                status.as_u16(),
                body
            )));
        }
        Ok(resp.json::<McLoginResponse>().await?.access_token)
    }

    async fn minecraft_profile(&self, mc_token: &str) -> Result<McProfile> {
        let resp = crate::http::client()
            .get(MC_PROFILE_URL)
            .header("Accept", "application/json")
            .bearer_auth(mc_token)
            .send()
            .await?;
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(Error::Auth(
                "this Microsoft account does not own Minecraft".into(),
            ));
        }
        Ok(resp.error_for_status()?.json::<McProfile>().await?)
    }
}

/// `n` bytes of OS entropy as a URL-safe-no-pad base64 string (PKCE verifier
/// / CSRF state).
fn random_b64url(n: usize) -> Result<String> {
    use base64::Engine;
    let mut buf = vec![0u8; n];
    getrandom::getrandom(&mut buf)
        .map_err(|e| Error::Auth(format!("couldn't gather randomness for sign-in: {e}")))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

/// Accept connections on the loopback listener until one carries the OAuth
/// redirect (`/?code=…&state=…` or `/?error=…`). Other requests (favicon, etc.)
/// get a "waiting…" page and are ignored. Returns the authorization code.
async fn accept_redirect(listener: &tokio::net::TcpListener, expected_state: &str) -> Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    loop {
        let (mut stream, _) = listener
            .accept()
            .await
            .map_err(|e| Error::Auth(format!("loopback accept failed: {e}")))?;

        // The request line ("GET /path?query HTTP/1.1") is all we need.
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).await.unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let target = req
            .lines()
            .next()
            .and_then(|line| line.split_whitespace().nth(1))
            .unwrap_or("");
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");

        let mut code = None;
        let mut state = None;
        let mut error = None;
        let mut error_desc = None;
        for (k, v) in url::form_urlencoded::parse(query.as_bytes()) {
            match k.as_ref() {
                "code" => code = Some(v.into_owned()),
                "state" => state = Some(v.into_owned()),
                "error" => error = Some(v.into_owned()),
                "error_description" => error_desc = Some(v.into_owned()),
                _ => {}
            }
        }

        // Not the redirect (e.g. the browser's favicon probe) — keep waiting.
        if code.is_none() && error.is_none() {
            let _ = write_page(&mut stream, "Waiting for sign-in…", "You can return to Aurora Launcher.").await;
            continue;
        }

        if let Some(err) = error {
            let detail = error_desc.unwrap_or(err);
            let _ = write_page(&mut stream, "Sign-in cancelled", "You can close this tab and try again.").await;
            let _ = stream.flush().await;
            return Err(Error::Auth(format!("sign-in was cancelled or failed: {detail}")));
        }

        if state.as_deref() != Some(expected_state) {
            let _ = write_page(&mut stream, "Sign-in error", "State mismatch — please try again.").await;
            let _ = stream.flush().await;
            return Err(Error::Auth(
                "sign-in state mismatch (possible CSRF) — please try again".into(),
            ));
        }

        let _ = write_page(
            &mut stream,
            "Signed in ✓",
            "All set — you can close this tab and return to Aurora Launcher.",
        )
        .await;
        let _ = stream.flush().await;
        return code.ok_or_else(|| Error::Auth("no authorization code in redirect".into()));
    }
}

/// Write a minimal self-contained HTML page and close the connection.
async fn write_page(
    stream: &mut tokio::net::TcpStream,
    title: &str,
    note: &str,
) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    let body = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Aurora Launcher</title>\
<style>html{{height:100%}}body{{margin:0;height:100%;display:flex;align-items:center;\
justify-content:center;font-family:'Segoe UI',system-ui,sans-serif;\
background:radial-gradient(120% 120% at 30% 0%,#171034,#081226);color:#eef}}\
.card{{text-align:center;padding:40px 56px;border-radius:20px;\
background:rgba(255,255,255,0.06);border:1px solid rgba(255,255,255,0.14);\
box-shadow:0 20px 60px rgba(0,0,0,0.45)}}h1{{margin:0 0 8px;font-size:26px;\
background:linear-gradient(90deg,#b794f6,#34d399);-webkit-background-clip:text;\
background-clip:text;color:transparent}}p{{margin:0;color:#aeb6cc}}</style></head>\
<body><div class=\"card\"><h1>{title}</h1><p>{note}</p></div></body></html>"
    );
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.as_bytes().len(),
        body
    );
    stream.write_all(resp.as_bytes()).await
}

/// Add which step failed to an error, unless it's already a friendly Auth message.
fn step_err(step: &str, e: Error) -> Error {
    match e {
        Error::Auth(m) => Error::Auth(m),
        other => Error::Auth(format!("{step} failed — {other}")),
    }
}

// --- Wire types ----------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct MsToken {
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    id_token: String,
}

#[derive(Debug, Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XboxResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: DisplayClaims,
}

#[derive(Debug, Deserialize)]
struct DisplayClaims {
    xui: Vec<Xui>,
}

#[derive(Debug, Deserialize)]
struct Xui {
    uhs: String,
    /// XUID — present on the XSTS response.
    #[serde(default)]
    xid: Option<String>,
}

struct Xsts {
    token: String,
    uhs: String,
    xuid: String,
}

#[derive(Debug, Deserialize)]
struct XstsError {
    #[serde(rename = "XErr", default)]
    xerr: u64,
}

impl XstsError {
    fn friendly_message(&self) -> Option<String> {
        let msg = match self.xerr {
            2148916233 => "This account has no Xbox profile. Create one at xbox.com, then try again.",
            2148916235 => "Xbox Live is not available in your account's region.",
            2148916236 | 2148916237 => "This account requires adult verification.",
            2148916238 => "This account is a child account and must be added to a Family.",
            _ => return None,
        };
        Some(msg.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct McLoginResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct McProfile {
    id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xsts_error_maps_to_friendly_message() {
        let e = XstsError { xerr: 2148916238 };
        assert!(e.friendly_message().unwrap().contains("child account"));
        let unknown = XstsError { xerr: 1 };
        assert!(unknown.friendly_message().is_none());
    }

    #[test]
    fn parses_device_code_response() {
        let json = r#"{
            "user_code":"ABCD-EFGH",
            "device_code":"long-device-code",
            "verification_uri":"https://microsoft.com/link",
            "expires_in":900,
            "interval":5,
            "message":"Go to https://microsoft.com/link and enter ABCD-EFGH"
        }"#;
        let dc: DeviceCode = serde_json::from_str(json).unwrap();
        assert_eq!(dc.user_code, "ABCD-EFGH");
        assert_eq!(dc.interval, 5);
    }
}
