//! Shared VM resource conventions and the neutral host-resource controller trait.
//!
//! This module is the low shared crate boundary for the Core-driven
//! single-effector runtime (M2.2a). It carries:
//!
//! - pure path/string conventions (VM runtime dir, API socket, bridge name,
//!   NIC id, default NIC CIDR) that both the legacy reconcile path and the Core
//!   runtime share without duplicates;
//! - [`ensure_vm_runtime_dir`] which creates the per-VM runtime directory with
//!   the canonical 0o775 mode and a tolerant `chv-stord` group chown;
//! - [`HostResourceController`], the trait through which `CloudHypervisorCoreRuntime`
//!   performs all stord/nwd side effects without depending on `chv-agent-core`.

use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Construct a bridge name for a network, guaranteed to be <= 15 chars (IFNAMSIZ limit).
///
/// For the "default" network, returns "chvbr0". For other networks, returns
/// "br-{net_id}" if it fits in 15 chars, otherwise truncates net_id and appends
/// a 4-hex-char hash suffix to avoid collisions: "br-{prefix}{hash}".
///
/// Relocated verbatim from `crates/chv-agent-core/src/reconcile.rs` so the
/// legacy reconcile path and the Core runtime share one definition (M2.2a).
pub fn bridge_name_for_network(net_id: &str) -> String {
    if net_id == "default" {
        return "chvbr0".to_string();
    }
    let candidate = format!("br-{}", net_id);
    if candidate.len() <= 15 {
        return candidate;
    }
    // "br-" (3) + up to 8 chars of net_id + 4-char hash = 15 chars total
    let prefix: String = net_id.chars().take(8).collect();
    let hash = {
        let mut h: u32 = 0x811c9dc5;
        for b in net_id.as_bytes() {
            h = h.wrapping_mul(0x01000193) ^ (*b as u32);
        }
        format!("{:04x}", h & 0xffff)
    };
    format!("br-{}{}", prefix, hash)
}

/// Returns the per-VM runtime directory for the given VM.
/// This directory holds the VM's socket, logs, PID file, and other runtime artifacts.
///
/// Relocated verbatim from `crates/chv-agent-core/src/reconcile.rs` (M2.2a).
pub fn vm_runtime_dir(base: &Path, vm_id: &str) -> PathBuf {
    base.join("vms").join(vm_id)
}

/// The Cloud Hypervisor API socket for a VM runtime directory.
///
/// Unified convention across the legacy reconcile path and the Core runtime:
/// `{runtime_dir}/vms/{vm_id}/vm.sock`.
pub fn vm_api_socket(vm_dir: &Path) -> PathBuf {
    vm_dir.join("vm.sock")
}

/// Legacy-compatible NIC attachment identifier: `{vm_id}-{network_id}`.
///
/// This exactly matches `cellhv_nodecache_migration::legacy_network_attachment_id`
/// so Core attachment IDs align with the NodeCache projection (M2.2b).
pub fn nic_id(vm_id: &str, network_id: &str) -> String {
    format!("{vm_id}-{network_id}")
}

/// Default NIC subnet CIDR applied when the Core request does not carry one.
///
/// Mirrors the legacy reconcile default (`reconcile.rs` `DEFAULT_NIC_CIDR`).
/// M2.2a residual: the Core create request does not model per-NIC CIDR/gateway,
/// so the runtime always uses this default with an empty gateway.
pub const DEFAULT_NIC_CIDR: &str = "10.0.0.0/24";

/// Ensure the per-VM runtime directory exists with 0o775 permissions and,
/// when running as root and the `chv-stord` group exists, is owned by that
/// group. Both the permission fix and the group chown are best-effort: a
/// failure to chown is logged and does not fail the create (mirrors
/// `reconcile.rs:842-845` semantics).
///
/// # Errors
/// Returns [`ChvError::Io`] only if the directory cannot be created or its
/// permissions cannot be applied.
pub async fn ensure_vm_runtime_dir(vm_dir: &Path) -> Result<(), ChvError> {
    tokio::fs::create_dir_all(vm_dir)
        .await
        .map_err(|e| ChvError::Io {
            path: vm_dir.display().to_string(),
            source: e,
        })?;
    set_mode_775(vm_dir).await?;
    #[cfg(target_family = "unix")]
    chown_chv_stord(vm_dir).await;
    Ok(())
}

#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;

