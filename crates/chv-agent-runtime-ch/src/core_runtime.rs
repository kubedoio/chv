use cellhv_core_executor::{CoreVmRuntime, RuntimeFailure};
use cellhv_core_operations::OperationJournalEntry;
use cellhv_core_types::{OperationKind, VmDefinition};
use std::sync::Arc;
use crate::adapter::{CloudHypervisorAdapter, VmConfig, VmDiskConfig, VmNicConfig};

pub struct CloudHypervisorCoreRuntime {
    adapter: Arc<dyn CloudHypervisorAdapter>,
}

impl CloudHypervisorCoreRuntime {
    pub fn new(adapter: Arc<dyn CloudHypervisorAdapter>) -> Self {
        Self { adapter }
    }

    fn translate(&self, def: &VmDefinition) -> VmConfig {
        VmConfig {
            vm_id: def.id.as_str().to_string(),
            cpus: def.compute.vcpus,
            memory_bytes: def.compute.memory_bytes,
            kernel_path: std::path::PathBuf::from(&def.boot.kernel),
            firmware_path: def.boot.firmware.as_ref().map(std::path::PathBuf::from),
            disks: def.storage.iter().map(|s| VmDiskConfig {
                path: std::path::PathBuf::from(&s.storage_ref),
                read_only: s.read_only,
                id: Some(s.attachment_id.clone()),
            }).collect(),
            nics: def.networks.iter().map(|n| VmNicConfig {
                network_id: n.network_ref.clone(),
                mac_address: n.mac_address.clone().unwrap_or_default(),
                ip_address: "".to_string(),
                tap_name: n.attachment_id.clone(),
                cidr: "".to_string(),
                gateway: "".to_string(),
            }).collect(),
            api_socket_path: std::path::PathBuf::from(format!("/var/run/chv/{}.sock", def.id.as_str())),
            cloud_init_userdata: None,
            hypervisor_overrides: None,
        }
    }
}

#[async_trait::async_trait]
impl CoreVmRuntime for CloudHypervisorCoreRuntime {
    async fn execute(
        &self,
        operation: OperationJournalEntry,
    ) -> std::result::Result<Option<serde_json::Value>, RuntimeFailure> {
        let op_id = operation.operation.id.as_str();
        match operation.operation.kind {
            OperationKind::CreateVm => {
                let def: VmDefinition = serde_json::from_value(operation.request.clone())
                    .map_err(|_| RuntimeFailure::InvalidRequest)?;
                let config = self.translate(&def);
                self.adapter.create_vm(&config, Some(op_id)).await.map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            OperationKind::StartVm => {
                self.adapter.start_vm(operation.operation.vm_id.as_str(), Some(op_id)).await.map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            OperationKind::StopVm => {
                // Determine if force from request? For now just force=false
                self.adapter.stop_vm(operation.operation.vm_id.as_str(), false, Some(op_id)).await.map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            OperationKind::RebootVm => {
                self.adapter.reboot_vm(operation.operation.vm_id.as_str(), Some(op_id)).await.map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            OperationKind::DeleteVm => {
                self.adapter.delete_vm(operation.operation.vm_id.as_str(), Some(op_id)).await.map_err(|_| RuntimeFailure::Internal)?;
                Ok(None)
            }
            OperationKind::UpdateVm | OperationKind::AttachVolume | OperationKind::DetachVolume | OperationKind::AttachNetwork | OperationKind::DetachNetwork => {
                let _def: VmDefinition = serde_json::from_value(operation.request.clone())
                    .map_err(|_| RuntimeFailure::InvalidRequest)?;
                
                Ok(None)
            }
        }
    }
}
