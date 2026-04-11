use keyring::{Entry, Error as KeyringError};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use crate::domain::ProviderSecretStatus;

const KEYRING_SERVICE: &str = "sol.desktop";
const TORBOX_KEY_ID: &str = "torbox_api_key";
const TMDB_KEY_ID: &str = "tmdb_api_read_token";
const FALLBACK_FILE_NAME: &str = "provider-secrets.json";

#[derive(Clone, Default)]
pub struct SecretStore;

impl SecretStore {
    pub fn status(&self) -> Result<ProviderSecretStatus, String> {
        Ok(ProviderSecretStatus {
            torbox_configured: self.get_torbox_api_key()?.is_some(),
            tmdb_configured: self.get_tmdb_api_read_token()?.is_some(),
        })
    }

    pub fn get_torbox_api_key(&self) -> Result<Option<String>, String> {
        get_secret(TORBOX_KEY_ID)
    }

    pub fn get_tmdb_api_read_token(&self) -> Result<Option<String>, String> {
        get_secret(TMDB_KEY_ID)
    }

    pub fn set_torbox_api_key(&self, value: &str) -> Result<(), String> {
        set_secret(TORBOX_KEY_ID, value)
    }

    pub fn set_tmdb_api_read_token(&self, value: &str) -> Result<(), String> {
        set_secret(TMDB_KEY_ID, value)
    }

    pub fn clear_torbox_api_key(&self) -> Result<(), String> {
        clear_secret(TORBOX_KEY_ID)
    }

    pub fn clear_tmdb_api_read_token(&self) -> Result<(), String> {
        clear_secret(TMDB_KEY_ID)
    }

    pub fn load_into_env(&self) -> Result<(), String> {
        set_or_remove_env("TORBOX_API_KEY", self.get_torbox_api_key()?);
        set_or_remove_env("TMDB_API_READ_TOKEN", self.get_tmdb_api_read_token()?);
        Ok(())
    }
}

fn set_or_remove_env(name: &str, value: Option<String>) {
    if let Some(value) = value {
        // SAFETY: Called during app configuration updates on the main thread.
        unsafe { std::env::set_var(name, value) };
    } else {
        // SAFETY: Called during app configuration updates on the main thread.
        unsafe { std::env::remove_var(name) };
    }
}

fn entry(name: &str) -> Result<Entry, String> {
    Entry::new(KEYRING_SERVICE, name)
        .map_err(|error| format!("Could not open secure store: {error}"))
}

fn get_secret(name: &str) -> Result<Option<String>, String> {
    let entry = entry(name)?;
    match entry.get_password() {
        Ok(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed))
            }
        }
        Err(KeyringError::NoEntry) => read_fallback_secret(name),
        Err(error) => {
            eprintln!("[secrets] keyring read failed for {name}: {error}");
            read_fallback_secret(name)
        }
    }
}

fn set_secret(name: &str, value: &str) -> Result<(), String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return clear_secret(name);
    }
    match entry(name)?.set_password(trimmed) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("[secrets] keyring write failed for {name}: {error}");
        }
    }
    write_fallback_secret(name, Some(trimmed.to_string()))
}

fn clear_secret(name: &str) -> Result<(), String> {
    match entry(name)?.delete_credential() {
        Ok(()) | Err(KeyringError::NoEntry) => {}
        Err(error) => {
            eprintln!("[secrets] keyring clear failed for {name}: {error}");
        }
    }
    write_fallback_secret(name, None)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct ProviderSecretsFile {
    torbox_api_key: Option<String>,
    tmdb_api_read_token: Option<String>,
}

fn read_fallback_secret(name: &str) -> Result<Option<String>, String> {
    let secrets = read_fallback_file()?;
    let value = match name {
        TORBOX_KEY_ID => secrets.torbox_api_key,
        TMDB_KEY_ID => secrets.tmdb_api_read_token,
        _ => None,
    };
    Ok(value.and_then(|candidate| {
        let trimmed = candidate.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    }))
}

fn write_fallback_secret(name: &str, value: Option<String>) -> Result<(), String> {
    let mut secrets = read_fallback_file()?;
    match name {
        TORBOX_KEY_ID => secrets.torbox_api_key = value,
        TMDB_KEY_ID => secrets.tmdb_api_read_token = value,
        _ => {}
    }
    write_fallback_file(&secrets)
}

fn read_fallback_file() -> Result<ProviderSecretsFile, String> {
    let path = fallback_path()?;
    if !path.exists() {
        return Ok(ProviderSecretsFile::default());
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Could not read fallback secrets file: {error}"))?;
    if raw.trim().is_empty() {
        return Ok(ProviderSecretsFile::default());
    }
    serde_json::from_str::<ProviderSecretsFile>(&raw)
        .map_err(|error| format!("Could not parse fallback secrets file: {error}"))
}

fn write_fallback_file(secrets: &ProviderSecretsFile) -> Result<(), String> {
    let path = fallback_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not prepare fallback secrets directory: {error}"))?;
    }
    let serialized = serde_json::to_string_pretty(secrets)
        .map_err(|error| format!("Could not serialize fallback secrets: {error}"))?;
    fs::write(path, serialized)
        .map_err(|error| format!("Could not write fallback secrets file: {error}"))
}

fn fallback_path() -> Result<PathBuf, String> {
    let mut base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| {
            "Could not resolve local data directory for fallback secrets.".to_string()
        })?;
    base.push("sol");
    base.push(FALLBACK_FILE_NAME);
    Ok(base)
}

#[cfg(test)]
mod tests {
    use super::set_or_remove_env;

    #[test]
    fn set_or_remove_env_sets_when_value_present() {
        set_or_remove_env("SOL_TEST_SECRET_ENV_SET", Some("abc123".into()));
        let value = std::env::var("SOL_TEST_SECRET_ENV_SET").expect("env var should be set");
        assert_eq!(value, "abc123");
        set_or_remove_env("SOL_TEST_SECRET_ENV_SET", None);
    }

    #[test]
    fn set_or_remove_env_clears_when_value_missing() {
        // SAFETY: test-scoped environment mutation.
        unsafe { std::env::set_var("SOL_TEST_SECRET_ENV_CLEAR", "temp") };
        set_or_remove_env("SOL_TEST_SECRET_ENV_CLEAR", None);
        assert!(
            std::env::var("SOL_TEST_SECRET_ENV_CLEAR").is_err(),
            "env var should be removed"
        );
    }
}
