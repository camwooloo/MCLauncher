//! The player account model.
//!
//! Shared between launch (which needs the token + uuid for game arguments) and
//! the Microsoft auth flow (which produces a fully-populated [`Account`]).
//!
//! An [`Account::offline`] variant lets us launch singleplayer for testing
//! before the online auth flow is wired up. Offline accounts use the standard
//! "OfflinePlayer:<name>" UUID derivation so worlds stay consistent.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// In-game name.
    pub username: String,
    /// Profile UUID as 32 lowercase hex chars (no hyphens), as Mojang returns.
    pub uuid: String,
    /// Minecraft access token (a JWT for online accounts; "0" offline).
    pub access_token: String,
    /// Xbox user id; empty for offline accounts.
    #[serde(default)]
    pub xuid: String,
    /// `"msa"` for Microsoft accounts, `"legacy"` for offline.
    pub user_type: String,
}

impl Account {
    /// Construct an offline account for singleplayer testing. The game will run
    /// but cannot authenticate with online servers.
    pub fn offline(username: impl Into<String>) -> Self {
        let username = username.into();
        let uuid = offline_uuid(&username);
        Self {
            username,
            uuid,
            access_token: "0".to_string(),
            xuid: String::new(),
            user_type: "legacy".to_string(),
        }
    }

    /// Is this an online (authenticated) account?
    pub fn is_online(&self) -> bool {
        self.user_type == "msa" && self.access_token != "0"
    }
}

/// Derive a stable offline UUID from a username, mirroring the vanilla
/// "OfflinePlayer:<name>" convention closely enough for consistent worlds.
///
/// (Vanilla uses a name-based UUIDv3 over MD5; we use SHA-1 over the same seed
/// and set the version/variant bits to produce a well-formed UUID. The exact
/// bytes differ from vanilla but are stable per name, which is what matters.)
fn offline_uuid(username: &str) -> String {
    let digest = crate::util::sha1_hex(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes = [0u8; 16];
    let raw = hex::decode(&digest[..32]).unwrap_or_default();
    bytes.copy_from_slice(&raw[..16]);
    // Set version (3) and RFC 4122 variant bits.
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    hex::encode(bytes)
}

/// A persisted account, including the refresh token needed for silent
/// re-login. Offline accounts store an empty refresh token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    #[serde(flatten)]
    pub account: Account,
    #[serde(default)]
    pub refresh_token: String,
}

/// On-disk store of known accounts and which one is active.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccountStore {
    #[serde(default)]
    pub accounts: Vec<StoredAccount>,
    /// UUID of the currently selected account.
    #[serde(default)]
    pub active_uuid: Option<String>,
}

impl AccountStore {
    /// Load the store from disk, returning an empty store if it doesn't exist.
    pub async fn load(path: &std::path::Path) -> crate::Result<Self> {
        match tokio::fs::read(path).await {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(crate::Error::io(path, e)),
        }
    }

    /// Persist the store to disk (pretty-printed).
    pub async fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        crate::util::ensure_parent(path).await?;
        let bytes = serde_json::to_vec_pretty(self)?;
        tokio::fs::write(path, bytes)
            .await
            .map_err(|e| crate::Error::io(path, e))
    }

    /// Insert or update an account (matched by UUID) and make it active.
    pub fn upsert(&mut self, account: Account, refresh_token: String) {
        let uuid = account.uuid.clone();
        let entry = StoredAccount {
            account,
            refresh_token,
        };
        match self.accounts.iter_mut().find(|a| a.account.uuid == uuid) {
            Some(existing) => *existing = entry,
            None => self.accounts.push(entry),
        }
        self.active_uuid = Some(uuid);
    }

    /// Remove an account by UUID.
    pub fn remove(&mut self, uuid: &str) {
        self.accounts.retain(|a| a.account.uuid != uuid);
        if self.active_uuid.as_deref() == Some(uuid) {
            self.active_uuid = self.accounts.first().map(|a| a.account.uuid.clone());
        }
    }

    /// The currently active account, if any.
    pub fn active(&self) -> Option<&StoredAccount> {
        let uuid = self.active_uuid.as_deref()?;
        self.accounts.iter().find(|a| a.account.uuid == uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_upsert_and_active() {
        let mut store = AccountStore::default();
        store.upsert(Account::offline("Steve"), String::new());
        let steve_uuid = store.active().unwrap().account.uuid.clone();
        store.upsert(Account::offline("Alex"), "rt".into());
        assert_eq!(store.accounts.len(), 2);
        assert_eq!(store.active().unwrap().account.username, "Alex");
        // Re-upserting Steve updates in place, doesn't duplicate.
        store.upsert(Account::offline("Steve"), String::new());
        assert_eq!(store.accounts.len(), 2);
        store.remove(&steve_uuid);
        assert_eq!(store.accounts.len(), 1);
    }

    #[test]
    fn offline_uuid_is_stable_and_well_formed() {
        let a = Account::offline("Steve");
        let b = Account::offline("Steve");
        assert_eq!(a.uuid, b.uuid);
        assert_eq!(a.uuid.len(), 32);
        // Version nibble is 3.
        assert_eq!(&a.uuid[12..13], "3");
        assert!(!a.is_online());
    }

    #[test]
    fn different_names_differ() {
        assert_ne!(Account::offline("Alex").uuid, Account::offline("Steve").uuid);
    }
}
