use crate::error::ControlPlaneServiceError;
use chv_controlplane_store::StorePool;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use tokio::net::TcpListener;
use tracing::info;

#[derive(Debug, Clone)]
pub struct ControlPlaneRuntime {
    bind_addr: SocketAddr,
    runtime_dir: PathBuf,
}

impl ControlPlaneRuntime {
    pub fn new(bind_addr: SocketAddr, runtime_dir: PathBuf) -> Self {
        Self {
            bind_addr,
            runtime_dir,
        }
    }

    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    pub fn runtime_dir(&self) -> &Path {
        &self.runtime_dir
    }
}

#[derive(Clone)]
pub struct ControlPlaneComponents {
    store_pool: StorePool,
}

impl ControlPlaneComponents {
    pub fn new(store_pool: StorePool) -> Self {
        Self { store_pool }
    }

    pub fn store_pool(&self) -> &StorePool {
        &self.store_pool
    }
}

#[derive(Clone)]
pub struct ControlPlaneService {
    runtime: ControlPlaneRuntime,
    components: ControlPlaneComponents,
}

impl ControlPlaneService {
    pub fn new(runtime: ControlPlaneRuntime, components: ControlPlaneComponents) -> Self {
        Self {
            runtime,
            components,
        }
    }

    pub fn runtime(&self) -> &ControlPlaneRuntime {
        &self.runtime
    }

    pub fn components(&self) -> &ControlPlaneComponents {
        &self.components
    }

    pub async fn run(&self) -> Result<(), ControlPlaneServiceError> {
        let _listener = TcpListener::bind(self.runtime.bind_addr()).await?;

        info!(
            bind_addr = %self.runtime.bind_addr(),
            runtime_dir = %self.runtime.runtime_dir().display(),
            "chv-controlplane foundation started"
        );

        tokio::signal::ctrl_c().await?;

        info!("chv-controlplane foundation stopping");
        Ok(())
    }
}
