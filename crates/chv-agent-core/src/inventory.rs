use control_plane_node_api::control_plane_node_api as proto;

pub struct InventoryReporter {
    node_id: String,
    hostname: String,
}

impl InventoryReporter {
    pub fn new(node_id: impl Into<String>, hostname: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            hostname: hostname.into(),
        }
    }

    pub fn build_inventory(&self) -> proto::NodeInventory {
        proto::NodeInventory {
            node_id: self.node_id.clone(),
            hostname: self.hostname.clone(),
            architecture: std::env::consts::ARCH.to_string(),
            cpu_threads: 0, // TODO: probe host
            memory_bytes: 0, // TODO: probe host
            storage_classes: vec![],
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

    #[test]
    fn inventory_has_node_id() {
        let reporter = InventoryReporter::new("node-abc", "host-1");
        let inventory = reporter.build_inventory();
        assert_eq!(inventory.node_id, "node-abc");
        assert_eq!(inventory.hostname, "host-1");
    }
}
