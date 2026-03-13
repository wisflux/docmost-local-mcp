use anyhow::{Context, Result, anyhow};

use crate::types::StoredCredentials;

const KEYRING_SERVICE: &str = "docmost-local-mcp";
const KEYRING_USERNAME: &str = "credentials";

#[derive(Debug, Clone, Default)]
pub struct KeyringStore;

impl KeyringStore {
    pub fn read_credentials(&self) -> Result<Option<StoredCredentials>> {
        if keyring_disabled() {
            return Ok(None);
        }

        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .context("Failed to initialize keyring entry")?;

        match entry.get_password() {
            Ok(value) => {
                let credentials = serde_json::from_str(&value)
                    .context("Failed to parse keyring credentials payload")?;
                Ok(Some(credentials))
            }
            Err(error) if is_missing_entry(&error) => Ok(None),
            Err(error) if should_fallback(&error) => Ok(None),
            Err(error) => Err(anyhow!(error)).context("Failed to read credentials from keyring"),
        }
    }

    pub fn write_credentials(&self, credentials: &StoredCredentials) -> Result<bool> {
        if keyring_disabled() {
            return Ok(false);
        }

        let entry = keyring::Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .context("Failed to initialize keyring entry")?;
        let value = serde_json::to_string(credentials)
            .context("Failed to encode credentials for keyring")?;

        match entry.set_password(&value) {
            Ok(()) => Ok(true),
            Err(error) if should_fallback(&error) => Ok(false),
            Err(error) => Err(anyhow!(error)).context("Failed to write credentials to keyring"),
        }
    }
}

fn keyring_disabled() -> bool {
    matches!(
        std::env::var("DOCMOST_DISABLE_KEYRING").ok().as_deref(),
        Some("1") | Some("true")
    )
}

fn should_fallback(error: &keyring::Error) -> bool {
    matches!(
        error,
        keyring::Error::PlatformFailure(_)
            | keyring::Error::NoStorageAccess(_)
            | keyring::Error::NoEntry
            | keyring::Error::BadEncoding(_)
    )
}

fn is_missing_entry(error: &keyring::Error) -> bool {
    matches!(error, keyring::Error::NoEntry)
}
