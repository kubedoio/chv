use control_plane_node_api::control_plane_node_api as proto;
use std::path::{Path, PathBuf};

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

    fn probe_storage_classes(base: &Path) -> Vec<String> {
        // Known storage class subdirectory names mirroring the stord backend names.
        const KNOWN: &[&str] = &["localdisk", "ceph", "nfs"];
        KNOWN.iter()
            .filter(|&&name| base.join(name).is_dir())
            .map(|&name| name.to_string())
            .collect()
    }

    pub fn build_inventory(&self) -> proto::NodeInventory {
        proto::NodeInventory {
            node_id: self.node_id.clone(),
            hostname: self.hostname.clone(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_threads: 0,  // TODO: probe host
            memory_bytes: 0, // TODO: probe host
            storage_classes: Self::probe_storage_classes(&self.storage_base_dir),
            network_capabilities: vec![],
            labels: std::collections::HashMap::new(),
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
}
