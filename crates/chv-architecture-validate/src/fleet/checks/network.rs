//! Network bridge / VLAN availability and IP collisions.

use std::borrow::Cow;
use std::collections::HashSet;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::InventorySnapshot;
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    let all_bridges: HashSet<&str> = inv
        .nodes
        .iter()
        .flat_map(|n| n.bridges.iter().map(String::as_str))
        .collect();
    let all_vlans: HashSet<u32> = inv
        .nodes
        .iter()
        .flat_map(|n| n.vlans.iter().copied())
        .collect();
    let all_used_ips: HashSet<&str> = inv
        .nodes
        .iter()
        .flat_map(|n| n.used_ips.iter().map(String::as_str))
        .collect();

    for (idx, net) in model.networks.iter().enumerate() {
        if let Some(bridge) = net.bridge.as_deref() {
            if !all_bridges.contains(bridge) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: Cow::Borrowed(codes::BRIDGE_UNAVAILABLE),
                    message: format!(
                        "network {} references bridge {} which no host exposes",
                        net.name, bridge
                    ),
                    path: Some(format!("networks[{idx}].bridge")),
                    resource_ref: Some(format!("network/{}", net.name)),
                    blocking: true,
                    suggestion: Some(format!(
                        "configure bridge {bridge} on at least one host or pick a known bridge"
                    )),
                });
            }
        }
        if let Some(vlan) = net.vlan_id {
            if !all_vlans.contains(&vlan) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: Cow::Borrowed(codes::VLAN_UNAVAILABLE),
                    message: format!(
                        "network {} references VLAN {} which no host permits",
                        net.name, vlan
                    ),
                    path: Some(format!("networks[{idx}].vlan_id")),
                    resource_ref: Some(format!("network/{}", net.name)),
                    blocking: true,
                    suggestion: Some(format!(
                        "trunk VLAN {vlan} on a host or pick an allowed VLAN id"
                    )),
                });
            }
        }
    }

    // IP collisions: any explicit instance IP that is already in use by the
    // live fleet (and not declared by the same instance — a re-declaration
    // is fine for the same resource).
    for (idx, inst) in model.instances.iter().enumerate() {
        for (nidx, attach) in inst.networks.iter().enumerate() {
            let Some(ip) = attach.ip.as_deref() else {
                continue;
            };
            if all_used_ips.contains(ip) {
                findings.push(Finding {
                    severity: Severity::Error,
                    code: Cow::Borrowed(codes::IP_ALREADY_USED),
                    message: format!(
                        "instance {} requests IP {} which is already in use in the fleet",
                        inst.name, ip
                    ),
                    path: Some(format!("instances[{idx}].networks[{nidx}].ip")),
                    resource_ref: Some(format!("instance/{}", inst.name)),
                    blocking: true,
                    suggestion: Some("pick a free IP or release the existing reservation".into()),
                });
            }
        }
    }

    findings
}
