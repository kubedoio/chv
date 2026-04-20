use chv_agent_runtime_ch::adapter::{CloudHypervisorAdapter, VmConfig, VmInfo};
use chv_errors::ChvError;
use std::collections::HashMap;
use std::os::fd::OwnedFd;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct VmRecord {
    pub vm_id: String,
    pub observed_generation: String,
    pub runtime_status: String,
    pub last_error: Option<String>,
}

pub struct VmRuntime {
    vms: Arc<Mutex<HashMap<String, VmRecord>>>,
    adapter: Arc<dyn CloudHypervisorAdapter>,
}

impl Clone for VmRuntime {
    fn clone(&self) -> Self {
        Self {
            vms: self.vms.clone(),
            adapter: self.adapter.clone(),
        }
    }
}

impl VmRuntime {
    pub fn new(adapter: Arc<dyn CloudHypervisorAdapter>) -> Self {
        Self {
            vms: Arc::new(Mutex::new(HashMap::new())),
            adapter,
        }
    }

    pub fn pty_master(&self, vm_id: &str) -> Option<OwnedFd> {
        self.adapter.pty_master(vm_id)
    }

    pub async fn create_vm(
        &self,
        vm_id: impl Into<String>,
        generation: impl Into<String>,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let id = vm_id.into();
        self.adapter.create_vm(config, operation_id).await?;
        let mut map = self.vms.lock().unwrap();
        map.insert(
            id.clone(),
            VmRecord {
                vm_id: id,
                observed_generation: generation.into(),
                runtime_status: "Created".to_string(),
                last_error: None,
            },
        );
        Ok(())
    }

