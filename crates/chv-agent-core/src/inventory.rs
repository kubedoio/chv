use control_plane_node_api::control_plane_node_api as proto;
use std::path::{Path, PathBuf};
use tracing::warn;

/// Node fabric identity reported by nwd's `GetFabricIdentity` RPC
/// (ADR-021). An empty public key / zero MTU means "no identity yet"
/// (nwd not up or fabric disabled); the agent re-reports on the next
/// inventory cycle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FabricIdentity {
    pub wireguard_public_key: String,
    pub underlay_mtu: u32,
}

/// Best-effort fetch of the node's fabric identity from the local nwd
/// daemon over its UDS socket.
///
/// Never fails: when nwd is unreachable or the RPC errors (e.g. during
/// enrollment, before nwd is up), a `warn!` is emitted and the empty
/// identity is returned so callers can keep building inventory.
pub async fn fetch_fabric_identity(nwd_socket: &Path) -> FabricIdentity {
    let mut nwd = match crate::daemon_clients::NwdClient::connect(nwd_socket).await {
        Ok(nwd) => nwd,
        Err(e) => {
            warn!(
                error = %e,
                "nwd unavailable for fabric identity; reporting empty fabric identity"
            );
            return FabricIdentity::default();
        }
    };
    match nwd.get_fabric_identity().await {
        Ok((wireguard_public_key, underlay_mtu)) => FabricIdentity {
            wireguard_public_key,
            underlay_mtu,
        },
        Err(e) => {
            warn!(
                error = %e,
                "failed to fetch fabric identity from nwd; reporting empty fabric identity"
            );
            FabricIdentity::default()
        }
    }
}

pub struct InventoryReporter {
    node_id: String,
    hostname: String,
    storage_base_dir: PathBuf,
}

impl InventoryReporter {
    pub fn new(node_id: impl Into<String>, hostname: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            hostname: hostname.into(),
            storage_base_dir: PathBuf::from("/var/lib/chv/storage"),
        }
    }

    pub fn with_storage_base_dir(
        node_id: impl Into<String>,
        hostname: impl Into<String>,
        storage_base_dir: impl Into<PathBuf>,
    ) -> Self {
        Self {
            node_id: node_id.into(),
            hostname: hostname.into(),
            storage_base_dir: storage_base_dir.into(),
        }
    }

    fn probe_kvm_available() -> bool {
        std::path::Path::new("/dev/kvm").exists()
    }

    fn probe_storage_classes(base: &Path) -> Vec<String> {
        // Known storage class subdirectory names mirroring the stord backend names.
        const KNOWN: &[&str] = &["localdisk", "ceph", "nfs"];
        KNOWN
            .iter()
            .filter(|&&name| base.join(name).is_dir())
            .map(|&name| name.to_string())
            .collect()
    }

    pub fn build_inventory(&self) -> proto::NodeInventory {
        self.build_inventory_with_fabric_identity(FabricIdentity::default())
    }

    /// Build inventory carrying the node's fabric identity (ADR-021).
    /// Callers obtain it via [`fetch_fabric_identity`], which is
    /// best-effort and returns the empty identity when nwd is not up.
    pub fn build_inventory_with_fabric_identity(
        &self,
        fabric: FabricIdentity,
    ) -> proto::NodeInventory {
        let mut hypervisor_capabilities = Vec::new();
        if Self::probe_kvm_available() {
            hypervisor_capabilities.push("kvm".to_string());
        }

        proto::NodeInventory {
            node_id: self.node_id.clone(),
            hostname: self.hostname.clone(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_threads: std::thread::available_parallelism()
                .map(|n| n.get() as u64)
                .unwrap_or(0),
            memory_bytes: probe_memory_bytes(),
            storage_classes: Self::probe_storage_classes(&self.storage_base_dir),
            network_capabilities: vec![],
            labels: std::collections::HashMap::new(),
            hypervisor_capabilities,
            vtep_ip: "".to_string(),
            wireguard_public_key: fabric.wireguard_public_key,
            underlay_mtu: fabric.underlay_mtu,
        }
    }

    pub fn build_versions(&self) -> proto::ServiceVersions {
        proto::ServiceVersions {
            node_id: self.node_id.clone(),
            chv_agent_version: env!("CARGO_PKG_VERSION").to_string(),
            chv_stord_version: "".to_string(),
            chv_nwd_version: "".to_string(),
            cloud_hypervisor_version: "".to_string(),
            host_bundle_version: "".to_string(),
        }
    }
}

fn parse_meminfo_total(content: &str) -> u64 {
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let rest = rest.trim();
            if let Some(kb_str) = rest.strip_suffix("kB") {
                if let Ok(kb) = kb_str.trim().parse::<u64>() {
                    return kb * 1024;
                }
            }
        }
    }
    0
}

