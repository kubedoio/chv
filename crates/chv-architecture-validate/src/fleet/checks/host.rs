//! Host placement, schedulability, and resource fit.

use std::borrow::Cow;

use chv_controlplane_types::architecture::{Finding, Severity};

use crate::codes;
use crate::fleet::{InventorySnapshot, NodeInfo};
use crate::model::CHVArchitecture;

pub(super) fn check(model: &CHVArchitecture, inv: &InventorySnapshot) -> Vec<Finding> {
    let mut findings = Vec::new();

    for (idx, inst) in model.instances.iter().enumerate() {
        let placement_server = inst
            .placement
            .as_ref()
            .and_then(|p| p.server.as_deref())
            .map(str::to_string);
        let Some(server_name) = placement_server else {
            continue;
        };

        let path = format!("instances[{idx}].placement.server");
        let resource_ref = format!("instance/{}", inst.name);

        let node = inv.nodes.iter().find(|n| n.name == server_name);
        let Some(node) = node else {
            findings.push(Finding {
                severity: Severity::Error,
                code: Cow::Borrowed(codes::HOST_NOT_FOUND),
                message: format!(
                    "instance {} placed on host {} but no such host exists in the fleet",
                    inst.name, server_name
                ),
                path: Some(path),
                resource_ref: Some(resource_ref),
                blocking: true,
                suggestion: Some(format!(
                    "register host {server_name} or change instance.placement.server"
                )),
            });
            continue;
        };

        if !node.schedulable {
            findings.push(Finding {
                severity: Severity::Error,
                code: Cow::Borrowed(codes::HOST_NOT_SCHEDULABLE),
                message: format!(
                    "instance {} targets host {} which is currently not schedulable",
                    inst.name, node.name
                ),
                path: Some(path.clone()),
                resource_ref: Some(resource_ref.clone()),
                blocking: true,
                suggestion: Some("uncordon the host or pick a different placement".into()),
            });
        }

        let (req_cpu, req_mem_mb) = required_resources(model, inst);
        check_cpu(node, idx, inst, req_cpu, &mut findings);
        check_memory(node, idx, inst, req_mem_mb, &mut findings);
    }

    findings
}

fn required_resources(
    model: &CHVArchitecture,
    inst: &crate::model::Instance,
) -> (Option<u32>, Option<u32>) {
    // Instance overrides win; fall back to template defaults.
    let mut cpu = inst.resources.as_ref().and_then(|r| r.cpu);
    let mut mem_mb = inst.resources.as_ref().and_then(|r| r.memory_mb);
    if cpu.is_none() || mem_mb.is_none() {
        if let Some(tmpl_name) = inst.template.as_deref() {
            if let Some(t) = model.templates.iter().find(|t| t.name == tmpl_name) {
                cpu = cpu.or(t.cpu);
                mem_mb = mem_mb.or(t.memory_mb);
            }
        }
    }
    (cpu, mem_mb)
}

fn check_cpu(
    node: &NodeInfo,
    idx: usize,
    inst: &crate::model::Instance,
    req_cpu: Option<u32>,
    findings: &mut Vec<Finding>,
) {
    let Some(req) = req_cpu else { return };
    if req > node.cpu_cores {
        findings.push(Finding {
            severity: Severity::Error,
            code: Cow::Borrowed(codes::INSUFFICIENT_CPU),
            message: format!(
                "host {} has {} cores but instance {} requests {}",
                node.name, node.cpu_cores, inst.name, req
            ),
            path: Some(format!("instances[{idx}].resources.cpu")),
            resource_ref: Some(format!("instance/{}", inst.name)),
            blocking: true,
            suggestion: Some("reduce instance.resources.cpu or pick a larger host".into()),
        });
    }
}

fn check_memory(
    node: &NodeInfo,
    idx: usize,
    inst: &crate::model::Instance,
    req_mem_mb: Option<u32>,
    findings: &mut Vec<Finding>,
) {
    let Some(req_mb) = req_mem_mb else { return };
    let host_mb: u64 = u64::from(node.memory_gb) * 1024;
    if u64::from(req_mb) > host_mb {
        findings.push(Finding {
            severity: Severity::Error,
            code: Cow::Borrowed(codes::INSUFFICIENT_MEMORY),
            message: format!(
                "host {} has {} GB ({} MB) but instance {} requests {} MB",
                node.name, node.memory_gb, host_mb, inst.name, req_mb
            ),
            path: Some(format!("instances[{idx}].resources.memory_mb")),
            resource_ref: Some(format!("instance/{}", inst.name)),
            blocking: true,
            suggestion: Some("reduce instance.resources.memory_mb or pick a larger host".into()),
        });
    }
}