#[cfg(target_family = "unix")]
async fn set_mode_775(vm_dir: &Path) -> Result<(), ChvError> {
    tokio::fs::set_permissions(vm_dir, std::fs::Permissions::from_mode(0o775))
        .await
        .map_err(|e| ChvError::Io {
            path: vm_dir.display().to_string(),
            source: e,
        })
}

#[cfg(not(target_family = "unix"))]
async fn set_mode_775(vm_dir: &Path) -> Result<(), ChvError> {
    // Non-unix targets have no meaningful mode to apply; creation succeeded.
    let _ = vm_dir;
    Ok(())
}

/// Best-effort chown of the VM runtime directory to the `chv-stord` group.
/// Only attempted when running as root (non-root agents cannot change group
/// ownership) and when the group exists; any failure is logged and swallowed
/// so a missing/blocked chown never breaks VM create.
#[cfg(target_family = "unix")]
async fn chown_chv_stord(vm_dir: &Path) {
    if !nix::unistd::geteuid().is_root() {
        return;
    }
    match nix::unistd::Group::from_name("chv-stord") {
        Ok(Some(group)) => {
            if let Err(e) = nix::unistd::chown(vm_dir, None, Some(group.gid)) {
                tracing::warn!(
                    path = %vm_dir.display(),
                    error = %e,
                    "chv-stord group chown failed on vm runtime dir; continuing"
                );
            }
        }
        Ok(None) => {
            tracing::debug!(path = %vm_dir.display(), "chv-stord group not present; skipping chown");
        }
        Err(e) => {
            tracing::warn!(
                path = %vm_dir.display(),
                error = %e,
                "chv-stord group lookup failed; skipping chown"
            );
        }
    }
}

/// Neutral host-resource controller consumed by the Core runtime to perform
/// the stord/nwd side effects of a VM create/delete.
///
/// The production implementation (`chv_agent_core::resources::AgentResourceController`)
/// connects a fresh `StordClient`/`NwdClient` per call, exactly matching the
/// existing agent convention. A publicly exported deterministic mock lives in
/// `crates/chv-agent-runtime-ch/src/mock.rs` so cross-crate integration tests
/// can drive both the runtime and the controller without real daemons.
#[async_trait]
pub trait HostResourceController: Send + Sync + 'static {
    /// Open a volume in the local backend and receive a live attachment handle
    /// plus the export path to attach to the VM.
    ///
    /// # Precondition
    /// The volume is not currently open by this VM lifecycle.
    /// # Postcondition (must be honoured by the caller)
    /// A successful open MUST be followed by [`Self::attach_volume_to_vm`] on
    /// the happy path, and by [`Self::close_volume`] on any path where the
    /// volume ends up unattached. On the happy path the eventual
    /// [`Self::detach_volume_from_vm`] MUST be followed by
    /// [`Self::close_volume`].
    async fn open_volume(
        &self,
        volume_id: &str,
        backend_class: &str,
        locator: &str,
        options: HashMap<String, String>,
        operation_id: Option<&str>,
    ) -> Result<(String, String, String), ChvError>; // (volume_id, attachment_handle, export_path)

    /// Attach an opened volume to a VM, returning the export kind and the final
    /// export path to present to the hypervisor.
    ///
    /// # Precondition
    /// [`Self::open_volume`] succeeded for `attachment_handle`.
    async fn attach_volume_to_vm(
        &self,
        volume_id: &str,
        vm_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError>; // (export_kind, export_path)

    /// Detach a volume from a VM (non-forced).
    async fn detach_volume_from_vm(
        &self,
        volume_id: &str,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    /// Close a previously opened volume attachment handle.
    ///
    /// # Precondition
    /// The volume has been detached (or was never attached).
    async fn close_volume(
        &self,
        volume_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    /// Ensure a network's bridge topology exists.
    async fn ensure_network_topology(
        &self,
        network_id: &str,
        bridge_name: &str,
        subnet_cidr: &str,
        gateway_ip: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;

    /// Attach a NIC to a VM, returning the namespace and tap handles.
    async fn attach_vm_nic(
        &self,
        nic_id: &str,
        vm_id: &str,
        network_id: &str,
        mac_address: &str,
        ip_address: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError>; // (namespace_handle, tap_handle)

    /// Detach a NIC from a VM.
    async fn detach_vm_nic(
        &self,
        nic_id: &str,
        vm_id: &str,
        network_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;
}
