use crate::state_machine::NodeState;

/// Minimum free disk space before declaring disk pressure (5 GB).
const MIN_DISK_AVAILABLE_BYTES: u64 = 5 * 1024 * 1024 * 1024;
/// Maximum memory usage percentage before declaring memory pressure.
const MAX_MEMORY_USAGE_PERCENT: f64 = 90.0;
/// Maximum disk usage percentage before declaring disk pressure.
#[allow(dead_code)]
const MAX_DISK_USAGE_PERCENT: f64 = 95.0;

/// Indicates whether the host is under resource pressure.
#[derive(Debug, Clone)]
pub struct ResourcePressure {
    pub disk_pressure: bool,
    pub memory_pressure: bool,
    pub disk_available_bytes: u64,
    pub memory_usage_percent: f64,
}

/// Check host resources for disk and memory pressure using sysinfo.
pub fn check_host_resources() -> ResourcePressure {
    use sysinfo::{Disks, System};

    let mut sys = System::new();
    sys.refresh_memory();
    let memory_total = sys.total_memory();
    let memory_used = sys.used_memory();
    let memory_pct = if memory_total > 0 {
        (memory_used as f64 / memory_total as f64) * 100.0
    } else {
        0.0
    };

    let disks = Disks::new_with_refreshed_list();
    let disk_available = disks
        .iter()
        .find(|d| d.mount_point() == std::path::Path::new("/"))
        .map(|d| d.available_space())
        .unwrap_or(u64::MAX);

    let disk_pressure = disk_available < MIN_DISK_AVAILABLE_BYTES;
    let memory_pressure = memory_pct > MAX_MEMORY_USAGE_PERCENT;

    if disk_pressure {
        tracing::warn!(
            disk_available_bytes = disk_available,
            threshold_bytes = MIN_DISK_AVAILABLE_BYTES,
            "disk pressure detected: available space below threshold"
        );
    }
    if memory_pressure {
        tracing::warn!(
            memory_usage_percent = memory_pct,
            threshold_percent = MAX_MEMORY_USAGE_PERCENT,
            "memory pressure detected: usage above threshold"
        );
    }

    ResourcePressure {
        disk_pressure,
        memory_pressure,
        disk_available_bytes: disk_available,
        memory_usage_percent: memory_pct,
    }
}

#[derive(Debug, Clone, Default)]
pub struct HealthAggregator {
    stord: Option<bool>,
    nwd: Option<bool>,
    resource_pressure: Option<ResourcePressure>,
}

impl HealthAggregator {
    pub fn new() -> Self {
        Self {
            stord: None,
            nwd: None,
            resource_pressure: None,
        }
    }

    pub fn update_stord(&mut self, healthy: bool) {
        self.stord = Some(healthy);
    }

    pub fn update_nwd(&mut self, healthy: bool) {
        self.nwd = Some(healthy);
    }

    /// Update the cached resource pressure state.
    pub fn update_resource_pressure(&mut self, pressure: ResourcePressure) {
        self.resource_pressure = Some(pressure);
    }

    /// Returns whether the host is currently under resource pressure.
    pub fn has_resource_pressure(&self) -> bool {
        self.resource_pressure
            .as_ref()
            .map(|p| p.disk_pressure || p.memory_pressure)
            .unwrap_or(false)
    }

    /// Returns the current resource pressure state, if available.
    pub fn resource_pressure(&self) -> Option<&ResourcePressure> {
        self.resource_pressure.as_ref()
    }

    pub fn derive_node_state(&self, current: NodeState) -> NodeState {
        let stord_ok = self.stord.unwrap_or(false);
        let nwd_ok = self.nwd.unwrap_or(false);

        match current {
            NodeState::Bootstrapping => NodeState::HostReady,
            NodeState::HostReady => {
                if stord_ok {
                    NodeState::StorageReady
                } else {
                    NodeState::HostReady
                }
            }
            NodeState::StorageReady => {
                if !stord_ok {
                    NodeState::Degraded
                } else if nwd_ok {
                    NodeState::NetworkReady
                } else {
                    NodeState::StorageReady
                }
            }
            NodeState::NetworkReady => {
                if stord_ok && nwd_ok {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::TenantReady => {
                if stord_ok && nwd_ok && !self.has_resource_pressure() {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::Degraded => {
                if stord_ok && nwd_ok && !self.has_resource_pressure() {
                    NodeState::TenantReady
                } else {
                    NodeState::Degraded
                }
            }
            NodeState::Failed => {
                if stord_ok && nwd_ok {
                    NodeState::HostReady
                } else if stord_ok || nwd_ok {
                    NodeState::Degraded
                } else {
                    current
                }
            }
            NodeState::Draining | NodeState::Maintenance | NodeState::Discovered => current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_both_ready_goes_tenant_ready() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::StorageReady
        );
    }

    #[test]
    fn health_stord_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_nwd_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_both_down_degrades() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::TenantReady),
            NodeState::Degraded
        );
    }

    #[test]
    fn health_from_host_ready_partial() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(false);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::StorageReady
        );
    }

    #[test]
    fn health_from_host_ready_nwd_only() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(true);
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::HostReady
        );
    }

    #[test]
    fn health_bootstrap_progresses_one_step_at_a_time() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(true);

        assert_eq!(
            h.derive_node_state(NodeState::Bootstrapping),
            NodeState::HostReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::HostReady),
            NodeState::StorageReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::StorageReady),
            NodeState::NetworkReady
        );
        assert_eq!(
            h.derive_node_state(NodeState::NetworkReady),
            NodeState::TenantReady
        );
    }

    #[test]
    fn health_failed_recover_to_host_ready_when_both_up() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(true);
        assert_eq!(h.derive_node_state(NodeState::Failed), NodeState::HostReady);
    }

    #[test]
    fn health_failed_recover_to_degraded_when_partial() {
        let mut h = HealthAggregator::new();
        h.update_stord(true);
        h.update_nwd(false);
        assert_eq!(h.derive_node_state(NodeState::Failed), NodeState::Degraded);
    }

    #[test]
    fn health_failed_stays_failed_when_both_down() {
        let mut h = HealthAggregator::new();
        h.update_stord(false);
        h.update_nwd(false);
        assert_eq!(h.derive_node_state(NodeState::Failed), NodeState::Failed);
    }
}
