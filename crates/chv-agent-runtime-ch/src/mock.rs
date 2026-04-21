use async_trait::async_trait;
use chv_errors::ChvError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::adapter::{AddDiskParams, AddNetParams, CloudHypervisorAdapter, VmConfig, VmCounters, VmInfo};

#[derive(Debug, Clone, Default)]
pub struct MockCloudHypervisorAdapter {
    pub vms: Arc<Mutex<HashMap<String, VmConfig>>>,
}

#[async_trait]
impl CloudHypervisorAdapter for MockCloudHypervisorAdapter {
    async fn create_vm(
        &self,
        config: &VmConfig,
        _operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        self.vms
            .lock()
            .unwrap()
            .insert(config.vm_id.clone(), config.clone());
        Ok(config.vm_id.clone())
    }

    async fn start_vm(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn stop_vm(
        &self,
        _vm_id: &str,
        _force: bool,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn delete_vm(&self, vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }

    async fn reboot_vm(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn pause_vm(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn resume_vm(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn power_button(&self, _vm_id: &str, _operation_id: Option<&str>) -> Result<(), ChvError> {
        Ok(())
    }

    async fn resize_vm(
        &self,
        vm_id: &str,
        cpus: Option<u32>,
        memory_bytes: Option<u64>,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let mut map = self.vms.lock().unwrap();
        let config = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        if let Some(c) = cpus {
            config.cpus = c;
        }
        if let Some(m) = memory_bytes {
            config.memory_bytes = m;
        }
        Ok(())
    }

    async fn add_disk(
        &self,
        _vm_id: &str,
        _params: &AddDiskParams,
        _operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        Ok("mock-disk-id".to_string())
    }

    async fn remove_device(
        &self,
        _vm_id: &str,
        _device_id: &str,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn add_net(
        &self,
        _vm_id: &str,
        _params: &AddNetParams,
        _operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        Ok("mock-net-id".to_string())
    }

    async fn resize_disk(
        &self,
        _vm_id: &str,
        _disk_id: &str,
        _new_size_bytes: u64,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn snapshot_vm(
        &self,
        _vm_id: &str,
        _destination: &str,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn restore_snapshot(
        &self,
        _vm_id: &str,
        _source: &str,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }

    async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError> {
        let map = self.vms.lock().unwrap();
        let config = map.get(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        Ok(VmInfo {
            state: "Running".to_string(),
            cpus: config.cpus,
            memory_bytes: config.memory_bytes,
        })
    }

    async fn vm_counters(&self, _vm_id: &str) -> Result<VmCounters, ChvError> {
        Ok(VmCounters::default())
    }

    async fn ping(&self, _vm_id: &str) -> Result<bool, ChvError> {
        Ok(true)
    }

    async fn coredump(
        &self,
        _vm_id: &str,
        _destination: &str,
        _operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        Ok(())
    }
}
