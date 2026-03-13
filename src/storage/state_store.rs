use std::path::{Path, PathBuf};

use aes_gcm::{
    Aes256Gcm, KeyInit, Nonce,
    aead::{Aead, OsRng, rand_core::RngCore},
};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Serialize, de::DeserializeOwned};
use tokio::fs;

use crate::{
    storage::keyring_store::KeyringStore,
    types::{StoredConfig, StoredCredentials, StoredSession},
};

const DEFAULT_DIRNAME: &str = ".docmost-local-mcp";

#[derive(Debug, Clone)]
pub struct StateStore {
    pub base_dir: PathBuf,
    config_path: PathBuf,
    session_path: PathBuf,
    credentials_path: PathBuf,
    key_path: PathBuf,
    keyring: KeyringStore,
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct EncryptedPayload {
    iv: String,
    tag: String,
    ciphertext: String,
}

impl StateStore {
    pub fn new(base_dir: Option<PathBuf>) -> Result<Self> {
        let base_dir = match base_dir {
            Some(base_dir) => base_dir,
            None => dirs::home_dir()
                .context("Unable to determine the home directory")?
                .join(DEFAULT_DIRNAME),
        };

        Ok(Self {
            config_path: base_dir.join("config.json"),
            session_path: base_dir.join("session.json"),
            credentials_path: base_dir.join("credentials.enc.json"),
            key_path: base_dir.join("credentials.key"),
            base_dir,
            keyring: KeyringStore,
        })
    }

    pub async fn read_config(&self) -> Result<Option<StoredConfig>> {
        self.read_json_file(&self.config_path).await
    }

    pub async fn write_config(&self, config: &StoredConfig) -> Result<()> {
        self.write_json_file(&self.config_path, config).await
    }

    pub async fn read_session(&self) -> Result<Option<StoredSession>> {
        self.read_json_file(&self.session_path).await
    }

    pub async fn write_session(&self, session: &StoredSession) -> Result<()> {
        self.write_json_file(&self.session_path, session).await
    }

    pub async fn clear_session(&self) -> Result<()> {
        match fs::remove_file(&self.session_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error).context("Failed to clear saved session"),
        }
    }

    pub async fn read_credentials(&self) -> Result<Option<StoredCredentials>> {
        if let Some(credentials) = self.keyring.read_credentials()? {
            return Ok(Some(credentials));
        }

        let Some(payload) = self
            .read_json_file::<EncryptedPayload>(&self.credentials_path)
            .await?
        else {
            return Ok(None);
        };

        let key = self.get_or_create_encryption_key().await?;
        let plaintext = decrypt_string(&payload, &key)?;
        let credentials =
            serde_json::from_str(&plaintext).context("Failed to parse decrypted credentials")?;
        Ok(Some(credentials))
    }

    pub async fn write_credentials(&self, credentials: &StoredCredentials) -> Result<()> {
        if self.keyring.write_credentials(credentials)? {
            return Ok(());
        }

        let key = self.get_or_create_encryption_key().await?;
        let payload = encrypt_string(&serde_json::to_string(credentials)?, &key)?;
        self.write_json_file(&self.credentials_path, &payload).await
    }

    async fn ensure_base_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.base_dir)
            .await
            .with_context(|| format!("Failed to create {}", self.base_dir.display()))?;
        set_mode(&self.base_dir, 0o700).await
    }

    async fn read_json_file<T>(&self, file_path: &Path) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        match fs::read_to_string(file_path).await {
            Ok(contents) => Ok(Some(
                serde_json::from_str(&contents)
                    .with_context(|| format!("Failed to parse {}", file_path.display()))?,
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => {
                Err(error).with_context(|| format!("Failed to read {}", file_path.display()))
            }
        }
    }

    async fn write_json_file<T>(&self, file_path: &Path, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.ensure_base_dir().await?;

        let temp_path = file_path.with_extension(format!(
            "{}.tmp",
            file_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
        ));
        let contents = format!(
            "{}\n",
            serde_json::to_string_pretty(value)
                .with_context(|| format!("Failed to serialize {}", file_path.display()))?
        );

        fs::write(&temp_path, contents)
            .await
            .with_context(|| format!("Failed to write {}", temp_path.display()))?;
        set_mode(&temp_path, 0o600).await?;
        fs::rename(&temp_path, file_path)
            .await
            .with_context(|| format!("Failed to move {} into place", temp_path.display()))?;
        set_mode(file_path, 0o600).await
    }

    async fn get_or_create_encryption_key(&self) -> Result<Vec<u8>> {
        self.ensure_base_dir().await?;

        match fs::read_to_string(&self.key_path).await {
            Ok(value) => STANDARD
                .decode(value.trim())
                .context("Failed to decode stored encryption key"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut key = vec![0u8; 32];
                OsRng.fill_bytes(&mut key);
                fs::write(&self.key_path, STANDARD.encode(&key))
                    .await
                    .with_context(|| format!("Failed to write {}", self.key_path.display()))?;
                set_mode(&self.key_path, 0o600).await?;
                Ok(key)
            }
            Err(error) => {
                Err(error).with_context(|| format!("Failed to read {}", self.key_path.display()))
            }
        }
    }
}

fn encrypt_string(plaintext: &str, key: &[u8]) -> Result<EncryptedPayload> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("Failed to initialize AES-256-GCM"))?;
    let mut iv = [0u8; 12];
    OsRng.fill_bytes(&mut iv);
    let ciphertext_with_tag = cipher
        .encrypt(Nonce::from_slice(&iv), plaintext.as_bytes())
        .map_err(|_| anyhow::anyhow!("Failed to encrypt credentials"))?;
    let split_index = ciphertext_with_tag
        .len()
        .checked_sub(16)
        .context("Encrypted payload was shorter than the GCM tag")?;

    let (ciphertext, tag) = ciphertext_with_tag.split_at(split_index);

    Ok(EncryptedPayload {
        iv: STANDARD.encode(iv),
        tag: STANDARD.encode(tag),
        ciphertext: STANDARD.encode(ciphertext),
    })
}

fn decrypt_string(payload: &EncryptedPayload, key: &[u8]) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| anyhow::anyhow!("Failed to initialize AES-256-GCM"))?;
    let iv = STANDARD
        .decode(&payload.iv)
        .context("Failed to decode IV")?;
    let tag = STANDARD
        .decode(&payload.tag)
        .context("Failed to decode tag")?;
    let ciphertext = STANDARD
        .decode(&payload.ciphertext)
        .context("Failed to decode ciphertext")?;

    let mut combined = ciphertext;
    combined.extend_from_slice(&tag);
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&iv), combined.as_ref())
        .map_err(|_| anyhow::anyhow!("Failed to decrypt credentials"))?;

    String::from_utf8(plaintext).context("Decrypted credentials were not valid UTF-8")
}

#[cfg(unix)]
async fn set_mode(path: &Path, mode: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let permissions = std::fs::Permissions::from_mode(mode);
    fs::set_permissions(path, permissions)
        .await
        .with_context(|| format!("Failed to set permissions on {}", path.display()))
}

#[cfg(not(unix))]
async fn set_mode(_path: &Path, _mode: u32) -> Result<()> {
    Ok(())
}