    pub async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.start_vm(vm_id, operation_id).await?;
        let mut map = self.vms.lock().unwrap();
        let rec = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        rec.runtime_status = "Running".to_string();
        Ok(())
    }

    pub async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.stop_vm(vm_id, force, operation_id).await?;
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }

    pub async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.delete_vm(vm_id, operation_id).await?;
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }

    pub async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.reboot_vm(vm_id, operation_id).await?;
        let mut map = self.vms.lock().unwrap();
        let rec = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        rec.runtime_status = "Running".to_string();
        Ok(())
    }

    pub async fn resize_vm(
        &self,
        vm_id: &str,
        cpus: Option<u32>,
        memory_bytes: Option<u64>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter
            .resize_vm(vm_id, cpus, memory_bytes, operation_id)
            .await
    }

    pub async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError> {
        self.adapter.vm_info(vm_id).await
    }

    pub async fn snapshot_vm(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.snapshot_vm(vm_id, destination, operation_id).await
    }

    pub async fn restore_snapshot(
        &self,
        vm_id: &str,
        source: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.restore_snapshot(vm_id, source, operation_id).await
    }

    pub async fn pause_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.pause_vm(vm_id, operation_id).await?;
        let mut map = self.vms.lock().unwrap();
        if let Some(rec) = map.get_mut(vm_id) {
            rec.runtime_status = "Paused".to_string();
        }
        Ok(())
    }

    pub async fn resume_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.resume_vm(vm_id, operation_id).await?;
        let mut map = self.vms.lock().unwrap();
        if let Some(rec) = map.get_mut(vm_id) {
            rec.runtime_status = "Running".to_string();
        }
        Ok(())
    }

    pub async fn power_button(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.power_button(vm_id, operation_id).await
    }

    pub async fn add_disk(
        &self,
        vm_id: &str,
        params: &chv_agent_runtime_ch::adapter::AddDiskParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        self.adapter.add_disk(vm_id, params, operation_id).await
    }

    pub async fn remove_device(
        &self,
        vm_id: &str,
        device_id: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.remove_device(vm_id, device_id, operation_id).await
    }

    pub async fn add_net(
        &self,
        vm_id: &str,
        params: &chv_agent_runtime_ch::adapter::AddNetParams,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError> {
        self.adapter.add_net(vm_id, params, operation_id).await
    }

    pub async fn resize_disk(
        &self,
        vm_id: &str,
        disk_id: &str,
        new_size_bytes: u64,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.resize_disk(vm_id, disk_id, new_size_bytes, operation_id).await
    }

    pub async fn ping(&self, vm_id: &str) -> Result<bool, ChvError> {
        self.adapter.ping(vm_id).await
    }

    pub async fn coredump(
        &self,
        vm_id: &str,
        destination: &str,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        self.adapter.coredump(vm_id, destination, operation_id).await
    }

    pub fn get(&self, vm_id: &str) -> Option<VmRecord> {
        self.vms.lock().unwrap().get(vm_id).cloned()
    }

    pub fn list(&self) -> Vec<VmRecord> {
        self.vms.lock().unwrap().values().cloned().collect()
    }

    pub fn record_failure(
        &self,
        vm_id: impl Into<String>,
        generation: impl Into<String>,
        error: impl Into<String>,
    ) {
        let vm_id = vm_id.into();
        let generation = generation.into();
        let error = error.into();
        let mut map = self.vms.lock().unwrap();
        // Only update existing records; do not create phantom records for VMs
        // that were never successfully created. A phantom record causes the
        // reconciler to skip creation and try start/stop on a non-existent VM.
        if let Some(entry) = map.get_mut(&vm_id) {
            entry.observed_generation = generation;
            entry.runtime_status = "Failed".to_string();
            entry.last_error = Some(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chv_agent_runtime_ch::mock::MockCloudHypervisorAdapter;
    use std::path::PathBuf;

    fn test_runtime() -> (VmRuntime, Arc<MockCloudHypervisorAdapter>) {
        let mock = Arc::new(MockCloudHypervisorAdapter::default());
        (VmRuntime::new(mock.clone()), mock)
    }

    #[tokio::test]
    async fn vm_runtime_create_and_get() {
        let (rt, mock) = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/var/lib/chv/agent/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        let rec = rt.get("vm-1").unwrap();
        assert_eq!(rec.observed_generation, "5");
        assert_eq!(rec.runtime_status, "Created");
        assert!(mock.vms.lock().unwrap().contains_key("vm-1"));
    }

    #[tokio::test]
    async fn vm_runtime_start_and_stop() {
        let (rt, _mock) = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/var/lib/chv/agent/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        rt.start_vm("vm-1", Some("op-2")).await.unwrap();
        assert_eq!(rt.get("vm-1").unwrap().runtime_status, "Running");
        rt.stop_vm("vm-1", false, Some("op-3")).await.unwrap();
        assert!(rt.get("vm-1").is_none(), "VM should be removed from map after stop");
    }

    #[tokio::test]
    async fn vm_runtime_delete() {
        let (rt, _mock) = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/var/lib/chv/agent/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        rt.delete_vm("vm-1", Some("op-4")).await.unwrap();
        assert!(rt.get("vm-1").is_none());
    }

    #[tokio::test]
    async fn vm_runtime_record_failure_upserts_failed_state() {
        let (rt, _mock) = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            firmware_path: None,
            disks: vec![],
            nics: vec![],
            api_socket_path: PathBuf::from("/var/lib/chv/agent/vms/vm-1/vm.sock"),
            cloud_init_userdata: None,
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        rt.record_failure("vm-1", "7", "kernel missing");
        let rec = rt.get("vm-1").unwrap();
        assert_eq!(rec.observed_generation, "7");
        assert_eq!(rec.runtime_status, "Failed");
        assert_eq!(rec.last_error.as_deref(), Some("kernel missing"));
    }

    #[test]
    fn vm_runtime_record_failure_does_not_create_phantom() {
        let (rt, _mock) = test_runtime();
        rt.record_failure("vm-phantom", "1", "prepare failed");
        assert!(rt.get("vm-phantom").is_none());
    }
}
