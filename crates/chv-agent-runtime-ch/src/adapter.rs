use async_trait::async_trait;
use chv_errors::ChvError;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub vm_id: String,
    pub cpus: u32,
    pub memory_bytes: u64,
    pub kernel_path: PathBuf,
    pub disk_paths: Vec<PathBuf>,
}

#[async_trait]
pub trait CloudHypervisorAdapter: Send + Sync + 'static {
    async fn create_vm(&self, config: &VmConfig) -> Result<String, ChvError>;
    async fn start_vm(&self, vm_id: &str) -> Result<(), ChvError>;
    async fn stop_vm(&self, vm_id: &str, force: bool) -> Result<(), ChvError>;
    async fn delete_vm(&self, vm_id: &str) -> Result<(), ChvError>;
}
