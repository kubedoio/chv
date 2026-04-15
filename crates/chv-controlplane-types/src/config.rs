use std::net::SocketAddr;
use std::path::PathBuf;

use crate::domain::NodeId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlPlaneConfig {
    pub node_id: NodeId,
    pub api: ApiConfig,
    pub persistence: PersistenceConfig,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiConfig {
    pub listen_addr: SocketAddr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PersistenceConfig {
    pub state_dir: PathBuf,
    pub event_dir: PathBuf,
}
