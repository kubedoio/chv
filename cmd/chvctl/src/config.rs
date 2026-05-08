use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

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
    let path = credentials_path();
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn save_credentials(token: &str) -> Result<(), String> {
    let dir = config_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create config dir: {e}"))?;
    fs::write(credentials_path(), token).map_err(|e| format!("failed to save credentials: {e}"))
}
