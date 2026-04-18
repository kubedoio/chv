use async_trait::async_trait;
use chv_errors::ChvError;
use std::path::PathBuf;

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
}
