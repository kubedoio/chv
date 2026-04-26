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
}
