//! Production [`HostResourceController`] implementation for the Core runtime.
//!
//! The single-effector Core runtime (M2.2a) performs every stord/nwd side
//! effect through the neutral `chv_hypervisor_api::HostResourceController`
//! trait; this module is the production adapter that speaks to the real
//! `chv-stord` / `chv-nwd` daemons.
//!
//! # Client connect model (Decision 3)
//! Each method connects a **fresh** `StordClient`/`NwdClient` over its socket
//! and immediately closes it when the call returns, exactly matching the
//! existing agent convention (`agent_server.rs` and `reconcile.rs` connect per
//! call every time; `Reconciler` holds socket `PathBuf`s, never persistent
//! clients). There are no persistent clients and no lazy caching.
//!
//! Construction never fails fast: a down stord/nwd is only observable at
//! `execute()` time as a failed connect, which the Core runtime maps to
//! `RuntimeFailure::RuntimeUnavailable`. That keeps the agent alive for
//! start/stop (which do not touch stord/nwd); only create/delete are affected
//! by a down provider.

use crate::daemon_clients::{NwdClient, StordClient};
use async_trait::async_trait;
use chv_errors::ChvError;
use chv_hypervisor_api::resources::HostResourceController;
use std::collections::HashMap;
use std::path::PathBuf;

/// Production host-resource controller connecting `chv-stord` and `chv-nwd`
/// on demand, per call.
#[derive(Debug, Clone)]
pub struct AgentResourceController {
    stord_socket: PathBuf,
    nwd_socket: PathBuf,
}

impl AgentResourceController {
    pub fn new(stord_socket: PathBuf, nwd_socket: PathBuf) -> Self {
        Self {
            stord_socket,
            nwd_socket,
        }
    }
}

#[async_trait]
impl HostResourceController for AgentResourceController {
    async fn open_volume(
        &self,
        volume_id: &str,
        backend_class: &str,
        locator: &str,
        options: HashMap<String, String>,
        operation_id: Option<&str>,
    ) -> Result<(String, String, String), ChvError> {
        let mut client = StordClient::connect(&self.stord_socket).await?;
        client
            .open_volume_with_options(volume_id, backend_class, locator, options, operation_id)
            .await
    }

    async fn attach_volume_to_vm(
        &self,
        volume_id: &str,
        vm_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError> {
        let mut client = StordClient::connect(&self.stord_socket).await?;
        client
            .attach_volume_to_vm(volume_id, vm_id, attachment_handle, operation_id)
            .await
    }

    async fn detach_volume_from_vm(
        &self,
        volume_id: &str,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut client = StordClient::connect(&self.stord_socket).await?;
        client
            .detach_volume_from_vm(volume_id, vm_id, force, operation_id)
            .await
    }

    async fn close_volume(
        &self,
        volume_id: &str,
        attachment_handle: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut client = StordClient::connect(&self.stord_socket).await?;
        client
            .close_volume(volume_id, attachment_handle, operation_id)
            .await
    }

    async fn ensure_network_topology(
        &self,
        network_id: &str,
        bridge_name: &str,
        subnet_cidr: &str,
        gateway_ip: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut client = NwdClient::connect(&self.nwd_socket).await?;
        client
            .ensure_network_topology(
                network_id,
                bridge_name,
                subnet_cidr,
                gateway_ip,
                operation_id,
            )
            .await
    }

    async fn attach_vm_nic(
        &self,
        nic_id: &str,
        vm_id: &str,
        network_id: &str,
        mac_address: &str,
        ip_address: &str,
        operation_id: Option<&str>,
    ) -> Result<(String, String), ChvError> {
        let mut client = NwdClient::connect(&self.nwd_socket).await?;
        client
            .attach_vm_nic(
                nic_id,
                vm_id,
                network_id,
                mac_address,
                ip_address,
                operation_id,
            )
            .await
    }

    async fn detach_vm_nic(
        &self,
        nic_id: &str,
        vm_id: &str,
        network_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut client = NwdClient::connect(&self.nwd_socket).await?;
        client
            .detach_vm_nic(nic_id, vm_id, network_id, operation_id)
            .await
    }
}
