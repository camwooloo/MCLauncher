//! Microsoft → Xbox Live → Minecraft authentication.
//!
//! Mojang accounts are gone; logging in is a five-step token chain:
//!
//! 1. **Microsoft OAuth2** — we use the *device-code* flow: the user visits a
//!    URL and types a short code, so we need neither a redirect server nor a
//!    custom URL scheme. Yields an MS access token + refresh token.
//! 2. **Xbox Live** — exchange the MS token for an XBL token + user hash.
//! 3. **XSTS** — authorize the XBL token for the Minecraft relying party;
//!    also surfaces the XUID and common "no Xbox account / child account"
//!    errors.
//! 4. **Minecraft** — `login_with_xbox` yields the Minecraft access token.
//! 5. **Profile** — fetch the player's UUID and name.
//!
//! ### Azure setup required
//! You must register a free Azure AD application (Entra ID) to obtain a
//! **client id**, configure it as a *public client* allowing the device-code
//! flow, and have the Minecraft API scope enabled. Pass that id to
//! [`Auth::new`].

use serde::Deserialize;
use serde_json::json;

use crate::account::Account;
use crate::{Error, Result};

const DEVICE_CODE_URL: &str =
    "https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode";
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