fn probe_memory_bytes() -> u64 {
    std::fs::read_to_string("/proc/meminfo")
        .map(|c| parse_meminfo_total(&c))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn inventory_has_node_id() {
        let reporter = InventoryReporter::new("node-abc", "host-1");
        let inventory = reporter.build_inventory();
        assert_eq!(inventory.node_id, "node-abc");
        assert_eq!(inventory.hostname, "host-1");
    }

    #[test]
    fn storage_classes_empty_when_no_dirs() {
        let dir = tempdir().unwrap();
        let reporter = InventoryReporter::with_storage_base_dir("n", "h", dir.path());
        let inventory = reporter.build_inventory();
        assert!(inventory.storage_classes.is_empty());
    }

    #[test]
    fn storage_classes_discovered_when_dirs_exist() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("localdisk")).unwrap();
        let reporter = InventoryReporter::with_storage_base_dir("n", "h", dir.path());
        let inventory = reporter.build_inventory();
        assert_eq!(inventory.storage_classes, vec!["localdisk"]);
    }

    #[test]
    fn cpu_threads_is_nonzero() {
        let reporter = InventoryReporter::new("node-x", "host-x");
        let inventory = reporter.build_inventory();
        assert!(
            inventory.cpu_threads > 0,
            "cpu_threads should be > 0 on any real machine"
        );
    }

    #[test]
    fn probe_memory_bytes_format() {
        let mock_meminfo = "MemTotal:       16384000 kB\nMemFree:         8000000 kB\n";
        let bytes = parse_meminfo_total(mock_meminfo);
        assert_eq!(bytes, 16384000 * 1024);
    }

    #[test]
    fn probe_memory_bytes_missing_entry_returns_zero() {
        let mock_meminfo = "MemFree:         8000000 kB\nSwapTotal:       2048000 kB\n";
        let bytes = parse_meminfo_total(mock_meminfo);
        assert_eq!(bytes, 0);
    }

    #[test]
    fn inventory_defaults_to_empty_fabric_identity() {
        let reporter = InventoryReporter::new("node-abc", "host-1");
        let inventory = reporter.build_inventory();
        assert_eq!(inventory.wireguard_public_key, "");
        assert_eq!(inventory.underlay_mtu, 0);
        assert_eq!(inventory.vtep_ip, "");
    }

    #[test]
    fn inventory_carries_fabric_identity() {
        let reporter = InventoryReporter::new("node-abc", "host-1");
        let inventory = reporter.build_inventory_with_fabric_identity(FabricIdentity {
            wireguard_public_key: "pub-key-1".to_string(),
            underlay_mtu: 1500,
        });
        assert_eq!(inventory.wireguard_public_key, "pub-key-1");
        assert_eq!(inventory.underlay_mtu, 1500);
    }

    /// Best-effort contract: an unreachable nwd socket (e.g. during
    /// enrollment, before nwd is up) yields the empty identity instead of
    /// an error, so inventory construction never fails because of nwd.
    #[tokio::test]
    async fn fetch_fabric_identity_unreachable_nwd_returns_empty() {
        let dir = tempdir().unwrap();
        let identity = fetch_fabric_identity(&dir.path().join("no-such-nwd.sock")).await;
        assert_eq!(identity, FabricIdentity::default());
        assert_eq!(identity.wireguard_public_key, "");
        assert_eq!(identity.underlay_mtu, 0);
    }

    /// Best-effort contract: an nwd that answers the identity RPC with an
    /// in-band error (e.g. fabric disabled) also yields the empty identity.
    #[tokio::test]
    async fn fetch_fabric_identity_nwd_error_returns_empty() {
        use crate::daemon_clients::fabric_test_support::{FabricNwdCalls, MockFabricNwd};

        let dir = tempdir().unwrap();
        let socket = dir.path().join("nwd.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        let service = MockFabricNwd {
            calls: std::sync::Arc::new(FabricNwdCalls::default()),
            public_key: String::new(),
            underlay_mtu: 0,
            identity_error: true,
        };
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        service,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        let identity = fetch_fabric_identity(&socket).await;
        assert_eq!(identity, FabricIdentity::default());
    }

    /// End-to-end identity reporting: a reachable nwd serving
    /// GetFabricIdentity populates the inventory fabric fields.
    #[tokio::test]
    async fn fetch_fabric_identity_populates_inventory_fields() {
        use crate::daemon_clients::fabric_test_support::{FabricNwdCalls, MockFabricNwd};

        let dir = tempdir().unwrap();
        let socket = dir.path().join("nwd.sock");

        let uds = tokio::net::UnixListener::bind(&socket).unwrap();
        let service = MockFabricNwd {
            calls: std::sync::Arc::new(FabricNwdCalls::default()),
            public_key: "wg-pub-key-1".to_string(),
            underlay_mtu: 1500,
            identity_error: false,
        };
        tokio::spawn(async move {
            tonic::transport::Server::builder()
                .add_service(
                    chv_nwd_api::chv_nwd_api::network_service_server::NetworkServiceServer::new(
                        service,
                    ),
                )
                .serve_with_incoming(tokio_stream::wrappers::UnixListenerStream::new(uds))
                .await
                .ok();
        });

        let identity = fetch_fabric_identity(&socket).await;
        assert_eq!(identity.wireguard_public_key, "wg-pub-key-1");
        assert_eq!(identity.underlay_mtu, 1500);

        let reporter = InventoryReporter::new("node-abc", "host-1");
        let inventory = reporter.build_inventory_with_fabric_identity(identity);
        assert_eq!(inventory.wireguard_public_key, "wg-pub-key-1");
        assert_eq!(inventory.underlay_mtu, 1500);
    }
}
