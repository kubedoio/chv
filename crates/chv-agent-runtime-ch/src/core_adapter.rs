use crate::adapter::{CloudHypervisorAdapter, VmConfig, VmDiskConfig, VmNicConfig};
use async_trait::async_trait;
use cellhv_core_executor::{CoreVmRuntime, RuntimeFailure};
use cellhv_core_operations::{MutationCommand, OperationJournalEntry};
use cellhv_core_types::VmDefinition;
use std::sync::Arc;

pub struct CoreCloudHypervisorRuntime {
    adapter: Arc<dyn CloudHypervisorAdapter>,
}

impl CoreCloudHypervisorRuntime {
    pub fn new(adapter: Arc<dyn CloudHypervisorAdapter>) -> Self {
        Self { adapter }
    }
}

fn translate_vm_config(def: &VmDefinition) -> VmConfig {
    VmConfig {
        vm_id: def.id.to_string(),
        cpus: def.compute.vcpus,
        memory_bytes: def.compute.memory_bytes,
        kernel_path: std::path::PathBuf::from(&def.boot.kernel),
        firmware_path: def.boot.firmware.as_ref().map(std::path::PathBuf::from),
        disks: def
            .storage
            .iter()
            .map(|disk| VmDiskConfig {
                path: std::path::PathBuf::from(&disk.attachment_id), // Not quite right, but wait!
                read_only: false,
                id: None,
            })
            .collect(),
        nics: def
            .networks
            .iter()
            .map(|nic| VmNicConfig {
                network_id: nic.attachment_id.clone(),
                mac_address: nic.mac_address.clone().unwrap_or_default(),
                ip_address: "".to_string(),          // we don't have this
                tap_name: nic.attachment_id.clone(), // just temporary?
                cidr: "".to_string(),
                gateway: "".to_string(),
            })
            .collect(),
        api_socket_path: std::path::PathBuf::from(format!("/var/run/chv/{}.sock", def.id)),
        cloud_init_userdata: None,
        hypervisor_overrides: None,
    }
}

#[async_trait]
impl CoreVmRuntime for CoreCloudHypervisorRuntime {
    async fn execute(
        &self,
        operation: OperationJournalEntry,
    ) -> Result<Option<serde_json::Value>, RuntimeFailure> {
        let op_id = operation.operation.id.to_string();
        let cmd: MutationCommand = serde_json::from_value(operation.request)
            .map_err(|_| RuntimeFailure::InvalidRequest)?;

        match cmd {
            MutationCommand::CreateVm { definition } => {
                let config = translate_vm_config(&definition);
                self.adapter
                    .create_vm(&config, Some(&op_id))
                    .await
                    .map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            MutationCommand::StartVm { vm_id } => {
                self.adapter
                    .start_vm(vm_id.as_str(), Some(&op_id))
                    .await
                    .map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            MutationCommand::StopVm { vm_id } => {
                self.adapter
                    .stop_vm(vm_id.as_str(), false, Some(&op_id))
                    .await
                    .map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            MutationCommand::DeleteVm { vm_id } => {
                self.adapter
                    .delete_vm(vm_id.as_str(), Some(&op_id))
                    .await
                    .map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            MutationCommand::RebootVm { vm_id } => {
                self.adapter
                    .reboot_vm(vm_id.as_str(), Some(&op_id))
                    .await
                    .map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            _ => Err(RuntimeFailure::Unsupported),
        }
    }
}
