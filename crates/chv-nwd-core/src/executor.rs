use async_trait::async_trait;
use chv_errors::ChvError;
use chv_nwd_api::chv_nwd_api::TopologySpec;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::{info, warn};

#[derive(Debug, Clone)]
pub struct TopologyApplyResult {
    pub namespace_handle: String,
    pub bridge_handle: String,
}

#[async_trait]
pub trait NetworkExecutor: Send + Sync + 'static {
    async fn ensure_topology(
        &self,
        spec: &TopologySpec,
    ) -> Result<TopologyApplyResult, ChvError>;

    async fn delete_topology(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<(), ChvError>;

    async fn health(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<String, ChvError>;
}

pub struct LinuxExecutor {
    _runtime_dir: PathBuf,
}

impl LinuxExecutor {
    pub fn new(runtime_dir: PathBuf) -> Self {
        Self { _runtime_dir: runtime_dir }
    }

    async fn run_ip(args: &[&str]) -> Result<(), ChvError> {
        let out = Command::new("ip")
            .args(args)
            .output()
            .await
            .map_err(|e| ChvError::Io {
                path: "ip".to_string(),
                source: e,
            })?;

        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            if stderr.contains("File exists") || stderr.contains("already exists") {
                return Ok(());
            }
            return Err(ChvError::NetworkUnavailable {
                resource: "ip".to_string(),
                reason: format!("ip {} failed: {}", args.join(" "), stderr),
            });
        }
        Ok(())
    }

    #[allow(dead_code)]
    async fn run_ip_netns(namespace: &str, args: &[&str]) -> Result<(), ChvError> {
        let mut cmd_args = vec!["netns", "exec", namespace];
        cmd_args.extend(args);
        Self::run_ip(&cmd_args).await
    }

    async fn bridge_exists(name: &str) -> bool {
        Command::new("ip")
            .args(["link", "show", "dev", name])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    async fn namespace_exists(name: &str) -> bool {
        std::path::Path::new("/var/run/netns").join(name).exists()
    }
}

#[async_trait]
impl NetworkExecutor for LinuxExecutor {
    async fn ensure_topology(
        &self,
        spec: &TopologySpec,
    ) -> Result<TopologyApplyResult, ChvError> {
        info!(
            network_id = %spec.network_id,
            bridge = %spec.bridge_name,
            namespace = %spec.namespace_name,
            "ensuring topology"
        );

        // Bridge
        if !Self::bridge_exists(&spec.bridge_name).await {
            Self::run_ip(&["link", "add", &spec.bridge_name, "type", "bridge"]).await?;
        }
        Self::run_ip(&["link", "set", &spec.bridge_name, "up"]).await?;

        // Namespace
        if !Self::namespace_exists(&spec.namespace_name).await {
            Self::run_ip(&["netns", "add", &spec.namespace_name]).await?;
        }

        // Minimal nftables table to satisfy baseline hook
        let _ = Command::new("nft")
            .args(["add", "table", "inet", &format!("chv-{}", spec.network_id)])
            .output()
            .await;

        Ok(TopologyApplyResult {
            namespace_handle: spec.namespace_name.clone(),
            bridge_handle: spec.bridge_name.clone(),
        })
    }

    async fn delete_topology(
        &self,
        network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<(), ChvError> {
        info!(
            network_id = %network_id,
            bridge = %state.bridge_name,
            namespace = %state.namespace_name,
            "deleting topology"
        );

        if Self::namespace_exists(&state.namespace_name).await {
            if let Err(e) = Self::run_ip(&["netns", "del", &state.namespace_name]).await {
                warn!(error = %e, "failed to delete namespace");
            }
        }

        if Self::bridge_exists(&state.bridge_name).await {
            if let Err(e) = Self::run_ip(&["link", "del", "dev", &state.bridge_name]).await {
                warn!(error = %e, "failed to delete bridge");
            }
        }

        let _ = Command::new("nft")
            .args(["delete", "table", "inet", &format!("chv-{}", network_id)])
            .output()
            .await;

        Ok(())
    }

    async fn health(
        &self,
        _network_id: &str,
        state: &crate::state::TopologyState,
    ) -> Result<String, ChvError> {
        let bridge_ok = Self::bridge_exists(&state.bridge_name).await;
        let ns_ok = Self::namespace_exists(&state.namespace_name).await;

        if bridge_ok && ns_ok {
            Ok("healthy".to_string())
        } else {
            let missing: Vec<&str> = [
                if bridge_ok { None } else { Some("bridge") },
                if ns_ok { None } else { Some("namespace") },
            ]
            .into_iter()
            .flatten()
            .collect();
            Ok(format!("degraded: missing {}", missing.join(", ")))
        }
    }
}
