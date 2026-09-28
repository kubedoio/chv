use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    pub server_url: Option<String>,
}

fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("chvctl")
}

fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

fn credentials_path() -> PathBuf {
    config_dir().join("credentials")
}

pub fn load() -> Config {
    let path = config_path();
    if !path.exists() {
        return Config::default();
    }

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Config::default(),
    };

    toml::from_str(&content).unwrap_or_default()
}

pub fn load_credentials() -> Option<String> {
    load_credentials_at(&credentials_path())
}

fn load_credentials_at(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn save_credentials(token: &str) -> Result<(), String> {
    save_credentials_at(&config_dir(), token)
}

fn save_credentials_at(dir: &Path, token: &str) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    fs::create_dir_all(dir).map_err(|e| format!("failed to create config dir: {e}"))?;
    let path = dir.join("credentials");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .mode(0o600)
        .create(true)
        .truncate(true)
        .open(&path)
        .map_err(|e| format!("failed to save credentials: {e}"))?;
    file.write_all(token.as_bytes())
        .map_err(|e| format!("failed to save credentials: {e}"))?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|e| format!("failed to set credentials permissions: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn credentials_round_trip_overwrites_and_is_owner_private() {
        // Regression: `OpenOptions` without `.write(true)` cannot create or
        // truncate — std rejects the combination with "creating or
        // truncating a file requires write or append access", so
        // `chvctl login` failed after a successful API login on every
        // invocation.
        let dir = tempfile::tempdir().unwrap();
        save_credentials_at(dir.path(), "first-token").unwrap();
        assert_eq!(
            load_credentials_at(&dir.path().join("credentials")).as_deref(),
            Some("first-token")
        );

        // Re-login overwrites the previous token rather than appending.
        save_credentials_at(dir.path(), "second-token").unwrap();
        assert_eq!(
            load_credentials_at(&dir.path().join("credentials")).as_deref(),
            Some("second-token")
        );

        let mode = fs::metadata(dir.path().join("credentials"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
