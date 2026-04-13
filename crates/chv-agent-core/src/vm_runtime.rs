use chv_agent_runtime_ch::adapter::{CloudHypervisorAdapter, VmConfig};
use chv_errors::ChvError;
use std::collections::HashMap;
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
        let mut map = self.vms.lock().unwrap();
        let rec = map.get_mut(vm_id).ok_or_else(|| ChvError::NotFound {
            resource: "vm".to_string(),
            id: vm_id.to_string(),
        })?;
        rec.runtime_status = "Stopped".to_string();
        Ok(())
    }

    pub async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError> {
        self.adapter.delete_vm(vm_id, operation_id).await?;
        self.vms.lock().unwrap().remove(vm_id);
        Ok(())
    }

    pub fn get(&self, vm_id: &str) -> Option<VmRecord> {
        self.vms.lock().unwrap().get(vm_id).cloned()
    }

    pub fn list(&self) -> Vec<VmRecord> {
        self.vms.lock().unwrap().values().cloned().collect()
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
            disk_paths: vec![],
            api_socket_path: PathBuf::from("/run/chv/vm-1.sock"),
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
            disk_paths: vec![],
            api_socket_path: PathBuf::from("/run/chv/vm-1.sock"),
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        rt.start_vm("vm-1", Some("op-2")).await.unwrap();
        assert_eq!(rt.get("vm-1").unwrap().runtime_status, "Running");
        rt.stop_vm("vm-1", false, Some("op-3")).await.unwrap();
        assert_eq!(rt.get("vm-1").unwrap().runtime_status, "Stopped");
    }

    #[tokio::test]
    async fn vm_runtime_delete() {
        let (rt, _mock) = test_runtime();
        let config = VmConfig {
            vm_id: "vm-1".to_string(),
            cpus: 2,
            memory_bytes: 1024,
            kernel_path: PathBuf::from("/dev/null"),
            disk_paths: vec![],
            api_socket_path: PathBuf::from("/run/chv/vm-1.sock"),
        };
        rt.create_vm("vm-1", "5", &config, Some("op-1"))
            .await
            .unwrap();
        rt.delete_vm("vm-1", Some("op-4")).await.unwrap();
        assert!(rt.get("vm-1").is_none());
    }
}
