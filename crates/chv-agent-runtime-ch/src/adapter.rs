use async_trait::async_trait;
use chv_errors::ChvError;
use std::os::fd::OwnedFd;
use std::path::PathBuf;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VmInfo {
    pub state: String,
    pub cpus: u32,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vm_id: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel_path: PathBuf,
    pub firmware_path: Option<PathBuf>,
    pub disks: Vec<VmDiskConfig>,
    pub nics: Vec<VmNicConfig>,
    pub api_socket_path: PathBuf,
    pub cloud_init_userdata: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VmDiskConfig {
    pub path: PathBuf,
    pub read_only: bool,
}

#[derive(Debug, Clone)]
pub struct VmNicConfig {
    pub network_id: String,
    pub mac_address: String,
    pub ip_address: String,
    pub tap_name: String,
}

#[async_trait]
pub trait CloudHypervisorAdapter: Send + Sync + 'static {
    async fn create_vm(
        &self,
        config: &VmConfig,
        operation_id: Option<&str>,
    ) -> Result<String, ChvError>;
    async fn start_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn stop_vm(
        &self,
        vm_id: &str,
        force: bool,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError>;
    async fn delete_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;
    async fn reboot_vm(&self, vm_id: &str, operation_id: Option<&str>) -> Result<(), ChvError>;

    async fn resize_vm(
        &self,
        vm_id: &str,
        cpus: Option<u32>,
        memory_bytes: Option<u64>,
        operation_id: Option<&str>,
    ) -> Result<(), ChvError> {
        let _ = (vm_id, cpus, memory_bytes, operation_id);
        Err(ChvError::Internal {
            reason: "resize_vm not implemented".to_string(),
        })
    }

    async fn vm_info(&self, vm_id: &str) -> Result<VmInfo, ChvError> {
        let _ = vm_id;
        Err(ChvError::Internal {
            reason: "vm_info not implemented".to_string(),
        })
    }

    fn pty_master(&self, _vm_id: &str) -> Option<OwnedFd> {
        None
    }
}
