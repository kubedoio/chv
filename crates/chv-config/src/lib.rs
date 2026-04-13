use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error: {0}")]
    Parse(#[from] toml::de::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct StordConfig {
    pub socket_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_level: String,
    #[serde(default)]
    pub backend_allowlist: Vec<String>,
    pub metrics_bind: Option<String>,
}

impl Default for StordConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/stord/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/stord"),
            log_level: "info".to_string(),
            backend_allowlist: vec![],
            metrics_bind: None,
        }
    }
}

pub fn load_stord_config(path: Option<&Path>) -> Result<StordConfig, ConfigError> {
    let mut cfg = StordConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}

#[derive(Debug, Clone, Deserialize)]
pub struct NwdConfig {
    pub socket_path: PathBuf,
    pub runtime_dir: PathBuf,
    pub log_level: String,
    pub metrics_bind: Option<String>,
}

impl Default for NwdConfig {
    fn default() -> Self {
        Self {
            socket_path: PathBuf::from("/run/chv/nwd/api.sock"),
            runtime_dir: PathBuf::from("/run/chv/nwd"),
            log_level: "info".to_string(),
            metrics_bind: None,
        }
    }
}

pub fn load_nwd_config(path: Option<&Path>) -> Result<NwdConfig, ConfigError> {
    let mut cfg = NwdConfig::default();
    if let Some(p) = path {
        let text = std::fs::read_to_string(p)?;
        cfg = toml::from_str(&text)?;
    }
    Ok(cfg)
}
