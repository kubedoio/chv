use chv_config::{load_controlplane_config, ConfigError, ControlPlaneConfig};
use std::path::{Path, PathBuf};

pub fn load_config(path: Option<&Path>) -> Result<ControlPlaneConfig, ConfigError> {
    load_controlplane_config(path)
}

pub fn config_path_from_args() -> Option<PathBuf> {
    std::env::args().nth(1).map(PathBuf::from)
}
