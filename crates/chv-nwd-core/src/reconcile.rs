//! Startup reconciliation of local VXLAN/FDB state against known topologies.
//!
//! On NWD restart, this module compares the system's existing VXLAN interfaces
//! against the set of networks tracked in local state (the TopologyTable). Any
//! VXLAN interface that has no corresponding active network is logged as orphaned.
//! Reconciliation is best-effort and does not block startup.

use crate::state::TopologyTable;
use std::collections::HashSet;
use tokio::process::Command;
use tracing::{info, warn};

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
