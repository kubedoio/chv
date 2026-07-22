//! Reconciliation of VXLAN/FDB state.
//!
//! This module provides:
//! - **Startup reconciliation**: On NWD restart, compares the system's existing
//!   VXLAN interfaces against known topologies and logs orphans.
//! - **FDB reconciliation**: When overlay topology changes at runtime (new peer
//!   joins or leaves), computes the delta of VTEP endpoints and issues the
//!   corresponding add/delete FDB entry calls.

use crate::executor::NetworkExecutor;
use crate::state::TopologyTable;
use chv_errors::ChvError;
use std::collections::HashSet;
use tokio::process::Command;
use tracing::{info, warn};

/// Result of an FDB reconciliation pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FdbReconcileResult {
    /// Number of FDB entries added for newly discovered peer VTEPs.
    pub added: usize,
    /// Number of FDB entries removed for departed peer VTEPs.
    pub removed: usize,
}

/// Reconcile FDB entries for a network's overlay when VTEP endpoints change.
///
/// This function computes the delta between the previously known peer VTEPs
/// (`old_vteps`) and the current set (`new_vteps`), then:
/// - Calls `add_fdb_entry` for each VTEP in `new_vteps` that was not in `old_vteps`.
/// - Calls `delete_fdb_entry` for each VTEP in `old_vteps` that is not in `new_vteps`.
///
/// Uses the broadcast MAC address `00:00:00:00:00:00` for BUM traffic FDB entries,
/// matching the convention used by `ensure_topology`.
///
/// Returns `Ok(FdbReconcileResult)` with counts, or the first error encountered.
pub async fn reconcile_fdb_entries<E: NetworkExecutor>(
    executor: &E,
    namespace: &str,
    vni: u32,
    old_vteps: &[String],
    new_vteps: &[String],
) -> Result<FdbReconcileResult, ChvError> {
    let old_set: HashSet<&str> = old_vteps.iter().map(|s| s.as_str()).collect();
    let new_set: HashSet<&str> = new_vteps.iter().map(|s| s.as_str()).collect();

    let to_add: Vec<&str> = new_set.difference(&old_set).copied().collect();
    let to_remove: Vec<&str> = old_set.difference(&new_set).copied().collect();

    if to_add.is_empty() && to_remove.is_empty() {
        info!(
            namespace = %namespace,
            vni = vni,
            "FDB reconciliation: no changes needed"
        );
        return Ok(FdbReconcileResult {
            added: 0,
            removed: 0,
        });
    }

    // Add FDB entries for new peer VTEPs
    for vtep_ip in &to_add {
        executor
            .add_fdb_entry(namespace, vni, "00:00:00:00:00:00", vtep_ip)
            .await?;
    }

    // Remove FDB entries for departed peer VTEPs
    for vtep_ip in &to_remove {
        executor
            .delete_fdb_entry(namespace, vni, "00:00:00:00:00:00", vtep_ip)
            .await?;
    }

    info!(
        namespace = %namespace,
        vni = vni,
        added = to_add.len(),
        removed = to_remove.len(),
        "FDB reconciliation complete"
    );
    metrics::counter!("chv_nwd_fdb_entries_reconciled_total")
        .increment((to_add.len() + to_remove.len()) as u64);

    Ok(FdbReconcileResult {
        added: to_add.len(),
        removed: to_remove.len(),
    })
}

/// Reconcile local VXLAN interface state against known topologies.
///
/// This function:
/// 1. Lists existing VXLAN interfaces on the system via `ip -d link show type vxlan`.
/// 2. Compares against the set of known networks from the topology table.
/// 3. Logs warnings for any orphaned VXLAN interfaces (those not matching a known VNI).
///
/// This is best-effort: errors in listing or parsing are logged and do not propagate.
pub async fn reconcile_on_startup(topologies: &TopologyTable) {
    info!("starting VXLAN reconciliation on startup");

    let system_vxlan_interfaces = match list_system_vxlan_interfaces().await {
        Ok(interfaces) => interfaces,
        Err(e) => {
            warn!(error = %e, "failed to list system VXLAN interfaces during reconciliation; skipping");
            return;
        }
    };

    if system_vxlan_interfaces.is_empty() {
        info!("no existing VXLAN interfaces found on system; reconciliation complete");
        return;
    }

    // Collect expected VNIs from known topologies
    let known_vnis: HashSet<u32> = topologies.list().iter().filter_map(|t| t.vni).collect();

    let mut orphaned_count = 0u32;
    for iface in &system_vxlan_interfaces {
        if let Some(vni) = extract_vni_from_interface_name(&iface.name) {
            if !known_vnis.contains(&vni) {
                warn!(
                    interface = %iface.name,
                    vni = vni,
                    "orphaned VXLAN interface detected: no matching network in local state"
                );
                orphaned_count += 1;
            }
        } else {
            // Interface name doesn't follow our naming convention (vxlan{VNI}),
            // so it may have been created by another system. Log but don't act.
            warn!(
                interface = %iface.name,
                "VXLAN interface with unrecognized naming convention found during reconciliation"
            );
        }
    }

    info!(
        total_vxlan_interfaces = system_vxlan_interfaces.len(),
        known_vnis = known_vnis.len(),
        orphaned = orphaned_count,
        "VXLAN reconciliation complete"
    );
}

/// Information about a VXLAN interface found on the system.
#[derive(Debug, Clone)]
struct VxlanInterfaceInfo {
    name: String,
}

/// List VXLAN interfaces present on the system by running `ip -d link show type vxlan`.
async fn list_system_vxlan_interfaces() -> Result<Vec<VxlanInterfaceInfo>, String> {
    let output = Command::new("ip")
        .args(["-d", "link", "show", "type", "vxlan"])
        .output()
        .await
        .map_err(|e| format!("failed to execute `ip -d link show type vxlan`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // On systems with no VXLAN interfaces, the command may succeed with empty output
        // or fail with a benign error. Treat non-zero exit as empty list if stderr
        // indicates "does not exist" or similar.
        if stderr.contains("does not exist") || stderr.contains("No such") {
            return Ok(Vec::new());
        }
        return Err(format!(
            "`ip -d link show type vxlan` exited with status {}: {}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let interfaces = parse_ip_link_output(&stdout);
    Ok(interfaces)
}

/// Parse the output of `ip -d link show type vxlan` to extract interface names.
///
/// The output format looks like:
/// ```text
/// 4: vxlan100: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 ...
///     vxlan id 100 local 10.0.0.1 dev eth0 srcport 0 0 dstport 4789 ...
/// ```
///
/// We extract the interface name from lines that match the pattern `N: NAME: <...>`.
fn parse_ip_link_output(output: &str) -> Vec<VxlanInterfaceInfo> {
    let mut interfaces = Vec::new();

    for line in output.lines() {
        // Interface header lines start with a number followed by colon
        let trimmed = line.trim();
        if let Some(_rest) = trimmed.strip_prefix(|c: char| c.is_ascii_digit()) {
            // Find the pattern: "N: ifname: <flags>"
            // The line starts with digits, then ": ", then the interface name, then ":"
            let full_line = trimmed;
            if let Some(colon_pos) = full_line.find(": ") {
                let after_index = &full_line[colon_pos + 2..];
                // Extract interface name (up to the next colon or @)
                let iface_name = after_index.split([':', '@']).next().unwrap_or("").trim();
                if !iface_name.is_empty() {
                    interfaces.push(VxlanInterfaceInfo {
                        name: iface_name.to_string(),
                    });
                }
            }
        }
    }

    interfaces
}

/// Extract VNI from an interface name following the convention `vxlan{VNI}`.
fn extract_vni_from_interface_name(name: &str) -> Option<u32> {
    name.strip_prefix("vxlan")?.parse::<u32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use chv_nwd_api::chv_nwd_api::TopologySpec;
    use std::sync::Mutex;

    /// A mock executor that records FDB calls for verification.
    struct MockExecutor {
        fdb_adds: Mutex<Vec<(String, u32, String, String)>>,
        fdb_deletes: Mutex<Vec<(String, u32, String, String)>>,
    }

    impl MockExecutor {
        fn new() -> Self {
            Self {
                fdb_adds: Mutex::new(Vec::new()),
                fdb_deletes: Mutex::new(Vec::new()),
            }
        }

        fn added_entries(&self) -> Vec<(String, u32, String, String)> {
            self.fdb_adds.lock().unwrap().clone()
        }

        fn deleted_entries(&self) -> Vec<(String, u32, String, String)> {
            self.fdb_deletes.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl NetworkExecutor for MockExecutor {
        async fn ensure_topology(
            &self,
            _spec: &TopologySpec,
        ) -> Result<crate::executor::TopologyApplyResult, ChvError> {
            unimplemented!()
        }

        async fn delete_topology(
            &self,
            _network_id: &str,
            _state: &crate::state::TopologyState,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn health(
            &self,
            _network_id: &str,
            _state: &crate::state::TopologyState,
        ) -> Result<String, ChvError> {
            unimplemented!()
        }

        async fn attach_vm_nic(
            &self,
            _network_id: &str,
            _nic_id: &str,
            _vm_id: &str,
            _bridge_name: &str,
            _mac_address: &str,
            _ip_address: &str,
        ) -> Result<(String, String), ChvError> {
            unimplemented!()
        }

        async fn detach_vm_nic(
            &self,
            _nic_id: &str,
            _ownership: chv_common::AttachmentOwnership,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_firewall_policy(
            &self,
            _network_id: &str,
            _policy_version: &str,
            _policy_json: &[u8],
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_nat_policy(
            &self,
            _network_id: &str,
            _policy_version: &str,
            _policy_json: &[u8],
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn ensure_dhcp_scope(
            &self,
            _network_id: &str,
            _cidr: &str,
            _range_start: &str,
            _range_end: &str,
            _dns_servers: &[String],
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn ensure_dns_scope(
            &self,
            _network_id: &str,
            _forwarders: &[&str],
            _static_records: &std::collections::HashMap<String, String>,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn expose_service(
            &self,
            _network_id: &str,
            _exposure_id: &str,
            _protocol: &str,
            _external_port: u32,
            _target_ip: &str,
            _target_port: u32,
            _mode: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn withdraw_service_exposure(
            &self,
            _network_id: &str,
            _exposure_id: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn create_vxlan_interface(
            &self,
            _namespace: &str,
            _bridge_name: &str,
            _vni: u32,
            _vtep_ip: &str,
            _vtep_port: u32,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn delete_vxlan_interface(
            &self,
            _namespace: &str,
            _vni: u32,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn add_fdb_entry(
            &self,
            namespace: &str,
            vni: u32,
            mac_address: &str,
            vtep_ip: &str,
        ) -> Result<(), ChvError> {
            self.fdb_adds.lock().unwrap().push((
                namespace.to_string(),
                vni,
                mac_address.to_string(),
                vtep_ip.to_string(),
            ));
            Ok(())
        }

        async fn delete_fdb_entry(
            &self,
            namespace: &str,
            vni: u32,
            mac_address: &str,
            vtep_ip: &str,
        ) -> Result<(), ChvError> {
            self.fdb_deletes.lock().unwrap().push((
                namespace.to_string(),
                vni,
                mac_address.to_string(),
                vtep_ip.to_string(),
            ));
            Ok(())
        }

        async fn replace_fdb_entry(
            &self,
            _namespace: &str,
            _vni: u32,
            _mac_address: &str,
            _new_vtep_ip: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn send_gratuitous_arp(
            &self,
            _namespace: &str,
            _bridge_name: &str,
            _vm_ip: &str,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn set_arp_suppression(
            &self,
            _namespace: &str,
            _vni: u32,
            _enabled: bool,
        ) -> Result<(), ChvError> {
            unimplemented!()
        }

        async fn get_overlay_status(
            &self,
            _namespace: &str,
            _vni: u32,
        ) -> Result<crate::executor::OverlayStatusInfo, ChvError> {
            unimplemented!()
        }
    }

    #[tokio::test]
    async fn reconcile_no_changes() {
        let executor = MockExecutor::new();
        let old = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
        let new = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];

        let result = reconcile_fdb_entries(&executor, "ns-test", 100, &old, &new)
            .await
            .unwrap();

        assert_eq!(
            result,
            FdbReconcileResult {
                added: 0,
                removed: 0
            }
        );
        assert!(executor.added_entries().is_empty());
        assert!(executor.deleted_entries().is_empty());
    }

    #[tokio::test]
    async fn reconcile_add_new_vteps() {
        let executor = MockExecutor::new();
        let old = vec!["10.0.0.1".to_string()];
        let new = vec![
            "10.0.0.1".to_string(),
            "10.0.0.2".to_string(),
            "10.0.0.3".to_string(),
        ];

        let result = reconcile_fdb_entries(&executor, "ns-net1", 200, &old, &new)
            .await
            .unwrap();

        assert_eq!(result.added, 2);
        assert_eq!(result.removed, 0);

        let adds = executor.added_entries();
        assert_eq!(adds.len(), 2);
        // All entries should use broadcast MAC and correct namespace/vni
        for (ns, vni, mac, _ip) in &adds {
            assert_eq!(ns, "ns-net1");
            assert_eq!(*vni, 200);
            assert_eq!(mac, "00:00:00:00:00:00");
        }
        let added_ips: HashSet<&str> = adds.iter().map(|(_, _, _, ip)| ip.as_str()).collect();
        assert!(added_ips.contains("10.0.0.2"));
        assert!(added_ips.contains("10.0.0.3"));
    }

    #[tokio::test]
    async fn reconcile_remove_departed_vteps() {
        let executor = MockExecutor::new();
        let old = vec![
            "10.0.0.1".to_string(),
            "10.0.0.2".to_string(),
            "10.0.0.3".to_string(),
        ];
        let new = vec!["10.0.0.1".to_string()];

        let result = reconcile_fdb_entries(&executor, "ns-net1", 300, &old, &new)
            .await
            .unwrap();

        assert_eq!(result.added, 0);
        assert_eq!(result.removed, 2);

        let deletes = executor.deleted_entries();
        assert_eq!(deletes.len(), 2);
        for (ns, vni, mac, _ip) in &deletes {
            assert_eq!(ns, "ns-net1");
            assert_eq!(*vni, 300);
            assert_eq!(mac, "00:00:00:00:00:00");
        }
        let removed_ips: HashSet<&str> = deletes.iter().map(|(_, _, _, ip)| ip.as_str()).collect();
        assert!(removed_ips.contains("10.0.0.2"));
        assert!(removed_ips.contains("10.0.0.3"));
    }

    #[tokio::test]
    async fn reconcile_add_and_remove() {
        let executor = MockExecutor::new();
        let old = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
        let new = vec!["10.0.0.2".to_string(), "10.0.0.3".to_string()];

        let result = reconcile_fdb_entries(&executor, "ns-net1", 100, &old, &new)
            .await
            .unwrap();

        assert_eq!(result.added, 1);
        assert_eq!(result.removed, 1);

        let adds = executor.added_entries();
        assert_eq!(adds.len(), 1);
        assert_eq!(adds[0].3, "10.0.0.3");

        let deletes = executor.deleted_entries();
        assert_eq!(deletes.len(), 1);
        assert_eq!(deletes[0].3, "10.0.0.1");
    }

    #[tokio::test]
    async fn reconcile_from_empty() {
        let executor = MockExecutor::new();
        let old: Vec<String> = Vec::new();
        let new = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];

        let result = reconcile_fdb_entries(&executor, "ns-net1", 100, &old, &new)
            .await
            .unwrap();

        assert_eq!(result.added, 2);
        assert_eq!(result.removed, 0);
    }

    #[tokio::test]
    async fn reconcile_to_empty() {
        let executor = MockExecutor::new();
        let old = vec!["10.0.0.1".to_string(), "10.0.0.2".to_string()];
        let new: Vec<String> = Vec::new();

        let result = reconcile_fdb_entries(&executor, "ns-net1", 100, &old, &new)
            .await
            .unwrap();

        assert_eq!(result.added, 0);
        assert_eq!(result.removed, 2);
    }

    #[test]
    fn parse_ip_link_output_extracts_interfaces() {
        let output = r#"4: vxlan100: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue master br0 state UNKNOWN
    link/ether 9a:1b:2c:3d:4e:5f brd ff:ff:ff:ff:ff:ff
    vxlan id 100 local 10.0.0.1 dev eth0 srcport 0 0 dstport 4789 nolearning
5: vxlan200: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1450 qdisc noqueue master br1 state UNKNOWN
    link/ether aa:bb:cc:dd:ee:ff brd ff:ff:ff:ff:ff:ff
    vxlan id 200 local 10.0.0.1 dev eth0 srcport 0 0 dstport 4789 nolearning
"#;
        let interfaces = parse_ip_link_output(output);
        assert_eq!(interfaces.len(), 2);
        assert_eq!(interfaces[0].name, "vxlan100");
        assert_eq!(interfaces[1].name, "vxlan200");
    }

    #[test]
    fn parse_ip_link_output_empty() {
        let interfaces = parse_ip_link_output("");
        assert!(interfaces.is_empty());
    }

    #[test]
    fn parse_ip_link_output_with_at_sign() {
        // Some interfaces show as "vxlan100@NONE"
        let output = "4: vxlan100@NONE: <BROADCAST,MULTICAST,UP> mtu 1450 qdisc noqueue\n";
        let interfaces = parse_ip_link_output(output);
        assert_eq!(interfaces.len(), 1);
        assert_eq!(interfaces[0].name, "vxlan100");
    }

    #[test]
    fn extract_vni_valid() {
        assert_eq!(extract_vni_from_interface_name("vxlan100"), Some(100));
        assert_eq!(extract_vni_from_interface_name("vxlan1"), Some(1));
        assert_eq!(extract_vni_from_interface_name("vxlan999999"), Some(999999));
    }

    #[test]
    fn extract_vni_invalid() {
        assert_eq!(extract_vni_from_interface_name("br0"), None);
        assert_eq!(extract_vni_from_interface_name("vxlan"), None);
        assert_eq!(extract_vni_from_interface_name("vxlanabc"), None);
        assert_eq!(extract_vni_from_interface_name(""), None);
    }
}
